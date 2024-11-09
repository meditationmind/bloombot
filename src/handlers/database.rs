#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Timelike, Utc};
use futures::{stream::Stream, StreamExt, TryStreamExt};
use log::{info, warn};
use pgvector::Vector;
use poise::serenity_prelude::{GuildId, MessageId, Role, RoleId, UserId};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use tokio::time;
use ulid::Ulid;

use crate::commands::helpers::time::{ChallengeTimeframe, Timeframe};
use crate::commands::stats::{LeaderboardType, SortBy};
use crate::data::bookmark::Bookmark;
use crate::data::common::{Aggregate, Exists, Migration};
use crate::data::course::{Course, ExtendedCourse};
use crate::data::erase::Erase;
use crate::data::meditation::Meditation;
use crate::data::quote::{Quote, QuoteModal};
use crate::data::star_message::StarMessage;
use crate::data::stats::{Guild, LeaderboardUser, Streak, Timeframe as TimeframeStats, User};
use crate::data::steam_key::{Recipient, SteamKey};
use crate::data::term::{Names, SearchResult, Term};
use crate::data::tracking_profile::TrackingProfile;

#[derive(Debug)]
struct Res {
  times_ago: Option<f64>,
  meditation_minutes: Option<i64>,
  meditation_count: Option<i64>,
}

#[derive(Debug)]
struct MeditationCountByDay {
  days_ago: Option<f64>,
}

#[allow(clippy::module_name_repetitions)]
pub struct DatabaseHandler {
  pool: sqlx::PgPool,
}

pub(crate) trait InsertQuery {
  fn insert_query(&self) -> Query<Postgres, PgArguments>;
}

pub(crate) trait UpdateQuery {
  fn update_query(&self) -> Query<Postgres, PgArguments>;
}

pub(crate) trait DeleteQuery {
  fn delete_query<'a>(
    guild_id: GuildId,
    unique_id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments>;
}

pub(crate) trait ExistsQuery {
  type Item<'a>;

  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    item: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments>;
}

impl DatabaseHandler {
  pub fn from_pool(pool: PgPool) -> Self {
    Self { pool }
  }

  pub async fn new() -> Result<Self> {
    let database_url =
      env::var("DATABASE_URL").with_context(|| "Missing DATABASE_URL environment variable")?;
    // let pool = sqlx::PgPool::connect(&database_url).await?;
    let max_retries = 5;
    let mut attempts = 0;

    loop {
      let pool = match PgPool::connect(&database_url).await {
        Ok(pool) => pool,
        Err(e) => {
          if attempts >= max_retries {
            return Err(e.into());
          }

          // Retry if error is sqlx::Error::Io
          if let sqlx::Error::Io(io_error) = e {
            attempts += 1;
            // Log warning
            warn!(
              "Error establishing a database connection ({}): retry attempt {} of {}",
              io_error.kind(),
              attempts,
              max_retries
            );
            // Wait before retrying
            time::sleep(Duration::from_secs(60)).await;
            continue;
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e.into());
        }
      };

      sqlx::migrate!("./migrations").run(&pool).await?;

      info!(target: "bloombot::database", "Successfully applied migrations.");

      return Ok(Self { pool });
    }
  }

  pub async fn get_connection(&self) -> Result<PoolConnection<Postgres>> {
    Ok(self.pool.acquire().await?)
  }

  pub async fn get_connection_with_retry(
    &self,
    max_retries: usize,
  ) -> Result<PoolConnection<Postgres>> {
    let mut attempts = 0;

    loop {
      match self.get_connection().await {
        Ok(connection) => return Ok(connection),
        Err(e) => {
          if attempts >= max_retries {
            return Err(e);
          }

          // Check if error is sqlx::Error::Io
          if let Some(sqlx::Error::Io(io_error)) = e.downcast_ref::<sqlx::Error>() {
            // Retry if connection was reset
            if io_error.kind() == std::io::ErrorKind::ConnectionReset {
              attempts += 1;
              // Wait for a moment before retrying
              time::sleep(Duration::from_secs(1)).await;
              continue;
            }
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e);
        }
      }
    }
  }

