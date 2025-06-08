use std::env;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Datelike, Duration as ChronoDuration};
use chrono::{Timelike, Utc};
use futures::{StreamExt, TryStreamExt, stream::Stream};
use pgvector::Vector;
use poise::serenity_prelude::{GuildId, MessageId, UserId};
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, PgPool, Postgres, Transaction};
use tokio::time;
use tracing::{info, warn};

use crate::commands::helpers::time::{ChallengeTimeframe, Timeframe};
use crate::commands::stats::{BestsType, LeaderboardType, SortBy};
use crate::data::bookmark::Bookmark;
use crate::data::common::{Aggregate, Exists, MaterializedView, Migration, ViewType};
use crate::data::course::Course;
use crate::data::erase::Erase;
use crate::data::meditation::Meditation;
use crate::data::pick_winner;
use crate::data::quote::Quote;
use crate::data::star_message::StarMessage;
use crate::data::stats::{BestData, Bests, BestsOptions, BestsPeriod, ByInterval, Streak, User};
use crate::data::stats::{LeaderboardUser, MeditationCountByDay, Timeframe as TimeframeStats};
use crate::data::steam_key::{Recipient, SteamKey};
use crate::data::term::{Term, VectorSearch};
use crate::data::tracking_profile::TrackingProfile;

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
  #[allow(dead_code)]
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

  pub async fn add_erase(transaction: &mut Transaction<'_, Postgres>, erase: &Erase) -> Result<()> {
    erase.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_erases(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Vec<Erase>> {
    Ok(
      Erase::retrieve_all(*guild_id, *user_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
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

  pub async fn get_user_meditation_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<i64> {
    Ok(
      Meditation::user_sum::<Aggregate>(*guild_id, *user_id)
        .fetch_one(&mut **transaction)
        .await?
        .sum,
    )
  }

  #[allow(dead_code)]
  pub async fn get_user_meditation_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<u64> {
    Ok(
      Meditation::user_count::<Aggregate>(*guild_id, *user_id)
        .fetch_one(&mut **transaction)
        .await?
        .count,
    )
  }

  pub async fn get_guild_meditation_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<i64> {
    Ok(
      Meditation::guild_sum::<Aggregate>(*guild_id)
        .fetch_one(&mut **transaction)
        .await?
        .sum,
    )
  }

  pub async fn get_guild_meditation_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<u64> {
    Ok(
      Meditation::guild_count::<Aggregate>(*guild_id)
        .fetch_one(&mut **transaction)
        .await?
        .count,
    )
  }

  pub fn get_candidates<'a>(
    conn: &'a mut PoolConnection<Postgres>,
    start_date: &'a DateTime<Utc>,
    end_date: &'a DateTime<Utc>,
    guild_id: &'a GuildId,
  ) -> impl Stream<Item = Result<UserId>> + 'a {
    let stream: Pin<Box<dyn Stream<Item = Result<Meditation, SqlxError>> + Send>> =
      pick_winner::retrieve_candidate(*guild_id, start_date, end_date).fetch(&mut **conn);

    stream.map(|row| Ok(row?.user_id))
  }

  pub async fn get_candidate_sum(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
  ) -> Result<i64> {
    Ok(
      pick_winner::candidate_sum::<Aggregate>(*guild_id, *user_id, start_date, end_date)
        .fetch_one(&mut **transaction)
        .await?
        .sum,
    )
  }

  pub async fn get_candidate_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    start_date: &DateTime<Utc>,
    end_date: &DateTime<Utc>,
  ) -> Result<u64> {
    Ok(
      pick_winner::candidate_count::<Aggregate>(*guild_id, *user_id, start_date, end_date)
        .fetch_one(&mut **transaction)
        .await?
        .count,
    )
  }

  pub async fn add_quote(transaction: &mut Transaction<'_, Postgres>, quote: &Quote) -> Result<()> {
    quote.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn update_quote(
    transaction: &mut Transaction<'_, Postgres>,
    quote: &Quote,
  ) -> Result<()> {
    quote.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn remove_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote_id: &str,
  ) -> Result<()> {
    Quote::delete_query(*guild_id, quote_id)
      .execute(&mut **transaction)
      .await?;

    Ok(())
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

  pub async fn get_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    quote_id: &str,
  ) -> Result<Option<Quote>> {
    Ok(
      Quote::retrieve(*guild_id, quote_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_random_quote(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Option<Quote>> {
    Ok(
      Quote::retrieve_random(*guild_id)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_random_quote_with_keyword(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    keyword: &str,
  ) -> Result<Option<Quote>> {
    Ok(
      Quote::retrieve_random_with_keyword(*guild_id, keyword)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_all_quotes(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Quote>> {
    Ok(
      Quote::retrieve_all(*guild_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn search_quotes(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    keyword: &str,
  ) -> Result<Vec<Quote>> {
    Ok(
      Quote::search(*guild_id, keyword)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn update_streak(
    transaction: &mut Transaction<'_, Postgres>,
    streak: &Streak,
  ) -> Result<()> {
    streak.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn get_streak(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Streak> {
    let mut streak_data = Streak::calculate(*guild_id, *user_id)
      .fetch_optional(&mut **transaction)
      .await?
      .unwrap_or_default();

    let mut row = MeditationCountByDay::calculate(*guild_id, *user_id).fetch(&mut **transaction);

    let mut last = 0;
    let mut streak = 0;
    let mut streak_broken = false;

    // Check if currently maintaining a streak
    if let Some(first) = row.try_next().await? {
      let days_ago = first.days_ago;

      if days_ago > 2 {
        streak_broken = true;
        streak_data.current = 0;
      }

      last = days_ago;
      streak = 1;
    }

    // Calculate most recent streak
    while let Some(row) = row.try_next().await? {
      let days_ago = row.days_ago;

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

      let streak = Streak::new(
        *guild_id,
        *user_id,
        streak_data.current,
        streak_data.longest,
      );
      DatabaseHandler::update_streak(transaction, &streak).await?;

      return Ok(streak_data);
    }

    streak_data.longest = if streak < 2 { 0 } else { streak };
    streak = 1;

    // Calculate longest streak (first time only)
    while let Some(row) = row.try_next().await? {
      let days_ago = row.days_ago;

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

    let streak = Streak::new(
      *guild_id,
      *user_id,
      streak_data.current,
      streak_data.longest,
    );
    DatabaseHandler::update_streak(transaction, &streak).await?;

    Ok(streak_data)
  }

  pub async fn add_course(
    transaction: &mut Transaction<'_, Postgres>,
    course: &Course,
  ) -> Result<()> {
    course.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn update_course(
    transaction: &mut Transaction<'_, Postgres>,
    course: &Course,
  ) -> Result<()> {
    course.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn remove_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
  ) -> Result<()> {
    Course::delete_query(*guild_id, course_name)
      .execute(&mut **transaction)
      .await?;

    Ok(())
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

  pub async fn get_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
  ) -> Result<Option<Course>> {
    Ok(
      Course::retrieve(*guild_id, course_name)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_course_in_dm(
    transaction: &mut Transaction<'_, Postgres>,
    course_name: &str,
  ) -> Result<Option<Course>> {
    Ok(
      Course::retrieve_in_dm(course_name)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_possible_course(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    course_name: &str,
    similarity: f32,
  ) -> Result<Option<Course>> {
    Ok(
      Course::retrieve_similar(*guild_id, course_name, similarity)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_all_courses(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Course>> {
    Ok(
      Course::retrieve_all(*guild_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn add_steam_key(
    transaction: &mut Transaction<'_, Postgres>,
    steam_key: &SteamKey,
  ) -> Result<()> {
    steam_key.insert_query().execute(&mut **transaction).await?;

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

  pub async fn add_term(transaction: &mut Transaction<'_, Postgres>, term: &Term) -> Result<()> {
    term.insert_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn update_term(transaction: &mut Transaction<'_, Postgres>, term: &Term) -> Result<()> {
    term.update_query().execute(&mut **transaction).await?;

    Ok(())
  }

  pub async fn update_term_embedding(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
    vector: Option<&Vector>,
  ) -> Result<()> {
    Term::update_embedding(*guild_id, term_name, vector)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn remove_term(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<()> {
    Term::delete_query(*guild_id, term_name)
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

  pub async fn get_term(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    Ok(
      Term::retrieve(*guild_id, term_name)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_term_meaning(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    Ok(
      Term::retrieve_meaning(*guild_id, term_name)
        .fetch_optional(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_term_list(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<Vec<Term>> {
    Ok(
      Term::retrieve_list(*guild_id)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_possible_terms(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    term_name: &str,
    similarity: f32,
  ) -> Result<Vec<Term>> {
    Ok(
      Term::retrieve_similar(*guild_id, term_name, similarity)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn search_terms_by_vector(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    search_vector: &Vector,
    limit: i64,
  ) -> Result<Vec<VectorSearch>> {
    Ok(
      VectorSearch::result(*guild_id, search_vector, limit)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  #[allow(dead_code)]
  pub async fn get_term_count(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<u64> {
    Ok(
      Term::count::<Aggregate>(*guild_id)
        .fetch_one(&mut **transaction)
        .await?
        .count,
    )
  }

  pub async fn get_challenge_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    timeframe: &ChallengeTimeframe,
  ) -> Result<User> {
    // Add 840 minutes to end_time to account for maximum time zone offset.
    let end_time = Utc::now() + ChronoDuration::minutes(840);
    let start_time = match timeframe {
      ChallengeTimeframe::Monthly => Utc::now()
        .with_day(1)
        .with_context(|| "Failed to set day to 1")?
        .with_hour(0)
        .with_context(|| "Failed to set hour to 0")?
        .with_minute(0)
        .with_context(|| "Failed to set minute to 0")?,
      ChallengeTimeframe::YearRound => Utc::now()
        .with_month(1)
        .with_context(|| "Failed to set month to 1")?
        .with_day(1)
        .with_context(|| "Failed to set day to 1")?
        .with_hour(0)
        .with_context(|| "Failed to set hour to 0")?
        .with_minute(0)
        .with_context(|| "Failed to set minute to 0")?,
    };

    let sum_and_count =
      TimeframeStats::user_sum_and_count(*guild_id, *user_id, &start_time, &end_time)
        .fetch_one(&mut **transaction)
        .await?;
    let streak = DatabaseHandler::get_streak(transaction, guild_id, user_id).await?;
    let challenge_stats = User::new(sum_and_count, streak);

    Ok(challenge_stats)
  }

  pub async fn get_leaderboard_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    timeframe: &Timeframe,
    sort_by: &SortBy,
    leaderboard_type: &LeaderboardType,
  ) -> Result<Vec<LeaderboardUser>> {
    Ok(
      LeaderboardUser::stats(*guild_id, timeframe, sort_by, leaderboard_type)
        .fetch_all(&mut **transaction)
        .await?,
    )
  }

  pub async fn get_user_bests_overall(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<Bests> {
    let times_day = BestData::user_times(*guild_id, *user_id, &Timeframe::Daily, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let times_week = BestData::user_times(*guild_id, *user_id, &Timeframe::Weekly, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let times_month = BestData::user_times(*guild_id, *user_id, &Timeframe::Monthly, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let times_year = BestData::user_times(*guild_id, *user_id, &Timeframe::Yearly, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let sessions_day = BestData::user_sessions(*guild_id, *user_id, &Timeframe::Daily, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let sessions_week = BestData::user_sessions(*guild_id, *user_id, &Timeframe::Weekly, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let sessions_month = BestData::user_sessions(*guild_id, *user_id, &Timeframe::Monthly, 1)
      .fetch_optional(&mut **transaction)
      .await?;
    let sessions_year = BestData::user_sessions(*guild_id, *user_id, &Timeframe::Yearly, 1)
      .fetch_optional(&mut **transaction)
      .await?;

    let times = BestsPeriod::new(times_day, times_week, times_month, times_year);
    let sessions = BestsPeriod::new(sessions_day, sessions_week, sessions_month, sessions_year);
    let bests = Bests::new(times, sessions);

    Ok(bests)
  }

  pub async fn get_user_bests(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    options: &BestsOptions,
  ) -> Result<Vec<BestData>> {
    let limit = match options.number {
      LeaderboardType::Top5 => 5,
      LeaderboardType::Top10 => 10,
    };
    let bests = match options.category {
      BestsType::Times => {
        BestData::user_times(*guild_id, *user_id, &options.timeframe, limit)
          .fetch_all(&mut **transaction)
          .await?
      }
      BestsType::Sessions => {
        BestData::user_sessions(*guild_id, *user_id, &options.timeframe, limit)
          .fetch_all(&mut **transaction)
          .await?
      }
      BestsType::Overall => return Err(anyhow!("Overall bests should return an image")),
    };

    Ok(bests)
  }

  pub async fn get_user_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
  ) -> Result<User> {
    let sessions = TimeframeStats::user_total_sum_and_count(*guild_id, *user_id)
      .fetch_one(&mut **transaction)
      .await?;
    let streak = DatabaseHandler::get_streak(transaction, guild_id, user_id).await?;
    let user_stats = User::new(sessions, streak);

    Ok(user_stats)
  }

  pub async fn get_guild_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
  ) -> Result<TimeframeStats> {
    let guild_stats = TimeframeStats::guild_total_sum_and_count(*guild_id)
      .fetch_one(&mut **transaction)
      .await?;

    Ok(guild_stats)
  }

  pub async fn get_user_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    user_id: &UserId,
    timeframe: &Timeframe,
    offset: i16,
  ) -> Result<Vec<TimeframeStats>> {
    let mut fresh_data: Option<ByInterval> = None;
    let now_offset = Utc::now() + ChronoDuration::minutes(offset.into());

    let rows: Vec<ByInterval> = if let Timeframe::Daily = timeframe {
      // Calculate fresh data for last 12 days.
      ByInterval::user_fresh(*guild_id, *user_id, timeframe, &now_offset)
        .fetch_all(&mut **transaction)
        .await?
    } else {
      // Calculate fresh data for present week/month/year.
      fresh_data = ByInterval::user_fresh(*guild_id, *user_id, timeframe, &now_offset)
        .fetch_optional(&mut **transaction)
        .await?;

      // Get data for previous 11 weeks/months/years from materialized view.
      ByInterval::user_from_view(*guild_id, *user_id, timeframe)
        .fetch_all(&mut **transaction)
        .await?
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

        let meditation_minutes = row.map_or(0, |row| row.meditation_minutes.unwrap_or(0));
        let meditation_count = row.map_or(0, |row| row.meditation_count.unwrap_or(0));

        TimeframeStats::new(Some(meditation_minutes), Some(meditation_count))
      })
      .rev()
      .collect();

    if let Some(fresh_data) = fresh_data {
      stats.push(TimeframeStats::new(
        Some(fresh_data.meditation_minutes.unwrap_or(0)),
        Some(fresh_data.meditation_count.unwrap_or(0)),
      ));
    } else if !daily {
      stats.push(TimeframeStats::new(Some(0), Some(0)));
    }

    Ok(stats)
  }

  pub async fn get_guild_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    guild_id: &GuildId,
    timeframe: &Timeframe,
  ) -> Result<Vec<TimeframeStats>> {
    let mut fresh_data: Option<ByInterval> = None;

    let rows: Vec<ByInterval> = if let Timeframe::Daily = timeframe {
      // Calculate fresh data for last 12 days.
      ByInterval::guild_fresh(*guild_id, timeframe)
        .fetch_all(&mut **transaction)
        .await?
    } else {
      // Calculate fresh data for present week/month/year.
      fresh_data = ByInterval::guild_fresh(*guild_id, timeframe)
        .fetch_optional(&mut **transaction)
        .await?;

      // Get data for previous 11 weeks/months/years from materialized view.
      ByInterval::guild_from_view(*guild_id, timeframe)
        .fetch_all(&mut **transaction)
        .await?
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

        let meditation_minutes = row.map_or(0, |row| row.meditation_minutes.unwrap_or(0));
        let meditation_count = row.map_or(0, |row| row.meditation_count.unwrap_or(0));

        TimeframeStats::new(Some(meditation_minutes), Some(meditation_count))
      })
      .rev()
      .collect();

    if let Some(fresh_data) = fresh_data {
      stats.push(TimeframeStats::new(
        Some(fresh_data.meditation_minutes.unwrap_or(0)),
        Some(fresh_data.meditation_count.unwrap_or(0)),
      ));
    } else if !daily {
      stats.push(TimeframeStats::new(Some(0), Some(0)));
    }

    Ok(stats)
  }

  pub async fn refresh_leaderboard(
    transaction: &mut Transaction<'_, Postgres>,
    timeframe: &Timeframe,
  ) -> Result<()> {
    MaterializedView::refresh(&ViewType::Leaderboard, timeframe)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn refresh_chart_stats(
    transaction: &mut Transaction<'_, Postgres>,
    timeframe: &Timeframe,
  ) -> Result<()> {
    MaterializedView::refresh(&ViewType::ChartStats, timeframe)
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

  pub async fn remove_star_message(
    transaction: &mut Transaction<'_, Postgres>,
    star_message: &str,
  ) -> Result<()> {
    StarMessage::delete_query(GuildId::default(), star_message)
      .execute(&mut **transaction)
      .await?;

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
