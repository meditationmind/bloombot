#![allow(
  clippy::needless_raw_string_hashes,
  clippy::fn_params_excessive_bools,
  clippy::struct_excessive_bools,
  clippy::too_many_arguments
)]
#![cfg_attr(any(), rustfmt::skip::macros(query, query_as))]

use crate::pagination::{PageRow, PageType};
use anyhow::{Context, Result};
use chrono::{Datelike, Timelike, Utc};
use futures::{stream::Stream, StreamExt, TryStreamExt};
use log::{info, warn};
use poise::serenity_prelude::{self as serenity, Mentionable};
use ulid::Ulid;

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

pub struct DatabaseHandler {
  pool: sqlx::PgPool,
}

#[derive(Debug)]
pub struct TrackingProfile {
  pub user_id: serenity::UserId,
  pub guild_id: serenity::GuildId,
  pub utc_offset: i16,
  pub anonymous_tracking: bool,
  pub streaks_active: bool,
  pub streaks_private: bool,
  pub stats_private: bool,
}

//Default values for tracking customization
impl Default for TrackingProfile {
  fn default() -> Self {
    Self {
      user_id: serenity::UserId::default(),
      guild_id: serenity::GuildId::default(),
      utc_offset: 0,
      anonymous_tracking: false,
      streaks_active: true,
      streaks_private: false,
      stats_private: false,
    }
  }
}

pub struct Streak {
  pub current: i32,
  pub longest: i32,
}

pub struct UserStats {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: TimeframeStats,
  pub streak: Streak,
}

pub struct GuildStats {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: TimeframeStats,
}

#[derive(poise::ChoiceParameter)]
pub enum Timeframe {
  Yearly,
  Monthly,
  Weekly,
  Daily,
}

#[derive(poise::ChoiceParameter, PartialEq)]
pub enum ChallengeTimeframe {
  #[name = "Monthly Challenge"]
  Monthly,
  #[name = "365-Day Challenge"]
  YearRound,
}

#[derive(Debug)]
pub struct TimeframeStats {
  pub sum: Option<i64>,
  pub count: Option<i64>,
}

pub struct EraseData {
  pub id: String,
  pub user_id: serenity::UserId,
  pub message_link: String,
  pub reason: String,
  pub occurred_at: chrono::DateTime<Utc>,
}

impl PageRow for EraseData {
  fn title(&self, page_type: PageType) -> String {
    match page_type {
      PageType::Standard => {
        if self.occurred_at == (chrono::DateTime::<Utc>::default()) {
          "Date: `Not Available`".to_owned()
        } else {
          format!("Date: `{}`", self.occurred_at.format("%Y-%m-%d %H:%M"))
        }
      }
      PageType::Alternate => {
        if self.occurred_at == (chrono::DateTime::<Utc>::default()) {
          "Date: `Not Available`".to_owned()
        } else {
          format!("Date: `{}`", self.occurred_at.format("%e %B %Y %H:%M"))
        }
      }
    }
  }

  fn body(&self) -> String {
    if self.message_link == "None" {
      format!("**Reason:** {}\n-# Notification not available", self.reason)
    } else {
      format!("**Reason:** {}\n[Go to erase notification]({})", self.reason, self.message_link)
    }
  }
}

pub struct MeditationData {
  pub id: String,
  pub user_id: serenity::UserId,
  pub meditation_minutes: i32,
  pub occurred_at: chrono::DateTime<Utc>,
}

impl PageRow for MeditationData {
  fn title(&self, _page_type: PageType) -> String {
    format!("{} minutes", self.meditation_minutes)
  }

  fn body(&self) -> String {
    let now = chrono::Utc::now();

    if now - self.occurred_at < chrono::Duration::days(1) {
      format!(
        "Date: {}\nID: `{}`",
        chrono_humanize::HumanTime::from(self.occurred_at),
        self.id
      )
    } else {
      format!(
        "Date: `{}`\nID: `{}`",
        self.occurred_at.format("%Y-%m-%d %H:%M"),
        self.id
      )
    }
  }
}

pub struct QuoteData {
  pub id: String,
  pub quote: String,
  pub author: Option<String>,
}

impl PageRow for QuoteData {
  fn title(&self, _page_type: PageType) -> String {
    format!("`ID: {}`", self.id)
  }

  fn body(&self) -> String {
    format!(
      "{}\n― {}",
      self.quote.clone(),
      self.author.clone().unwrap_or("Anonymous".to_owned())
    )
  }
}

pub struct SteamKeyData {
  pub steam_key: String,
  pub used: bool,
  pub reserved: Option<serenity::UserId>,
  pub guild_id: serenity::GuildId,
}

impl PageRow for SteamKeyData {
  fn title(&self, _page_type: PageType) -> String {
    self.steam_key.clone()
  }

  fn body(&self) -> String {
    format!(
      "Used: {}\nReserved for: {}",
      if self.used { "Yes" } else { "No" },
      match self.reserved {
        Some(reserved) => reserved.mention().to_string(),
        None => "Nobody".to_owned(),
      },
    )
  }
}

pub struct SteamKeyRecipientData {
  pub user_id: serenity::UserId,
  pub guild_id: serenity::GuildId,
  pub challenge_prize: Option<bool>,
  pub donator_perk: Option<bool>,
  pub total_keys: i16,
}

impl PageRow for SteamKeyRecipientData {
  fn title(&self, _page_type: PageType) -> String {
    "__Recipient__".to_owned()
  }

  fn body(&self) -> String {
    format!(
      "Name: {}\nDonator Perk: {}\nChallenge Prize: {}\nTotal Keys: {}",
      self.user_id.mention(),
      match self.donator_perk {
        Some(value) =>
          if value {
            "Yes"
          } else {
            "No"
          },
        None => "No",
      },
      match self.challenge_prize {
        Some(value) =>
          if value {
            "Yes"
          } else {
            "No"
          },
        None => "No",
      },
      self.total_keys,
    )
  }
}