  pub async fn start_transaction(&self) -> Result<Transaction<'_, Postgres>> {
    Ok(self.pool.begin().await?)
  }

  pub async fn start_transaction_with_retry(
    &self,
    max_retries: usize,
  ) -> Result<Transaction<'_, Postgres>> {
    let mut attempts = 0;

    loop {
      match self.start_transaction().await {
        Ok(transaction) => return Ok(transaction),
        Err(e) => {
          if attempts >= max_retries {
            return Err(e);
          }

          // Check if error is sqlx::Error::Io
          if let Some(sqlx::Error::Io(io_error)) = e.downcast_ref::<sqlx::Error>() {
            // Retry if connection was reset
            if io_error.kind() == std::io::ErrorKind::ConnectionReset {
              attempts += 1;
              // Wait for a moment before retrying
              time::sleep(Duration::from_secs(1)).await;
              continue;
            }
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e);
        }
      }
    }
  }

  pub async fn commit_transaction(transaction: Transaction<'_, Postgres>) -> Result<()> {
    transaction.commit().await?;
    Ok(())
  }

  /// This function is not technically necessary, as the transaction will be rolled back when dropped.
  /// However, for readability, it is recommended to call this function when you want to rollback a transaction.
  pub async fn rollback_transaction(transaction: Transaction<'_, Postgres>) -> Result<()> {
    transaction.rollback().await?;
    Ok(())
  }

  pub async fn add_tracking_profile(
    transaction: &mut Transaction<'_, Postgres>,
    tracking_profile: &TrackingProfile,
  ) -> Result<()> {
    tracking_profile
      .insert_query()
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn update_tracking_profile(
    transaction: &mut Transaction<'_, Postgres>,
    tracking_profile: &TrackingProfile,
  ) -> Result<()> {
    tracking_profile
      .update_query()
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn remove_tracking_profile(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<()> {
    TrackingProfile::delete_query(*guild_id, user_id.to_string())
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn migrate_tracking_profile(
    transaction: &mut Transaction<'_, Postgres>,
    migration: &Migration,
  ) -> Result<()> {
    migration.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_tracking_profile(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Option<TrackingProfile>> {
    Ok(
      TrackingProfile::retrieve(*guild_id, *user_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn add_steamkey_recipient(
    transaction: &mut Transaction<'_, Postgres>,
    recipient: &Recipient,
  ) -> Result<()> {
    recipient.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn update_steamkey_recipient(
    transaction: &mut Transaction<'_, Postgres>,
    recipient: &Recipient,
  ) -> Result<()> {
    recipient.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn remove_steamkey_recipient(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<()> {
    Recipient::delete_query(*guild_id, user_id.to_string())
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn get_steamkey_recipient(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Option<Recipient>> {
    Ok(
      Recipient::retrieve_one(*guild_id, *user_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_steamkey_recipients(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Recipient>> {
    Ok(
      Recipient::retrieve_all(*guild_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn steamkey_recipient_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<bool> {
    Ok(
      Recipient::exists_query::<Exists>(*guild_id, *user_id)
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn record_steamkey_receipt(
    connection: &mut PoolConnection<Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<()> {
    let exists = Recipient::exists_query::<Exists>(*guild_id, *user_id)
      .fetch_one(&mut **connection)
      .await?
      .exists;

    Recipient::record_win(*guild_id, *user_id, exists)
      .execute(&mut **connection)
      .await?;

    Ok(())
  }

  pub async fn add_bookmark(
    transaction: &mut Transaction<'_, Postgres>,
    bookmark: &Bookmark,
  ) -> Result<()> {
    bookmark.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_bookmark_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<u64> {
    Ok(
      Bookmark::user_total::<Aggregate>(*guild_id, *user_id)
        .fetch_one(&mut **transaction)
        .await?
        .count,
    )
  }

  pub async fn get_bookmarks(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Vec<Bookmark>> {
    Ok(
      Bookmark::retrieve_all(*guild_id, *user_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn search_bookmarks(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    keyword: &str,
  ) -> Result<Vec<Bookmark>> {
    Ok(
      Bookmark::search(*guild_id, *user_id, keyword)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn remove_bookmark(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    bookmark_id: &str,
  ) -> Result<u64> {
    Ok(
      Bookmark::delete_query(*guild_id, bookmark_id)
        .execute(&mut **transaction)
        .await?
        .rows_affected(),
    )
  }

  pub async fn add_erase(transaction: &mut Transaction<'_, Postgres>, erase: &Erase) -> Result<()> {
    erase.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_erases(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Vec<Erase>> {
    let erases: Vec<Erase> = sqlx::query_as(
      "SELECT record_id, message_link, reason, occurred_at FROM erases WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
    .fetch_all(&mut **transaction)
    .await?;

    Ok(erases)
  }

  pub async fn add_meditation_entry(
    transaction: &mut Transaction<'_, Postgres>,
    meditation_entry: &Meditation,
  ) -> Result<()> {
    meditation_entry
      .insert_query()
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn add_meditation_entry_batch(
    transaction: &mut Transaction<'_, Postgres>,
    batch_query: &str,
  ) -> Result<u64> {
    Ok(
      sqlx::query(batch_query)
        .execute(&mut **transaction)
        .await?
        .rows_affected(),
    )
  }

  pub async fn get_user_meditation_entries(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Vec<Meditation>> {
    Ok(
      Meditation::user_entries(*guild_id, *user_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_meditation_entry(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    meditation_id: &str,
  ) -> Result<Option<Meditation>> {
    Ok(
      Meditation::full_entry(*guild_id, meditation_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_latest_meditation_entry(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Option<Meditation>> {
    Ok(
      Meditation::latest_entry(*guild_id, *user_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn update_meditation_entry(
    transaction: &mut Transaction<'_, Postgres>,
    meditation_entry: &Meditation,
  ) -> Result<()> {
    meditation_entry
      .update_query()
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn remove_meditation_entry(
    transaction: &mut Transaction<'_, Postgres>,
    meditation_id: &str,
  ) -> Result<()> {
    Meditation::delete_query(GuildId::default(), meditation_id)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn reset_user_meditation_entries(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<()> {
    Meditation::remove_all(*guild_id, *user_id)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn migrate_meditation_entries(
    transaction: &mut Transaction<'_, Postgres>,
    migration: &Migration,
  ) -> Result<()> {
    migration.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub fn get_winner_candidates<'a>(
    conn: &'a mut PoolConnection<Postgres>,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    guild_id: &'a GuildId,
  ) -> impl Stream<Item = Result<UserId>> + 'a {
    // All entries that are greater than 0 minutes and within the start and end date
    // We only want a user ID to show up once, so we group by user ID and sum the meditation minutes
    let rows_stream = sqlx::query!(
      "
        SELECT user_id FROM meditation WHERE meditation_minutes > 0 AND occurred_at >= $1 AND occurred_at <= $2 AND guild_id = $3 GROUP BY user_id ORDER BY RANDOM()
      ",
      start_date,
      end_date,
      guild_id.to_string(),
    ).fetch(&mut **conn);

    rows_stream.map(|row| {
      let row = row?;

      let user_id = UserId::new(row.user_id.parse::<u64>()?);

      Ok(user_id)
    })
  }

  pub async fn get_winner_candidate_meditation_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
  ) -> Result<i64> {
    let row = sqlx::query!(
      "
        SELECT (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS winner_candidate_total FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      ",
      user_id.to_string(),
      guild_id.to_string(),
      start_date,
      end_date,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let winner_candidate_total = row
      .winner_candidate_total
      .with_context(|| "Failed to assign winner_candidate_total computed by DB query")?;

    Ok(winner_candidate_total)
  }

  pub async fn get_winner_candidate_meditation_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
  ) -> Result<u64> {
    let row = sqlx::query!(
      "
        SELECT COUNT(record_id) AS winner_candidate_total FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      ",
      user_id.to_string(),
      guild_id.to_string(),
      start_date,
      end_date,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let winner_candidate_total = row
      .winner_candidate_total
      .with_context(|| "Failed to assign winner_candidate_total computed by DB query")?;

    Ok(winner_candidate_total.try_into()?)
  }

  pub async fn get_user_meditation_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<i64> {
    let row = sqlx::query!(
      "
        SELECT (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS user_total FROM meditation WHERE user_id = $1 AND guild_id = $2
      ",
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_total = row
      .user_total
      .with_context(|| "Failed to assign user_total computed by DB query")?;

    Ok(user_total)
  }

  pub async fn get_user_meditation_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      "
        SELECT COUNT(record_id) AS user_total FROM meditation WHERE user_id = $1 AND guild_id = $2
      ",
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_total = row
      .user_total
      .with_context(|| "Failed to assign user_total computed by DB query")?;

    Ok(user_total.try_into()?)
  }

  pub async fn get_guild_meditation_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<i64> {
    let row = sqlx::query!(
      "
        SELECT (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS guild_total FROM meditation WHERE guild_id = $1
      ",
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let guild_total = row
      .guild_total
      .with_context(|| "Failed to assign guild_total computed by DB query")?;

    Ok(guild_total)
  }

  pub async fn get_guild_meditation_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      "
        SELECT COUNT(record_id) AS guild_total FROM meditation WHERE guild_id = $1
      ",
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let guild_total = row
      .guild_total
      .with_context(|| "Failed to assign guild_total computed by DB query")?;

    Ok(guild_total.try_into()?)
  }

  pub async fn get_all_quotes(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Quote>> {
    let rows = sqlx::query!(
      "
        SELECT record_id, quote, author FROM quote WHERE guild_id = $1
      ",
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let quotes = rows
      .into_iter()
      .map(|row| Quote {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      })
      .collect();

    Ok(quotes)
  }

  pub async fn search_quotes(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    keyword: &str,
  ) -> Result<Vec<Quote>> {
    let rows = sqlx::query!(
      "
        SELECT record_id, quote, author
        FROM quote
        WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2))
      ",
      guild_id.to_string(),
      keyword.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let quotes = rows
      .into_iter()
      .map(|row| Quote {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      })
      .collect();

    Ok(quotes)
  }

  pub async fn get_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote_id: &str,
  ) -> Result<Option<Quote>> {
    let row = sqlx::query!(
      "
        SELECT record_id, quote, author FROM quote WHERE record_id = $1 AND guild_id = $2
      ",
      quote_id,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(Quote {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn update_quote(
    transaction: &mut Transaction<'_, Postgres>,
    quote: Quote,
  ) -> Result<()> {
    sqlx::query!(
      "
        UPDATE quote SET quote = $1, author = $2 WHERE record_id = $3
      ",
      quote.quote,
      quote.author,
      quote.id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_random_motivation(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Option<String>> {
    let row = sqlx::query!(
      "
        SELECT quote FROM quote WHERE guild_id = $1 ORDER BY RANDOM() LIMIT 1
      ",
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(row.map(|row| row.quote))
  }

  pub async fn update_streak(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    current: i32,
    longest: i32,
  ) -> Result<()> {
    sqlx::query!(
      "
        INSERT INTO streak (record_id, user_id, guild_id, current_streak, longest_streak) VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id) DO UPDATE SET current_streak = $4, longest_streak = $5
      ",
      Ulid::new().to_string(),
      user_id.to_string(),
      guild_id.to_string(),
      current,
      longest,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_streak(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Streak> {
    let mut streak_data = sqlx::query_as!(
      Streak,
      "
        SELECT current_streak AS current, longest_streak AS longest FROM streak WHERE guild_id = $1 AND user_id = $2
      ",
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?
    .unwrap_or(Streak { current: 0, longest: 0 });

    let mut row = sqlx::query_as!(
      MeditationCountByDay,
      "
      WITH cte AS (
        SELECT date_part('day', NOW() - DATE_TRUNC('day', occurred_at)) AS days_ago
        FROM meditation 
        WHERE user_id = $1 AND guild_id = $2
        AND occurred_at::date <= NOW()::date
      )
      SELECT days_ago
      FROM cte
      GROUP BY days_ago
      ORDER BY days_ago ASC
      ",
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch(&mut **transaction);

    let mut last = 0;
    let mut streak = 0;
    let mut streak_broken = false;

    // Check if currently maintaining a streak
    if let Some(first) = row.try_next().await? {
      #[allow(clippy::cast_possible_truncation)]
      let days_ago = first
        .days_ago
        .with_context(|| "Failed to assign days_ago computed by DB query")?
        as i32;

      if days_ago > 2 {
        streak_broken = true;
        streak_data.current = 0;
      }

      last = days_ago;
      streak = 1;
    }

    // Calculate most recent streak
    while let Some(row) = row.try_next().await? {
      #[allow(clippy::cast_possible_truncation)]
      let days_ago = row
        .days_ago
        .with_context(|| "Failed to assign days_ago computed by DB query")?
        as i32;

      if days_ago != last + 1 {
        last = days_ago;
        break;
      }

      last = days_ago;
      streak += 1;
    }

    if !streak_broken {
      streak_data.current = if streak < 2 { 0 } else { streak };
    }
    // Return early if longest streak has already been calculated
    if streak_data.longest > 0 {
      if streak > streak_data.longest {
        streak_data.longest = if streak < 2 { 0 } else { streak };
      }

      drop(row);
      DatabaseHandler::update_streak(
        transaction,
        guild_id,
        user_id,
        streak_data.current,
        streak_data.longest,
      )
      .await?;

      return Ok(streak_data);
    }
    streak_data.longest = if streak < 2 { 0 } else { streak };
    streak = 1;

    // Calculate longest streak (first time only)
    while let Some(row) = row.try_next().await? {
      #[allow(clippy::cast_possible_truncation)]
      let days_ago = row
        .days_ago
        .with_context(|| "Failed to assign days_ago computed by DB query")?
        as i32;

      if days_ago != last + 1 {
        if streak > streak_data.longest {
          streak_data.longest = streak;
        }
        streak = 1;
        last = days_ago;
        continue;
      }

      last = days_ago;
      streak += 1;
    }

    if streak > streak_data.longest {
      streak_data.longest = if streak < 2 { 0 } else { streak };
    }

    drop(row);
    DatabaseHandler::update_streak(
      transaction,
      guild_id,
      user_id,
      streak_data.current,
      streak_data.longest,
    )
    .await?;

    Ok(streak_data)
  }

  pub async fn course_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
  ) -> Result<bool> {
    Ok(
      Course::exists_query::<Exists>(*guild_id, course_name)
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn add_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
    participant_role: &Role,
    graduate_role: &Role,
  ) -> Result<()> {
    sqlx::query!(
      "
        INSERT INTO course (record_id, course_name, participant_role, graduate_role, guild_id) VALUES ($1, $2, $3, $4, $5)
      ",
      Ulid::new().to_string(),
      course_name,
      participant_role.id.to_string(),
      graduate_role.id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn update_course(
    transaction: &mut Transaction<'_, Postgres>,
    course_name: &str,
    participant_role: String,
    graduate_role: String,
  ) -> Result<()> {
    sqlx::query!(
      "
        UPDATE course SET participant_role = $1, graduate_role = $2 WHERE LOWER(course_name) = LOWER($3)
      ",
      participant_role,
      graduate_role,
      course_name,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn steam_key_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    key: &str,
  ) -> Result<bool> {
    Ok(
      SteamKey::exists_query::<Exists>(*guild_id, Some(key))
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn add_steam_key(
    transaction: &mut Transaction<'_, Postgres>,
    steam_key: &SteamKey,
  ) -> Result<()> {
    steam_key.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_all_steam_keys(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<SteamKey>> {
    Ok(
      SteamKey::retrieve_all(*guild_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn add_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote: QuoteModal,
  ) -> Result<()> {
    sqlx::query!(
      "
        INSERT INTO quote (record_id, quote, author, guild_id) VALUES ($1, $2, $3, $4)
      ",
      Ulid::new().to_string(),
      quote.quote,
      quote.author,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn add_term(
    transaction: &mut Transaction<'_, Postgres>,
    term: Term,
    vector: Vector,
  ) -> Result<()> {
    sqlx::query(
      "
        INSERT INTO term (record_id, term_name, meaning, usage, links, category, aliases, guild_id, embedding) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
      ")
      .bind(Ulid::new().to_string())
      .bind(term.name)
      .bind(term.meaning)
      .bind(term.usage)
      .bind(term.links)
      .bind(term.category)
      .bind(term.aliases)
      .bind(term.guild_id.to_string())
      .bind(vector)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn search_terms_by_vector(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    search_vector: Vector,
    limit: usize,
  ) -> Result<Vec<SearchResult>> {
    // limit should be a small integer
    #[allow(clippy::cast_possible_wrap)]
    let terms: Vec<SearchResult> = sqlx::query_as(
      "
        SELECT term_name, meaning, embedding <=> $1 AS distance_score
        FROM term
        WHERE guild_id = $2
        ORDER BY distance_score ASC
        LIMIT $3
      ",
    )
    .bind(search_vector)
    .bind(guild_id.to_string())
    .bind(limit as i64)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(terms)
  }

  pub async fn get_term(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    let row = sqlx::query!(
      "
        SELECT term_name, meaning, usage, links, category, aliases
        FROM term
        WHERE guild_id = $2
        AND (LOWER(term_name) = LOWER($1)) OR (f_textarr2text(aliases) ~* ('(?:^|,)' || $1 || '(?:$|,)'))
      ",
      term_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let term = match row {
      Some(row) => Some(Term {
        guild_id: *guild_id,
        name: row.term_name,
        meaning: row.meaning,
        usage: row.usage,
        links: row.links,
        category: row.category,
        aliases: row.aliases,
      }),
      None => None,
    };

    Ok(term)
  }

  pub async fn get_term_meaning(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    let row = sqlx::query!(
      "
        SELECT meaning
        FROM term
        WHERE guild_id = $2
        AND (LOWER(term_name) = LOWER($1))
      ",
      term_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let term = match row {
      Some(row) => Some(Term {
        guild_id: *guild_id,
        name: String::new(),
        meaning: row.meaning,
        usage: None,
        links: None,
        category: None,
        aliases: None,
      }),
      None => None,
    };

    Ok(term)
  }

  pub async fn update_term(
    transaction: &mut Transaction<'_, Postgres>,
    term: Term,
    vector: Option<Vector>,
  ) -> Result<()> {
    sqlx::query(
      "
        UPDATE term
        SET meaning = $1, usage = $2, links = $3, category = $4, aliases = $5, embedding = COALESCE($6, embedding)
        WHERE LOWER(term_name) = LOWER($7)
      ",
    )
    .bind(term.meaning)
    .bind(term.usage)
    .bind(term.links)
    .bind(term.category)
    .bind(term.aliases)
    .bind(vector)
    .bind(term.name)
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn update_term_embedding(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
    vector: Option<pgvector::Vector>,
  ) -> Result<()> {
    sqlx::query(
      "
        UPDATE term
        SET embedding = $3
        WHERE guild_id = $1
        AND (LOWER(term_name) = LOWER($2))
      ",
    )
    .bind(guild_id.to_string())
    .bind(term_name)
    .bind(vector)
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_all_courses(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Course>> {
    let rows = sqlx::query!(
      "
        SELECT course_name, participant_role, graduate_role
        FROM course
        WHERE guild_id = $1
        ORDER BY course_name ASC
      ",
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let courses = rows
      .into_iter()
      .map(|row| Course {
        name: row.course_name,
        participant_role: RoleId::new(
          row
            .participant_role
            .parse::<u64>()
            .expect("parse should not fail since participant_role is RoleId.to_string()"),
        ),
        graduate_role: RoleId::new(
          row
            .graduate_role
            .parse::<u64>()
            .expect("parse should not fail since graduate_role is RoleId.to_string()"),
        ),
      })
      .collect();

    Ok(courses)
  }

  pub async fn get_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
  ) -> Result<Option<Course>> {
    let row = sqlx::query!(
      "
        SELECT course_name, participant_role, graduate_role
        FROM course
        WHERE LOWER(course_name) = LOWER($1) AND guild_id = $2
      ",
      course_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let course_data = match row {
      Some(row) => Some(Course {
        name: row.course_name,
        participant_role: RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: RoleId::new(row.graduate_role.parse::<u64>()?),
      }),
      None => None,
    };

    Ok(course_data)
  }

  pub async fn get_course_in_dm(
    transaction: &mut Transaction<'_, Postgres>,
    course_name: &str,
  ) -> Result<Option<ExtendedCourse>> {
    let row = sqlx::query!(
      "
        SELECT course_name, participant_role, graduate_role, guild_id
        FROM course
        WHERE LOWER(course_name) = LOWER($1)
      ",
      course_name,
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let extended_course_data = match row {
      Some(row) => Some(ExtendedCourse {
        name: row.course_name,
        participant_role: RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: RoleId::new(row.graduate_role.parse::<u64>()?),
        guild_id: GuildId::new(
          row
            .guild_id
            .with_context(|| "Failed to retrieve guild_id from DB record")?
            .parse::<u64>()?,
        ),
      }),
      None => None,
    };

    Ok(extended_course_data)
  }

  pub async fn get_possible_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
    similarity: f32,
  ) -> Result<Option<Course>> {
    let row = sqlx::query!(
      "
        SELECT course_name, participant_role, graduate_role, SET_LIMIT($2), SIMILARITY(LOWER(course_name), LOWER($1)) AS similarity_score
        FROM course
        WHERE LOWER(course_name) % LOWER($1) AND guild_id = $3
        ORDER BY similarity_score DESC
        LIMIT 1
      ",
      course_name,
      similarity,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let course_data = match row {
      Some(row) => Some(Course {
        name: row.course_name,
        participant_role: RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: RoleId::new(row.graduate_role.parse::<u64>()?),
      }),
      None => None,
    };

    Ok(course_data)
  }

  pub async fn get_possible_terms(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
    similarity: f32,
  ) -> Result<Vec<Term>> {
    let row = sqlx::query!(
      "
        SELECT term_name, meaning, usage, links, category, aliases, SET_LIMIT($2), SIMILARITY(LOWER(term_name), LOWER($1)) AS similarity_score
        FROM term
        WHERE guild_id = $3
        AND (LOWER(term_name) % LOWER($1)) OR (f_textarr2text(aliases) ILIKE '%' || $1 || '%')
        ORDER BY similarity_score DESC
        LIMIT 5
      ",
      term_name,
      similarity,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    Ok(
      row
        .into_iter()
        .map(|row| Term {
          guild_id: *guild_id,
          name: row.term_name,
          meaning: row.meaning,
          usage: row.usage,
          links: row.links,
          category: row.category,
          aliases: row.aliases,
        })
        .collect(),
    )
  }

  pub async fn get_term_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      "
        SELECT COUNT(record_id) AS term_count FROM term WHERE guild_id = $1
      ",
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let term_count = row
      .term_count
      .with_context(|| "Failed to assign term_count computed by DB query")?;

    Ok(term_count.try_into()?)
  }

  pub async fn get_term_list(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Names>> {
    let rows = sqlx::query!(
      "
        SELECT term_name, aliases
        FROM term
        WHERE guild_id = $1
        ORDER BY term_name DESC
      ",
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let term_list = rows
      .into_iter()
      .map(|row| Names {
        term_name: row.term_name,
        aliases: row.aliases,
      })
      .collect();

    Ok(term_list)
  }

  pub async fn get_all_glossary_terms(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Term>> {
    let rows = sqlx::query!(
      "
        SELECT term_name, meaning
        FROM term
        WHERE guild_id = $1
        ORDER BY term_name ASC
      ",
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let glossary = rows
      .into_iter()
      .map(|row| Term {
        guild_id: *guild_id,
        name: row.term_name,
        meaning: row.meaning,
        usage: None,
        links: None,
        category: None,
        aliases: None,
      })
      .collect();

    Ok(glossary)
  }

  pub async fn unused_key_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<bool> {
    Ok(
      SteamKey::exists_query::<Exists>(*guild_id, None)
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn reserve_key(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Option<String>> {
    Ok(
      SteamKey::reserve(*guild_id, *user_id)
        .fetch_optional(&mut **transaction)
        .await?
        .map(|reserved| reserved.key),
    )
  }

  pub async fn unreserve_key(connection: &mut PoolConnection<Postgres>, key: &str) -> Result<()> {
    SteamKey::unreserve(key).execute(&mut **connection).await?;

    Ok(())
  }

  pub async fn mark_key_used(connection: &mut PoolConnection<Postgres>, key: &str) -> Result<()> {
    SteamKey::mark_used(key).execute(&mut **connection).await?;

    Ok(())
  }

  pub async fn get_key_and_mark_used(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Option<String>> {
    Ok(
      SteamKey::consume(*guild_id)
        .fetch_optional(&mut **transaction)
        .await?
        .map(|consumed| consumed.key),
    )
  }

  pub async fn get_random_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Option<Quote>> {
    let row = sqlx::query!(
      "
        SELECT record_id, quote, author FROM quote WHERE guild_id = $1 ORDER BY RANDOM() LIMIT 1
      ",
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(Quote {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn get_random_quote_with_keyword(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    keyword: &str,
  ) -> Result<Option<Quote>> {
    let row = sqlx::query!(
      "
        SELECT record_id, quote, author
        FROM quote
        WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2))
        ORDER BY RANDOM()
        LIMIT 1
      ",
      guild_id.to_string(),
      keyword.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(Quote {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn remove_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
  ) -> Result<()> {
    sqlx::query!(
      "
        DELETE FROM course WHERE course_name = $1 AND guild_id = $2
      ",
      course_name,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn remove_steam_key(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    key: &str,
  ) -> Result<()> {
    SteamKey::delete_query(*guild_id, key)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn remove_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote: &str,
  ) -> Result<()> {
    sqlx::query!(
      "
        DELETE FROM quote WHERE record_id = $1 AND guild_id = $2
      ",
      quote,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn term_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<bool> {
    Ok(
      Term::exists_query::<Exists>(*guild_id, term_name)
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn remove_term(
    transaction: &mut Transaction<'_, Postgres>,
    term_name: &str,
    guild_id: &GuildId,
  ) -> Result<()> {
    sqlx::query!(
      "
        DELETE FROM term WHERE (LOWER(term_name) = LOWER($1)) AND guild_id = $2
      ",
      term_name,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_challenge_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    timeframe: &ChallengeTimeframe,
  ) -> Result<User> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = Utc::now() + ChronoDuration::minutes(840);
    let start_time = match timeframe {
      ChallengeTimeframe::Monthly => Utc::now()
        .with_day(1)
        .unwrap_or_default()
        .with_hour(0)
        .unwrap_or_default()
        .with_minute(0)
        .unwrap_or_default(),
      ChallengeTimeframe::YearRound => Utc::now()
        .with_month(1)
        .unwrap_or_default()
        .with_day(1)
        .unwrap_or_default()
        .with_hour(0)
        .unwrap_or_default()
        .with_minute(0)
        .unwrap_or_default(),
    };

    let timeframe_data = sqlx::query_as!(
      TimeframeStats,
      "
        SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      ",
      guild_id.to_string(),
      user_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_stats = User {
      all_minutes: 0,
      all_count: 0,
      timeframe_stats: timeframe_data,
      streak: DatabaseHandler::get_streak(transaction, guild_id, user_id).await?,
    };

    Ok(user_stats)
  }

  pub async fn get_leaderboard_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    //user_id: &UserId,
    timeframe: &Timeframe,
    sort_by: &SortBy,
    leaderboard_type: &LeaderboardType,
  ) -> Result<Vec<LeaderboardUser>> {
    let limit = match leaderboard_type {
      LeaderboardType::Top5 => 5,
      LeaderboardType::Top10 => 10,
    };
    match timeframe {
      Timeframe::Daily => {
        let leaderboard_data = match sort_by {
          SortBy::Minutes => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM daily_leaderboard
                WHERE guild = $1
                ORDER BY minutes DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Sessions => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM daily_leaderboard
                WHERE guild = $1
                ORDER BY sessions DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Streak => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM daily_leaderboard
                WHERE guild = $1
                ORDER BY streak DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
        };

        Ok(leaderboard_data)
      }
      Timeframe::Weekly => {
        let leaderboard_data = match sort_by {
          SortBy::Minutes => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM weekly_leaderboard
                WHERE guild = $1
                ORDER BY minutes DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Sessions => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM weekly_leaderboard
                WHERE guild = $1
                ORDER BY sessions DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Streak => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM weekly_leaderboard
                WHERE guild = $1
                ORDER BY streak DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
        };

        Ok(leaderboard_data)
      }
      Timeframe::Monthly => {
        let leaderboard_data = match sort_by {
          SortBy::Minutes => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM monthly_leaderboard
                WHERE guild = $1
                ORDER BY minutes DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Sessions => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM monthly_leaderboard
                WHERE guild = $1
                ORDER BY sessions DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Streak => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM monthly_leaderboard
                WHERE guild = $1
                ORDER BY streak DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
        };

        Ok(leaderboard_data)
      }
      Timeframe::Yearly => {
        let leaderboard_data = match sort_by {
          SortBy::Minutes => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM yearly_leaderboard
                WHERE guild = $1
                ORDER BY minutes DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Sessions => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM yearly_leaderboard
                WHERE guild = $1
                ORDER BY sessions DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
          SortBy::Streak => sqlx::query_as!(
              LeaderboardUser,
              "
                SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private
                FROM yearly_leaderboard
                WHERE guild = $1
                ORDER BY streak DESC
                LIMIT $2
              ",
              guild_id.to_string(),
              limit,
            )
            .fetch_all(&mut **transaction)
            .await?,
        };

        Ok(leaderboard_data)
      }
    }
  }

  pub async fn refresh_leaderboard(
    transaction: &mut Transaction<'_, Postgres>,
    timeframe: &Timeframe,
  ) -> Result<()> {
    match timeframe {
      Timeframe::Yearly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW CONCURRENTLY yearly_leaderboard;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Monthly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW CONCURRENTLY monthly_leaderboard;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Weekly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW CONCURRENTLY weekly_leaderboard;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Daily => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW CONCURRENTLY daily_leaderboard;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
    }

    Ok(())
  }

  pub async fn get_user_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    timeframe: &Timeframe,
  ) -> Result<User> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = Utc::now() + ChronoDuration::minutes(840);
    let start_time = match timeframe {
      Timeframe::Daily => end_time - ChronoDuration::days(12),
      Timeframe::Weekly => end_time - ChronoDuration::weeks(12),
      Timeframe::Monthly => end_time - ChronoDuration::days(30 * 12),
      Timeframe::Yearly => end_time - ChronoDuration::days(365 * 12),
    };

    let total_data = sqlx::query!(
      "
        SELECT COUNT(record_id) AS total_count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS total_sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2
      ",
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let timeframe_data = sqlx::query_as!(
      TimeframeStats,
      "
        SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      ",
      guild_id.to_string(),
      user_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_stats = User {
      all_minutes: total_data.total_sum.unwrap_or(0),
      all_count: total_data.total_count.unwrap_or(0).try_into()?,
      timeframe_stats: timeframe_data,
      streak: DatabaseHandler::get_streak(transaction, guild_id, user_id).await?,
    };

    Ok(user_stats)
  }

  pub async fn get_guild_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    timeframe: &Timeframe,
  ) -> Result<Guild> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = Utc::now() + ChronoDuration::minutes(840);
    let start_time = match timeframe {
      Timeframe::Daily => end_time - ChronoDuration::days(12),
      Timeframe::Weekly => end_time - ChronoDuration::weeks(12),
      Timeframe::Monthly => end_time - ChronoDuration::days(30 * 12),
      Timeframe::Yearly => end_time - ChronoDuration::days(365 * 12),
    };

    let total_data = sqlx::query!(
      "
        SELECT COUNT(record_id) AS total_count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS total_sum
        FROM meditation
        WHERE guild_id = $1
      ",
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let timeframe_data = sqlx::query_as!(
      TimeframeStats,
      "
        SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum
        FROM meditation
        WHERE guild_id = $1 AND occurred_at >= $2 AND occurred_at <= $3
      ",
      guild_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let guild_stats = Guild {
      all_minutes: total_data.total_sum.unwrap_or(0),
      all_count: total_data.total_count.unwrap_or(0).try_into()?,
      timeframe_stats: timeframe_data,
    };

    Ok(guild_stats)
  }

  pub async fn quote_exists(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote_id: &str,
  ) -> Result<bool> {
    Ok(
      Quote::exists_query::<Exists>(*guild_id, quote_id)
        .fetch_one(&mut **transaction)
        .await?
        .exists,
    )
  }

  pub async fn get_user_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    timeframe: &Timeframe,
    offset: i16,
  ) -> Result<Vec<TimeframeStats>> {
    let mut fresh_data: Option<Res> = None;
    let now_offset = Utc::now() + ChronoDuration::minutes(offset.into());

    // Calculate data for last 12 days
    let rows: Vec<Res> = match timeframe {
      Timeframe::Daily => {
        sqlx::query_as!(
          Res,
          "
            WITH daily_data AS
            (
              SELECT
                date_part('day', $1 - DATE_TRUNC('day', occurred_at)) AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $2 AND user_id = $3 AND occurred_at <= $1
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM daily_data
            WHERE times_ago <= 12
            GROUP BY times_ago
          ",
          now_offset,
          guild_id.to_string(),
          user_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present week, get previous 11 weeks from materialized view
      Timeframe::Weekly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_week_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('week', now()) + interval '1 week') - interval '1 second') - occurred_at) /
                  (60*60*24*7)
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1 AND user_id = $2
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_week_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM weekly_data
            WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present month, get previous 11 month from materialized view
      Timeframe::Monthly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_month_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('month', now()) + interval '1 month') - interval '1 second') - occurred_at) /
                  extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at))))
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1 AND user_id = $2
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_month_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM monthly_data
            WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present year, get previous 11 years from materialized view
      Timeframe::Yearly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_year_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('year', now()) + interval '1 year') - interval '1 second') - occurred_at) /
                  extract(epoch from (((date_trunc('year', occurred_at) + interval '1 year') - interval '1 second') - (date_trunc('year', occurred_at))))
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1 AND user_id = $2
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_year_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM yearly_data
            WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
          user_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
    };

    let daily = matches!(timeframe, Timeframe::Daily);
    let range = if daily { 0..12 } else { 1..12 };
    let mut stats: Vec<TimeframeStats> = range
      .map(|i| {
        // Comparison is safe since floor produces integer
        #[allow(clippy::float_cmp)]
        #[allow(clippy::expect_used)]
        let row = rows.iter().find(|row| {
          row
            .times_ago
            .expect("row should include times_ago since it is computed in the DB query")
            == f64::from(i)
        });

        let meditation_minutes = match row {
          Some(row) => row.meditation_minutes.unwrap_or(0),
          None => 0,
        };

        let meditation_count = match row {
          Some(row) => row.meditation_count.unwrap_or(0),
          None => 0,
        };

        TimeframeStats {
          sum: Some(meditation_minutes),
          count: Some(meditation_count),
        }
      })
      .rev()
      .collect();

    if let Some(fresh_data) = fresh_data {
      stats.push(TimeframeStats {
        sum: Some(fresh_data.meditation_minutes.unwrap_or(0)),
        count: Some(fresh_data.meditation_count.unwrap_or(0)),
      });
    } else if !daily {
      stats.push(TimeframeStats {
        sum: Some(0),
        count: Some(0),
      });
    }

    Ok(stats)
  }

  pub async fn get_guild_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    timeframe: &Timeframe,
  ) -> Result<Vec<TimeframeStats>> {
    let mut fresh_data: Option<Res> = None;

    // Calculate data for last 12 days
    let rows: Vec<Res> = match timeframe {
      Timeframe::Daily => {
        sqlx::query_as!(
          Res,
          "
            WITH daily_data AS
            (
              SELECT
                date_part('day', NOW() - DATE_TRUNC('day', occurred_at)) AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM daily_data
            WHERE times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present week, get previous 11 weeks from materialized view
      Timeframe::Weekly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_week_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('week', now()) + interval '1 week') - interval '1 second') - occurred_at) /
                  (60*60*24*7)
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_week_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM weekly_data
            WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present month, get previous 11 month from materialized view
      Timeframe::Monthly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_month_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('month', now()) + interval '1 month') - interval '1 second') - occurred_at) /
                  extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at))))
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_month_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM monthly_data
            WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
      // Calculate fresh data for present year, get previous 11 years from materialized view
      Timeframe::Yearly => {
        fresh_data = sqlx::query_as!(
          Res,
          "
            WITH current_year_data AS
            (
              SELECT
                floor(
                  extract(epoch from ((date_trunc('year', now()) + interval '1 year') - interval '1 second') - occurred_at) /
                  extract(epoch from (((date_trunc('year', occurred_at) + interval '1 year') - interval '1 second') - (date_trunc('year', occurred_at))))
                )::float AS times_ago,
                meditation_minutes,
                meditation_seconds
              FROM meditation
              WHERE guild_id = $1
            )
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM current_year_data
            WHERE times_ago = 0
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        ).fetch_optional(&mut **transaction).await?;

        sqlx::query_as!(
          Res,
          "
            SELECT
              times_ago,
              (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes,
              COUNT(*) AS meditation_count
            FROM yearly_data
            WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12
            GROUP BY times_ago
          ",
          guild_id.to_string(),
        )
        .fetch_all(&mut **transaction)
        .await?
      }
    };

    let daily = matches!(timeframe, Timeframe::Daily);
    let range = if daily { 0..12 } else { 1..12 };
    let mut stats: Vec<TimeframeStats> = range
      .map(|i| {
        // Comparison is safe since floor produces integer
        #[allow(clippy::float_cmp)]
        #[allow(clippy::expect_used)]
        let row = rows.iter().find(|row| {
          row
            .times_ago
            .expect("row should include times_ago since it is computed in the DB query")
            == f64::from(i)
        });

        let meditation_minutes = match row {
          Some(row) => row.meditation_minutes.unwrap_or(0),
          None => 0,
        };

        let meditation_count = match row {
          Some(row) => row.meditation_count.unwrap_or(0),
          None => 0,
        };

        TimeframeStats {
          sum: Some(meditation_minutes),
          count: Some(meditation_count),
        }
      })
      .rev()
      .collect();

    if let Some(fresh_data) = fresh_data {
      stats.push(TimeframeStats {
        sum: Some(fresh_data.meditation_minutes.unwrap_or(0)),
        count: Some(fresh_data.meditation_count.unwrap_or(0)),
      });
    } else if !daily {
      stats.push(TimeframeStats {
        sum: Some(0),
        count: Some(0),
      });
    }

    Ok(stats)
  }

  pub async fn refresh_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    timeframe: &Timeframe,
  ) -> Result<()> {
    match timeframe {
      Timeframe::Yearly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW yearly_data;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Monthly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW monthly_data;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Weekly => {
        sqlx::query!(
          "
            REFRESH MATERIALIZED VIEW weekly_data;
          "
        )
        .execute(&mut **transaction)
        .await?;
      }
      Timeframe::Daily => {}
    }

    Ok(())
  }

  pub async fn get_star_message(
    transaction: &mut Transaction<'_, Postgres>,
    message_id: &MessageId,
  ) -> Result<Option<StarMessage>> {
    Ok(
      StarMessage::retrieve(*message_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn remove_star_message(
    transaction: &mut Transaction<'_, Postgres>,
    star_message: &str,
  ) -> Result<()> {
    StarMessage::delete_query(GuildId::default(), star_message)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn add_star_message(
    transaction: &mut Transaction<'_, Postgres>,
    star_message: &StarMessage,
  ) -> Result<()> {
    star_message
      .insert_query()
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use anyhow::{Error, Result};
  use chrono::DateTime;
  use poise::serenity_prelude::{GuildId, UserId};
  use sqlx::PgPool;

  use crate::data::bookmark::Bookmark;
  use crate::handlers::database::DatabaseHandler;

  #[sqlx::test(fixtures(path = "fixtures", scripts("bookmarks")))]
  async fn test_get_bookmarks(pool: PgPool) -> Result<(), Error> {
    let handler = DatabaseHandler { pool };
    let mut transaction = handler.start_transaction().await?;
    let bookmarks = DatabaseHandler::get_bookmarks(
      &mut transaction,
      &GuildId::new(123u64),
      &UserId::new(123u64),
    )
    .await?;

    assert_eq!(bookmarks.len(), 4);
    assert_eq!(bookmarks[0].link, "https://foo.bar/1234");
    assert_eq!(bookmarks[0].description, Some("A bar of foo".to_string()));
    assert_eq!(bookmarks[0].id(), "01JBPTWBXJNAKK288S3D89JK7G");
    assert_eq!(
      bookmarks[0].added(),
      DateTime::from_timestamp(1_704_067_200, 0).as_ref()
    );

    assert_eq!(bookmarks[1].link, "https://foo.bar/1235");
    assert_eq!(bookmarks[1].id(), "01JBPTWBXJNAKK288S3D89JK7H");
    assert_eq!(
      bookmarks[1].added(),
      DateTime::from_timestamp(1_704_070_800, 0).as_ref()
    );

    assert_eq!(bookmarks[2].description, None);

    Ok(())
  }

  #[sqlx::test(fixtures(path = "fixtures", scripts("bookmarks")))]
  async fn test_bookmark_count(pool: PgPool) -> Result<(), Error> {
    let handler = DatabaseHandler { pool };
    let mut transaction = handler.start_transaction().await?;
    let count = DatabaseHandler::get_bookmark_count(
      &mut transaction,
      &GuildId::new(123u64),
      &UserId::new(123u64),
    )
    .await?;

    assert_eq!(count, 4);

    Ok(())
  }

  #[sqlx::test(fixtures(path = "fixtures", scripts("bookmarks")))]
  async fn test_remove_bookmark(pool: PgPool) -> Result<(), Error> {
    let handler = DatabaseHandler { pool };
    let mut transaction = handler.start_transaction().await?;
    let count = DatabaseHandler::remove_bookmark(
      &mut transaction,
      &GuildId::new(123u64),
      "01JBPTWBXJNAKK288S3D89JK7J",
    )
    .await?;

    assert_eq!(count, 1);

    let new_count = DatabaseHandler::get_bookmark_count(
      &mut transaction,
      &GuildId::new(123u64),
      &UserId::new(123u64),
    )
    .await?;

    assert_eq!(new_count, 3);

    Ok(())
  }

  #[sqlx::test(fixtures(path = "fixtures", scripts("bookmarks")))]
  async fn test_add_bookmark(pool: PgPool) -> Result<(), Error> {
    let handler = DatabaseHandler { pool };
    let mut transaction = handler.start_transaction().await?;
    () = DatabaseHandler::add_bookmark(
      &mut transaction,
      &Bookmark::new(
        GuildId::new(123u64),
        UserId::new(123u64),
        "https://polyglot.engineer/".to_string(),
        None,
      ),
    )
    .await?;

    let new_count = DatabaseHandler::get_bookmark_count(
      &mut transaction,
      &GuildId::new(123u64),
      &UserId::new(123u64),
    )
    .await?;

    assert_eq!(new_count, 5);

    Ok(())
  }

  #[sqlx::test(fixtures(path = "fixtures", scripts("quote")))]
  async fn test_quote_exists(pool: PgPool) -> Result<(), Error> {
    let handler = DatabaseHandler { pool };
    let mut transaction = handler.start_transaction().await?;

    let guild_id = &GuildId::new(123u64);
    let valid_id = "01JBPTWBXJNAKK288S3D89JK7I";
    let invalid_id = "The time is now";

    assert!(DatabaseHandler::quote_exists(&mut transaction, guild_id, valid_id).await?);
    assert!(!DatabaseHandler::quote_exists(&mut transaction, guild_id, invalid_id).await?);

    DatabaseHandler::remove_quote(&mut transaction, guild_id, valid_id).await?;

    assert!(!DatabaseHandler::quote_exists(&mut transaction, guild_id, valid_id).await?);

    Ok(())
  }
}