pub struct BookmarkData {
  pub id: String,
  pub link: String,
  pub description: Option<String>,
  pub added: chrono::DateTime<Utc>,
}

impl PageRow for BookmarkData {
  fn title(&self, _page_type: PageType) -> String {
    self.link.clone()
  }

  fn body(&self) -> String {
    if let Some(description) = &self.description {
      format!(
        "> {}\n> -# Added: <t:{}:f>\n> -# ID: [{}](discord://{} \"For copying a bookmark ID on mobile. Not a working link.\")\n** **",
        description,
        self.added.timestamp(),
        self.id,
        self.id,
      )
    } else {
      format!(
        "> -# Added: <t:{}:f>\n> -# ID: [{}](discord://{} \"For copying a bookmark ID on mobile. Not a working link.\")\n** **",
        self.added.timestamp(),
        self.id,
        self.id,
      )
    }
  }
}

pub struct CourseData {
  pub course_name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
}

impl PageRow for CourseData {
  fn title(&self, _page_type: PageType) -> String {
    self.course_name.clone()
  }

  fn body(&self) -> String {
    format!(
      "Participants: {}\nGraduates: {}",
      self.participant_role.mention(),
      self.graduate_role.mention()
    )
  }
}

pub struct ExtendedCourseData {
  pub course_name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
  pub guild_id: serenity::GuildId,
}

#[derive(Debug)]
pub struct Term {
  pub id: String,
  pub name: String,
  pub meaning: String,
  pub usage: Option<String>,
  pub links: Option<Vec<String>>,
  pub category: Option<String>,
  pub aliases: Option<Vec<String>>,
}

impl PageRow for Term {
  fn title(&self, _page_type: PageType) -> String {
    format!("__{}__", self.name.clone())
  }

  fn body(&self) -> String {
    /*let meaning = match self.meaning.len() > 157 {
      true => {
        let truncate = self.meaning.chars().take(157).collect::<String>();
        let truncate_split = match truncate.rsplit_once(' ') {
          Some(pair) => pair.0.to_string(),
          None => truncate
        };
        let truncate_final = if truncate_split.chars().last().unwrap().is_ascii_punctuation() {
          truncate_split.chars().take(truncate_split.chars().count() - 1).collect::<String>()
        } else {
          truncate_split
        };
        format!("{}...", truncate_final)
      },
      false => self.meaning.clone(),
    };
    meaning*/
    self.meaning.clone()
  }
}

#[derive(Debug, sqlx::FromRow)]
pub struct TermSearchResult {
  pub term_name: String,
  pub meaning: String,
  pub distance_score: Option<f64>,
}

#[derive(Debug)]
pub struct TermNames {
  pub term_name: String,
  pub aliases: Option<Vec<String>>,
}

#[allow(clippy::struct_field_names)]
pub struct StarMessage {
  pub record_id: String,
  pub starred_message_id: serenity::MessageId,
  pub board_message_id: serenity::MessageId,
  pub starred_channel_id: serenity::ChannelId,
}

impl DatabaseHandler {
  pub async fn new() -> Result<Self> {
    let database_url =
      std::env::var("DATABASE_URL").with_context(|| "Missing DATABASE_URL environment variable")?;
    // let pool = sqlx::PgPool::connect(&database_url).await?;
    let max_retries = 5;
    let mut attempts = 0;

    loop {
      let pool = match sqlx::PgPool::connect(&database_url).await {
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
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            continue;
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e.into());
        }
      };

      sqlx::migrate!("./migrations").run(&pool).await?;

      info!("Successfully applied migrations.");

      return Ok(Self { pool });
    }
  }

  pub async fn get_connection(&self) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>> {
    Ok(self.pool.acquire().await?)
  }

  pub async fn get_connection_with_retry(
    &self,
    max_retries: usize,
  ) -> Result<sqlx::pool::PoolConnection<sqlx::Postgres>> {
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
              tokio::time::sleep(std::time::Duration::from_secs(1)).await;
              continue;
            }
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e);
        }
      }
    }
  }

  pub async fn start_transaction(&self) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
    Ok(self.pool.begin().await?)
  }

  pub async fn start_transaction_with_retry(
    &self,
    max_retries: usize,
  ) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
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
              tokio::time::sleep(std::time::Duration::from_secs(1)).await;
              continue;
            }
          }

          // If it's a different kind of error, we might want to return it immediately
          return Err(e);
        }
      }
    }
  }

  pub async fn commit_transaction(
    transaction: sqlx::Transaction<'_, sqlx::Postgres>,
  ) -> Result<()> {
    transaction.commit().await?;
    Ok(())
  }

  /// This function is not technically necessary, as the transaction will be rolled back when dropped.
  /// However, for readability, it is recommended to call this function when you want to rollback a transaction.
  pub async fn rollback_transaction(
    transaction: sqlx::Transaction<'_, sqlx::Postgres>,
  ) -> Result<()> {
    transaction.rollback().await?;
    Ok(())
  }

  pub async fn create_tracking_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    utc_offset: i16,
    anonymous_tracking: bool,
    streaks_active: bool,
    streaks_private: bool,
    stats_private: bool,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO tracking_profile (record_id, user_id, guild_id, utc_offset, anonymous_tracking, streaks_active, streaks_private, stats_private) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      guild_id.to_string(),
      utc_offset,
      anonymous_tracking,
      streaks_active,
      streaks_private,
      stats_private,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn update_tracking_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    utc_offset: i16,
    anonymous_tracking: bool,
    streaks_active: bool,
    streaks_private: bool,
    stats_private: bool,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE tracking_profile SET utc_offset = $1, anonymous_tracking = $2, streaks_active = $3, streaks_private = $4, stats_private = $5 WHERE user_id = $6 AND guild_id = $7
      "#,
      utc_offset,
      anonymous_tracking,
      streaks_active,
      streaks_private,
      stats_private,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn remove_tracking_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM tracking_profile WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn migrate_tracking_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    old_user_id: &serenity::UserId,
    new_user_id: &serenity::UserId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE tracking_profile SET user_id = $3 WHERE user_id = $1 AND guild_id = $2
      "#,
      old_user_id.to_string(),
      guild_id.to_string(),
      new_user_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_tracking_profile(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Option<TrackingProfile>> {
    let row = sqlx::query!(
      r#"
        SELECT user_id, guild_id, utc_offset, anonymous_tracking, streaks_active, streaks_private, stats_private FROM tracking_profile WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let tracking_profile = match row {
      Some(row) => Some(TrackingProfile {
        user_id: serenity::UserId::new(row.user_id.parse::<u64>()?),
        guild_id: serenity::GuildId::new(row.guild_id.parse::<u64>()?),
        utc_offset: row.utc_offset,
        anonymous_tracking: row.anonymous_tracking,
        streaks_active: row.streaks_active,
        streaks_private: row.streaks_private,
        stats_private: row.stats_private,
      }),
      None => None,
    };

    Ok(tracking_profile)
  }

  pub async fn add_steamkey_recipient(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    challenge_prize: Option<bool>,
    donator_perk: Option<bool>,
    total_keys: i16,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO steamkey_recipients (record_id, user_id, guild_id, challenge_prize, donator_perk, total_keys) VALUES ($1, $2, $3, $4, $5, $6)
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      guild_id.to_string(),
      challenge_prize,
      donator_perk,
      total_keys
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn update_steamkey_recipient(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    challenge_prize: Option<bool>,
    donator_perk: Option<bool>,
    total_keys: i16,
  ) -> Result<()> {
    sqlx::query!(
      r#"
      UPDATE steamkey_recipients SET challenge_prize = $1, donator_perk = $2, total_keys = $3 WHERE user_id = $4 AND guild_id = $5
      "#,
      challenge_prize,
      donator_perk,
      total_keys,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn remove_steamkey_recipient(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM steamkey_recipients WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_steamkey_recipient(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Option<SteamKeyRecipientData>> {
    let row = sqlx::query!(
      r#"
        SELECT user_id, guild_id, challenge_prize, donator_perk, total_keys FROM steamkey_recipients WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let steamkey_recipient = match row {
      Some(row) => Some(SteamKeyRecipientData {
        user_id: serenity::UserId::new(row.user_id.parse::<u64>()?),
        guild_id: serenity::GuildId::new(row.guild_id.parse::<u64>()?),
        challenge_prize: row.challenge_prize,
        donator_perk: row.donator_perk,
        total_keys: row.total_keys,
      }),
      None => None,
    };

    Ok(steamkey_recipient)
  }

  pub async fn get_steamkey_recipients(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<SteamKeyRecipientData>> {
    let rows = sqlx::query!(
      r#"
        SELECT user_id, guild_id, challenge_prize, donator_perk, total_keys FROM steamkey_recipients WHERE guild_id = $1
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let steamkey_recipients = rows
      .into_iter()
      .map(|row| SteamKeyRecipientData {
        user_id: serenity::UserId::new(
          row
            .user_id
            .parse::<u64>()
            .expect("parse should not fail since user_id is UserId.to_string()"),
        ),
        guild_id: serenity::GuildId::new(
          row
            .guild_id
            .parse::<u64>()
            .expect("parse should not fail since guild_id is GuildId.to_string()"),
        ),
        challenge_prize: row.challenge_prize,
        donator_perk: row.donator_perk,
        total_keys: row.total_keys,
      })
      .collect();

    Ok(steamkey_recipients)
  }

  pub async fn steamkey_recipient_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM steamkey_recipients WHERE guild_id = $1 AND user_id = $2)
      "#,
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn record_steamkey_receipt(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<()> {
    let possible_record = sqlx::query!(
      r#"
        SELECT total_keys FROM steamkey_recipients WHERE guild_id = $1 AND user_id = $2
      "#,
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_optional(&mut **connection)
    .await?;

    match possible_record {
      Some(existing_record) => {
        let updated_keys = existing_record.total_keys + 1;
        sqlx::query!(
          r#"
          UPDATE steamkey_recipients SET challenge_prize = TRUE, total_keys = $1 WHERE user_id = $2 AND guild_id = $3
          "#,
          updated_keys,
          user_id.to_string(),
          guild_id.to_string(),
        )
        .execute(&mut **connection)
        .await?;
      }
      None => {
        sqlx::query!(
          r#"
            INSERT INTO steamkey_recipients (record_id, user_id, guild_id, challenge_prize, total_keys) VALUES ($1, $2, $3, TRUE, 1)
          "#,
          Ulid::new().to_string(),
          user_id.to_string(),
          guild_id.to_string(),
        )
        .execute(&mut **connection)
        .await?;
      }
    }

    Ok(())
  }

  pub async fn add_bookmark(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    message_link: &str,
    description: Option<&str>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc) VALUES ($1, $2, $3, $4, $5)
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      guild_id.to_string(),
      message_link,
      description,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_bookmark_count(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS bookmark_count FROM bookmarks WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let bookmark_count = row
      .bookmark_count
      .with_context(|| "Failed to assign bookmark_count computed by DB query")?;

    Ok(bookmark_count.try_into()?)
  }

  pub async fn get_bookmarks(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Vec<BookmarkData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, message_link, user_desc, occurred_at FROM bookmarks WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at ASC
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let bookmark_data = rows
      .into_iter()
      .map(|row| BookmarkData {
        id: row.record_id,
        link: row.message_link,
        description: row.user_desc,
        added: row.occurred_at,
      })
      .collect();

    Ok(bookmark_data)
  }

  pub async fn search_bookmarks(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    keyword: &str,
  ) -> Result<Vec<BookmarkData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, message_link, user_desc, occurred_at,
        ts_rank(desc_tsv, websearch_to_tsquery('english', $3)) AS rank
        FROM bookmarks
        WHERE user_id = $1 AND guild_id = $2
        AND (desc_tsv @@ websearch_to_tsquery('english', $3))
        ORDER BY rank DESC
      "#,
      user_id.to_string(),
      guild_id.to_string(),
      keyword.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let bookmark_data = rows
      .into_iter()
      .map(|row| BookmarkData {
        id: row.record_id,
        link: row.message_link,
        description: row.user_desc,
        added: row.occurred_at,
      })
      .collect();

    Ok(bookmark_data)
  }

  pub async fn remove_bookmark(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bookmark_id: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM bookmarks WHERE record_id = $1
      "#,
      bookmark_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn add_erase(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    message_link: &str,
    reason: Option<&str>,
    occurred_at: chrono::DateTime<Utc>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO erases (record_id, user_id, guild_id, message_link, reason, occurred_at) VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (message_link) DO UPDATE SET reason = $5
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      guild_id.to_string(),
      message_link,
      reason,
      occurred_at,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_erases(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Vec<EraseData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, user_id, message_link, reason, occurred_at FROM erases WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let erase_data = rows
      .into_iter()
      .map(|row| EraseData {
        id: row.record_id,
        user_id: serenity::UserId::new(
          row
            .user_id
            .parse::<u64>()
            .expect("parse should not fail since user_id is UserId.to_string()"),
        ),
        message_link: row.message_link.unwrap_or(String::from("None")),
        reason: row.reason.unwrap_or(String::from("No reason provided.")),
        occurred_at: row.occurred_at.unwrap_or_default(),
      })
      .collect();

    Ok(erase_data)
  }

  pub async fn add_minutes(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    minutes: i32,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO meditation (record_id, user_id, meditation_minutes, guild_id) VALUES ($1, $2, $3, $4)
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      minutes,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn create_meditation_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    minutes: i32,
    occurred_at: chrono::DateTime<Utc>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO meditation (record_id, user_id, meditation_minutes, guild_id, occurred_at) VALUES ($1, $2, $3, $4, $5)
      "#,
      Ulid::new().to_string(),
      user_id.to_string(),
      minutes,
      guild_id.to_string(),
      occurred_at,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_user_meditation_entries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Vec<MeditationData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, user_id, meditation_minutes, occurred_at FROM meditation WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let meditation_entries = rows
      .into_iter()
      .map(|row| MeditationData {
        id: row.record_id,
        user_id: serenity::UserId::new(
          row
            .user_id
            .parse::<u64>()
            .expect("parse should not fail since user_id is UserId.to_string()"),
        ),
        meditation_minutes: row.meditation_minutes,
        occurred_at: row.occurred_at,
      })
      .collect();

    Ok(meditation_entries)
  }

  pub async fn get_meditation_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    meditation_id: &str,
  ) -> Result<Option<MeditationData>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, user_id, meditation_minutes, occurred_at FROM meditation WHERE record_id = $1 AND guild_id = $2
      "#,
      meditation_id,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let meditation_entry = match row {
      Some(row) => Some(MeditationData {
        id: row.record_id,
        user_id: serenity::UserId::new(row.user_id.parse::<u64>()?),
        meditation_minutes: row.meditation_minutes,
        occurred_at: row.occurred_at,
      }),
      None => None,
    };

    Ok(meditation_entry)
  }

  pub async fn update_meditation_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    meditation_id: &str,
    minutes: i32,
    occurred_at: chrono::DateTime<Utc>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE meditation SET meditation_minutes = $1, occurred_at = $2 WHERE record_id = $3
      "#,
      minutes,
      occurred_at,
      meditation_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn delete_meditation_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    meditation_id: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM meditation WHERE record_id = $1
      "#,
      meditation_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn reset_user_meditation_entries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM meditation WHERE user_id = $1 AND guild_id = $2
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn migrate_meditation_entries(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    old_user_id: &serenity::UserId,
    new_user_id: &serenity::UserId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE meditation SET user_id = $3 WHERE user_id = $1 AND guild_id = $2
      "#,
      old_user_id.to_string(),
      guild_id.to_string(),
      new_user_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub fn get_winner_candidates<'a>(
    conn: &'a mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    start_date: chrono::DateTime<Utc>,
    end_date: chrono::DateTime<Utc>,
    guild_id: &'a serenity::GuildId,
  ) -> impl Stream<Item = Result<serenity::UserId>> + 'a {
    // All entries that are greater than 0 minutes and within the start and end date
    // We only want a user ID to show up once, so we group by user ID and sum the meditation minutes
    let rows_stream = sqlx::query!(
      r#"
        SELECT user_id FROM meditation WHERE meditation_minutes > 0 AND occurred_at >= $1 AND occurred_at <= $2 AND guild_id = $3 GROUP BY user_id ORDER BY RANDOM()
      "#,
      start_date,
      end_date,
      guild_id.to_string(),
    ).fetch(&mut **conn);

    rows_stream.map(|row| {
      let row = row?;

      let user_id = serenity::UserId::new(row.user_id.parse::<u64>()?);

      Ok(user_id)
    })
  }

  pub async fn get_winner_candidate_meditation_sum(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    start_date: chrono::DateTime<Utc>,
    end_date: chrono::DateTime<Utc>,
  ) -> Result<i64> {
    let row = sqlx::query!(
      r#"
        SELECT SUM(meditation_minutes) AS winner_candidate_total FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    start_date: chrono::DateTime<Utc>,
    end_date: chrono::DateTime<Utc>,
  ) -> Result<u64> {
    let row = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS winner_candidate_total FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<i64> {
    let row = sqlx::query!(
      r#"
        SELECT SUM(meditation_minutes) AS user_total FROM meditation WHERE user_id = $1 AND guild_id = $2
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS user_total FROM meditation WHERE user_id = $1 AND guild_id = $2
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<i64> {
    let row = sqlx::query!(
      r#"
        SELECT SUM(meditation_minutes) AS guild_total FROM meditation WHERE guild_id = $1
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS guild_total FROM meditation WHERE guild_id = $1
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<QuoteData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, quote, author FROM quote WHERE guild_id = $1
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let quotes = rows
      .into_iter()
      .map(|row| QuoteData {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      })
      .collect();

    Ok(quotes)
  }

  pub async fn search_quotes(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    keyword: &str,
  ) -> Result<Vec<QuoteData>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, quote, author
        FROM quote
        WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2))
      "#,
      guild_id.to_string(),
      keyword.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let quotes = rows
      .into_iter()
      .map(|row| QuoteData {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      })
      .collect();

    Ok(quotes)
  }

  pub async fn get_quote(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    quote_id: &str,
  ) -> Result<Option<QuoteData>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, quote, author FROM quote WHERE record_id = $1 AND guild_id = $2
      "#,
      quote_id,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(QuoteData {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn edit_quote(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    quote_id: &str,
    quote: &str,
    author: Option<&str>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE quote SET quote = $1, author = $2 WHERE record_id = $3
      "#,
      quote,
      author,
      quote_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_random_motivation(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Option<String>> {
    let row = sqlx::query!(
      r#"
        SELECT quote FROM quote WHERE guild_id = $1 ORDER BY RANDOM() LIMIT 1
      "#,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(row.map(|row| row.quote))
  }

  pub async fn update_streak(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    current: i32,
    longest: i32,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO streak (record_id, user_id, guild_id, current_streak, longest_streak) VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (user_id) DO UPDATE SET current_streak = $4, longest_streak = $5
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Streak> {
    let mut streak_data = sqlx::query_as!(
      Streak,
      r#"
        SELECT current_streak AS current, longest_streak AS longest FROM streak WHERE guild_id = $1 AND user_id = $2
      "#,
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?
    .unwrap_or(Streak { current: 0, longest: 0 });

    let mut row = sqlx::query_as!(
      MeditationCountByDay,
      r#"
      WITH cte AS (
        SELECT date_part('day', NOW() - DATE_TRUNC('day', "occurred_at")) AS "days_ago"
        FROM meditation 
        WHERE user_id = $1 AND guild_id = $2
        AND "occurred_at"::date <= NOW()::date
      )
      SELECT "days_ago"
      FROM cte
      GROUP BY "days_ago"
      ORDER BY "days_ago" ASC;
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    course_name: &str,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM course WHERE course_name = $1 AND guild_id = $2)
      "#,
      course_name,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn add_course(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    course_name: &str,
    participant_role: &serenity::Role,
    graduate_role: &serenity::Role,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO course (record_id, course_name, participant_role, graduate_role, guild_id) VALUES ($1, $2, $3, $4, $5)
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_name: &str,
    participant_role: String,
    graduate_role: String,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE course SET participant_role = $1, graduate_role = $2 WHERE LOWER(course_name) = LOWER($3)
      "#,
      participant_role,
      graduate_role,
      course_name,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn steam_key_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    key: &str,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM steamkey WHERE steam_key = $1 AND guild_id = $2)
      "#,
      key,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn add_steam_key(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    key: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO steamkey (record_id, steam_key, guild_id, used) VALUES ($1, $2, $3, $4)
      "#,
      Ulid::new().to_string(),
      key,
      guild_id.to_string(),
      false,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_all_steam_keys(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<SteamKeyData>> {
    let rows = sqlx::query!(
      r#"
        SELECT steam_key, reserved, used, guild_id FROM steamkey WHERE guild_id = $1
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let steam_keys = rows
      .into_iter()
      .map(|row| SteamKeyData {
        steam_key: row.steam_key,
        reserved: row.reserved.map(|reserved| {
          serenity::UserId::new(
            reserved
              .parse::<u64>()
              .expect("parse should not fail since reserved is UserId.to_string()"),
          )
        }),
        used: row.used,
        guild_id: serenity::GuildId::new(
          row
            .guild_id
            .parse::<u64>()
            .expect("parse should not fail since guild_id is GuildId.to_string()"),
        ),
      })
      .collect();

    Ok(steam_keys)
  }

  pub async fn add_quote(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    quote: &str,
    author: Option<&str>,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO quote (record_id, quote, author, guild_id) VALUES ($1, $2, $3, $4)
      "#,
      Ulid::new().to_string(),
      quote,
      author,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn add_term(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    term_name: &str,
    meaning: &str,
    usage: Option<&str>,
    links: &[String],
    category: Option<&str>,
    aliases: &[String],
    guild_id: &serenity::GuildId,
    vector: pgvector::Vector,
  ) -> Result<()> {
    sqlx::query(
      r#"
        INSERT INTO term (record_id, term_name, meaning, usage, links, category, aliases, guild_id, embedding) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
      "#)
      .bind(Ulid::new().to_string())
      .bind(term_name)
      .bind(meaning)
      .bind(usage)
      .bind(links)
      .bind(category)
      .bind(aliases)
      .bind(guild_id.to_string())
      .bind(vector)
      .execute(&mut **transaction)
      .await?;

    Ok(())
  }

  pub async fn search_terms_by_vector(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    search_vector: pgvector::Vector,
    limit: usize,
  ) -> Result<Vec<TermSearchResult>> {
    // For some reason, pgvector wants a vector to look like a string [1,2,3] instead of an array.
    // I'm sorry for what you are about to see.
    // let pgvector_format = format!("{:?}", search_vector);

    // limit should be a small integer
    #[allow(clippy::cast_possible_wrap)]
    let terms: Vec<TermSearchResult> = sqlx::query_as(
      r#"
        SELECT term_name, meaning, embedding <=> $1 AS distance_score
        FROM term
        WHERE guild_id = $2
        ORDER BY distance_score ASC
        LIMIT $3
      "#,
    )
    .bind(search_vector)
    .bind(guild_id.to_string())
    .bind(limit as i64)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(terms)
  }

  pub async fn get_term(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, term_name, meaning, usage, links, category, aliases
        FROM term
        WHERE guild_id = $2
        AND (LOWER(term_name) = LOWER($1)) OR (f_textarr2text(aliases) ~* ('(?:^|,)' || $1 || '(?:$|,)'))
      "#,
      term_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let term = match row {
      Some(row) => Some(Term {
        id: row.record_id,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    term_name: &str,
  ) -> Result<Option<Term>> {
    let row = sqlx::query!(
      r#"
        SELECT meaning
        FROM term
        WHERE guild_id = $2
        AND (LOWER(term_name) = LOWER($1))
      "#,
      term_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let term = match row {
      Some(row) => Some(Term {
        id: String::new(),
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

  pub async fn edit_term(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    original_id: &str,
    meaning: &str,
    usage: Option<&str>,
    links: &[String],
    category: Option<&str>,
    aliases: &[String],
    vector: Option<pgvector::Vector>,
  ) -> Result<()> {
    sqlx::query(
      r#"
        UPDATE term
        SET meaning = $1, usage = $2, links = $3, category = $4, aliases = $5, embedding = COALESCE($6, embedding)
        WHERE record_id = $7
      "#,
    )
    .bind(meaning)
    .bind(usage)
    .bind(links)
    .bind(category)
    .bind(aliases)
    .bind(vector)
    .bind(original_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn edit_term_embedding(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    term_name: &str,
    vector: Option<pgvector::Vector>,
  ) -> Result<()> {
    sqlx::query(
      r#"
        UPDATE term
        SET embedding = $3
        WHERE guild_id = $1
        AND (LOWER(term_name) = LOWER($2))
      "#,
    )
    .bind(guild_id.to_string())
    .bind(term_name)
    .bind(vector)
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_all_courses(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<CourseData>> {
    let rows = sqlx::query!(
      r#"
        SELECT course_name, participant_role, graduate_role
        FROM course
        WHERE guild_id = $1
        ORDER BY course_name ASC
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    #[allow(clippy::expect_used)]
    let courses = rows
      .into_iter()
      .map(|row| CourseData {
        course_name: row.course_name,
        participant_role: serenity::RoleId::new(
          row
            .participant_role
            .parse::<u64>()
            .expect("parse should not fail since participant_role is RoleId.to_string()"),
        ),
        graduate_role: serenity::RoleId::new(
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    course_name: &str,
  ) -> Result<Option<CourseData>> {
    let row = sqlx::query!(
      r#"
        SELECT course_name, participant_role, graduate_role
        FROM course
        WHERE LOWER(course_name) = LOWER($1) AND guild_id = $2
      "#,
      course_name,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let course_data = match row {
      Some(row) => Some(CourseData {
        course_name: row.course_name,
        participant_role: serenity::RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: serenity::RoleId::new(row.graduate_role.parse::<u64>()?),
      }),
      None => None,
    };

    Ok(course_data)
  }

  pub async fn get_course_in_dm(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    course_name: &str,
  ) -> Result<Option<ExtendedCourseData>> {
    let row = sqlx::query!(
      r#"
        SELECT course_name, participant_role, graduate_role, guild_id
        FROM course
        WHERE LOWER(course_name) = LOWER($1)
      "#,
      course_name,
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let extended_course_data = match row {
      Some(row) => Some(ExtendedCourseData {
        course_name: row.course_name,
        participant_role: serenity::RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: serenity::RoleId::new(row.graduate_role.parse::<u64>()?),
        guild_id: serenity::GuildId::new(
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    course_name: &str,
    similarity: f32,
  ) -> Result<Option<CourseData>> {
    let row = sqlx::query!(
      r#"
        SELECT course_name, participant_role, graduate_role, SET_LIMIT($2), SIMILARITY(LOWER(course_name), LOWER($1)) AS similarity_score
        FROM course
        WHERE LOWER(course_name) % LOWER($1) AND guild_id = $3
        ORDER BY similarity_score DESC
        LIMIT 1
      "#,
      course_name,
      similarity,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let course_data = match row {
      Some(row) => Some(CourseData {
        course_name: row.course_name,
        participant_role: serenity::RoleId::new(row.participant_role.parse::<u64>()?),
        graduate_role: serenity::RoleId::new(row.graduate_role.parse::<u64>()?),
      }),
      None => None,
    };

    Ok(course_data)
  }

  pub async fn get_possible_terms(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    term_name: &str,
    similarity: f32,
  ) -> Result<Vec<Term>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, term_name, meaning, usage, links, category, aliases, SET_LIMIT($2), SIMILARITY(LOWER(term_name), LOWER($1)) AS similarity_score
        FROM term
        WHERE guild_id = $3
        AND (LOWER(term_name) % LOWER($1)) OR (f_textarr2text(aliases) ILIKE '%' || $1 || '%')
        ORDER BY similarity_score DESC
        LIMIT 5
      "#,
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
          id: row.record_id,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<u64> {
    let row = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS term_count FROM term WHERE guild_id = $1
      "#,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<TermNames>> {
    let rows = sqlx::query!(
      r#"
        SELECT term_name, aliases
        FROM term
        WHERE guild_id = $1
        ORDER BY term_name DESC
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let term_list = rows
      .into_iter()
      .map(|row| TermNames {
        term_name: row.term_name,
        aliases: row.aliases,
      })
      .collect();

    Ok(term_list)
  }

  pub async fn get_all_glossary_terms(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Vec<Term>> {
    let rows = sqlx::query!(
      r#"
        SELECT record_id, term_name, meaning
        FROM term
        WHERE guild_id = $1
        ORDER BY term_name ASC
      "#,
      guild_id.to_string(),
    )
    .fetch_all(&mut **transaction)
    .await?;

    let glossary = rows
      .into_iter()
      .map(|row| Term {
        id: row.record_id,
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
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $1)
      "#,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn reserve_key(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
  ) -> Result<Option<String>> {
    let row = sqlx::query!(
      r#"
        UPDATE steamkey SET reserved = $1 WHERE steam_key = (SELECT steam_key FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $2 ORDER BY RANDOM() LIMIT 1) RETURNING steam_key
      "#,
      user_id.to_string(),
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(row.map(|row| row.steam_key))
  }

  pub async fn unreserve_key(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    key: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE steamkey SET reserved = NULL WHERE steam_key = $1
      "#,
      key,
    )
    .execute(&mut **connection)
    .await?;

    Ok(())
  }

  pub async fn mark_key_used(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    key: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        UPDATE steamkey SET used = TRUE WHERE steam_key = $1
      "#,
      key,
    )
    .execute(&mut **connection)
    .await?;

    Ok(())
  }

  pub async fn get_key_and_mark_used(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Option<String>> {
    let row = sqlx::query!(
      r#"
        UPDATE steamkey SET used = TRUE WHERE steam_key = (SELECT steam_key FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $1 ORDER BY RANDOM() LIMIT 1) RETURNING steam_key
      "#,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    Ok(row.map(|row| row.steam_key))
  }

  pub async fn get_random_quote(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
  ) -> Result<Option<QuoteData>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, quote, author FROM quote WHERE guild_id = $1 ORDER BY RANDOM() LIMIT 1
      "#,
      guild_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(QuoteData {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn get_random_quote_with_keyword(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    keyword: &str,
  ) -> Result<Option<QuoteData>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, quote, author
        FROM quote
        WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2))
        ORDER BY RANDOM()
        LIMIT 1
      "#,
      guild_id.to_string(),
      keyword.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let quote = match row {
      Some(row) => Some(QuoteData {
        id: row.record_id,
        quote: row.quote,
        author: row.author,
      }),
      None => None,
    };

    Ok(quote)
  }

  pub async fn remove_course(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    course_name: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM course WHERE course_name = $1 AND guild_id = $2
      "#,
      course_name,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn remove_steam_key(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    key: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM steamkey WHERE steam_key = $1 AND guild_id = $2
      "#,
      key,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn remove_quote(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    quote: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM quote WHERE record_id = $1 AND guild_id = $2
      "#,
      quote,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn term_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    term_name: &str,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM term WHERE term_name = $1 AND guild_id = $2)
      "#,
      term_name,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn remove_term(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    term_name: &str,
    guild_id: &serenity::GuildId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM term WHERE term_name = $1 AND guild_id = $2
      "#,
      term_name,
      guild_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn get_challenge_stats(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    timeframe: &ChallengeTimeframe,
  ) -> Result<UserStats> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = chrono::Utc::now();
    let start_time = match timeframe {
      ChallengeTimeframe::Monthly => end_time
        .with_day(1)
        .unwrap_or_default()
        .with_hour(0)
        .unwrap_or_default()
        .with_minute(0)
        .unwrap_or_default(),
      ChallengeTimeframe::YearRound => end_time
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
      r#"
        SELECT COUNT(record_id) AS count, SUM(meditation_minutes) AS sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      "#,
      guild_id.to_string(),
      user_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_stats = UserStats {
      all_minutes: 0,
      all_count: 0,
      timeframe_stats: timeframe_data,
      streak: DatabaseHandler::get_streak(transaction, guild_id, user_id).await?,
    };

    Ok(user_stats)
  }

  pub async fn get_user_stats(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    timeframe: &Timeframe,
  ) -> Result<UserStats> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = chrono::Utc::now();
    let start_time = match timeframe {
      Timeframe::Daily => end_time - chrono::Duration::days(12),
      Timeframe::Weekly => end_time - chrono::Duration::weeks(12),
      Timeframe::Monthly => end_time - chrono::Duration::days(30 * 12),
      Timeframe::Yearly => end_time - chrono::Duration::days(365 * 12),
    };

    let total_data = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS total_count, SUM(meditation_minutes) AS total_sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2
      "#,
      guild_id.to_string(),
      user_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let timeframe_data = sqlx::query_as!(
      TimeframeStats,
      r#"
        SELECT COUNT(record_id) AS count, SUM(meditation_minutes) AS sum
        FROM meditation
        WHERE guild_id = $1 AND user_id = $2 AND occurred_at >= $3 AND occurred_at <= $4
      "#,
      guild_id.to_string(),
      user_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let user_stats = UserStats {
      all_minutes: total_data.total_sum.unwrap_or(0),
      all_count: total_data.total_count.unwrap_or(0).try_into()?,
      timeframe_stats: timeframe_data,
      streak: DatabaseHandler::get_streak(transaction, guild_id, user_id).await?,
    };

    Ok(user_stats)
  }

  pub async fn get_guild_stats(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    timeframe: &Timeframe,
  ) -> Result<GuildStats> {
    // Get total count, total sum, and count/sum for timeframe
    let end_time = chrono::Utc::now();
    let start_time = match timeframe {
      Timeframe::Daily => end_time - chrono::Duration::days(12),
      Timeframe::Weekly => end_time - chrono::Duration::weeks(12),
      Timeframe::Monthly => end_time - chrono::Duration::days(30 * 12),
      Timeframe::Yearly => end_time - chrono::Duration::days(365 * 12),
    };

    let total_data = sqlx::query!(
      r#"
        SELECT COUNT(record_id) AS total_count, SUM(meditation_minutes) AS total_sum
        FROM meditation
        WHERE guild_id = $1
      "#,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    let timeframe_data = sqlx::query_as!(
      TimeframeStats,
      r#"
        SELECT COUNT(record_id) AS count, SUM(meditation_minutes) AS sum
        FROM meditation
        WHERE guild_id = $1 AND occurred_at >= $2 AND occurred_at <= $3
      "#,
      guild_id.to_string(),
      start_time,
      end_time,
    )
    .fetch_one(&mut **transaction)
    .await?;

    let guild_stats = GuildStats {
      all_minutes: total_data.total_sum.unwrap_or(0),
      all_count: total_data.total_count.unwrap_or(0).try_into()?,
      timeframe_stats: timeframe_data,
    };

    Ok(guild_stats)
  }

  pub async fn quote_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    quote_id: &str,
  ) -> Result<bool> {
    let row = sqlx::query!(
      r#"
        SELECT EXISTS(SELECT 1 FROM quote WHERE record_id = $1 AND guild_id = $2)
      "#,
      quote_id,
      guild_id.to_string(),
    )
    .fetch_one(&mut **transaction)
    .await?;

    row
      .exists
      .with_context(|| "Failed to return bool from EXISTS query")
  }

  pub async fn get_user_chart_stats(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    user_id: &serenity::UserId,
    timeframe: &Timeframe,
  ) -> Result<Vec<TimeframeStats>> {
    // Get the last 12 days, weeks, months, or years
    let rows: Vec<Res> = match timeframe {
      Timeframe::Daily => {
        sqlx::query_as!(
          Res,
          r#"WITH "daily_data" AS (
            SELECT date_part('day', NOW() - DATE_TRUNC('day', "occurred_at")) AS times_ago, meditation_minutes
            FROM meditation
            WHERE guild_id = $1 AND user_id = $2
          ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
          FROM "daily_data"
          WHERE "times_ago" <= 12
          GROUP BY "times_ago";"#,
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Weekly => {
        sqlx::query_as!(
          Res,
          r#"WITH "weekly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*7))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1 AND "user_id" = $2
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "weekly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Monthly => {
        sqlx::query_as!(
          Res,
          r#"WITH "monthly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*30))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1 AND "user_id" = $2
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "monthly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Yearly => {
        sqlx::query_as!(
          Res,
          r#"WITH "yearly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*365))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1 AND "user_id" = $2
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "yearly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
          user_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
    };

    #[allow(clippy::expect_used)]
    let stats: Vec<TimeframeStats> = (0..12)
      .map(|i| {
        // Comparison is safe since floor produces integer
        #[allow(clippy::float_cmp)]
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

    Ok(stats)
  }

  pub async fn get_guild_chart_stats(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    guild_id: &serenity::GuildId,
    timeframe: &Timeframe,
  ) -> Result<Vec<TimeframeStats>> {
    // Get the last 12 days, weeks, months, or years
    let rows: Vec<Res> = match timeframe {
      Timeframe::Daily => {
        sqlx::query_as!(
          Res,
          r#"WITH "daily_data" AS (
            SELECT date_part('day', NOW() - DATE_TRUNC('day', "occurred_at")) AS times_ago, meditation_minutes
            FROM meditation
            WHERE guild_id = $1
          ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
          FROM "daily_data"
          WHERE "times_ago" <= 12
          GROUP BY "times_ago";"#,
          guild_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Weekly => {
        sqlx::query_as!(
          Res,
          r#"WITH "weekly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*7))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "weekly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Monthly => {
        sqlx::query_as!(
          Res,
          r#"WITH "monthly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*30))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "monthly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
      Timeframe::Yearly => {
        sqlx::query_as!(
          Res,
          r#"WITH "yearly_data" AS (
            SELECT floor(extract(epoch from NOW() - "occurred_at")/(60*60*24*365))::float AS "times_ago", meditation_minutes
            FROM meditation
            WHERE "guild_id" = $1
        ) SELECT "times_ago", SUM(meditation_minutes) AS meditation_minutes, COUNT(*) AS meditation_count
            FROM "yearly_data"
            WHERE "times_ago" <= 12
        GROUP BY "times_ago";"#,
          guild_id.to_string(),
        ).fetch_all(&mut **transaction).await?
      },
    };

    #[allow(clippy::expect_used)]
    let stats: Vec<TimeframeStats> = (0..12)
      .map(|i| {
        // Comparison is safe since floor produces integer
        #[allow(clippy::float_cmp)]
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

    Ok(stats)
  }

  pub async fn get_star_message_by_message_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    message_id: &serenity::MessageId,
  ) -> Result<Option<StarMessage>> {
    let row = sqlx::query!(
      r#"
        SELECT record_id, starred_message_id, board_message_id, starred_channel_id
        FROM "star"
        WHERE starred_message_id = $1
      "#,
      message_id.to_string(),
    )
    .fetch_optional(&mut **transaction)
    .await?;

    let star_message = match row {
      Some(row) => Some(StarMessage {
        record_id: row.record_id,
        starred_message_id: serenity::MessageId::new(row.starred_message_id.parse::<u64>()?),
        board_message_id: serenity::MessageId::new(row.board_message_id.parse::<u64>()?),
        starred_channel_id: serenity::ChannelId::new(row.starred_channel_id.parse::<u64>()?),
      }),
      None => None,
    };

    Ok(star_message)
  }

  pub async fn delete_star_message(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    record_id: &str,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        DELETE FROM "star" WHERE record_id = $1
      "#,
      record_id,
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }

  pub async fn insert_star_message(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    starred_message_id: &serenity::MessageId,
    board_message_id: &serenity::MessageId,
    starred_channel_id: &serenity::ChannelId,
  ) -> Result<()> {
    sqlx::query!(
      r#"
        INSERT INTO "star" (record_id, starred_message_id, board_message_id, starred_channel_id) VALUES ($1, $2, $3, $4)
        ON CONFLICT (starred_message_id) DO UPDATE SET board_message_id = $3
      "#,
      Ulid::new().to_string(),
      starred_message_id.to_string(),
      board_message_id.to_string(),
      starred_channel_id.to_string(),
    )
    .execute(&mut **transaction)
    .await?;

    Ok(())
  }
}
