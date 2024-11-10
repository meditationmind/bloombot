use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::data::common;
use crate::handlers::database::InsertQuery;

pub struct Erase {
  id: String,
  guild_id: GuildId,
  user_id: UserId,
  message_link: String,
  reason: String,
  occurred_at: DateTime<Utc>,
}

impl Erase {
  /// Creates a new [`Erase`] with the specified values and an automatically
  /// generated unique ID.
  pub fn new(
    guild_id: GuildId,
    user_id: UserId,
    link: impl Into<String>,
    reason: impl Into<String>,
    datetime: &DateTime<Utc>,
  ) -> Self {
    Self {
      id: Ulid::new().to_string(),
      guild_id,
      user_id,
      message_link: link.into(),
      reason: reason.into(),
      occurred_at: *datetime,
    }
  }

  /// Retrieves all [`Erase`]s for the specified `user_id`.
  pub fn retrieve_all<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, message_link, reason, occurred_at FROM erases WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }
}

impl Default for Erase {
  fn default() -> Self {
    Self {
      id: String::default(),
      guild_id: GuildId::default(),
      user_id: UserId::default(),
      message_link: "None".to_string(),
      reason: "No reason provided.".to_string(),
      occurred_at: DateTime::<Utc>::default(),
    }
  }
}

impl InsertQuery for Erase {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO erases (record_id, user_id, guild_id, message_link, reason, occurred_at) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (message_link) DO UPDATE SET reason = $5",
      self.id,
      self.user_id.to_string(),
      self.guild_id.to_string(),
      self.message_link,
      self.reason,
      self.occurred_at,
    )
  }
}

impl PageRow for Erase {
  fn title(&self, page_type: PageType) -> String {
    match page_type {
      PageType::Standard => {
        if self.occurred_at == (DateTime::<Utc>::default()) {
          "Date: `Not Available`".to_owned()
        } else {
          format!("Date: `{}`", self.occurred_at.format("%Y-%m-%d %H:%M"))
        }
      }
      PageType::Alternate => {
        if self.occurred_at == (DateTime::<Utc>::default()) {
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
      format!(
        "**Reason:** {}\n[Go to erase notification]({})",
        self.reason, self.message_link
      )
    }
  }
}

impl FromRow<'_, PgRow> for Erase {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);
    let user_id = UserId::new(common::decode_id_row(row, "user_id")?);

    Ok(Self {
      id: row.try_get("record_id").unwrap_or_default(),
      guild_id,
      user_id,
      message_link: row.try_get("message_link").unwrap_or_default(),
      reason: row
        .try_get::<Option<String>, &str>("reason")?
        .unwrap_or_default(),
      occurred_at: row.try_get("occurred_at").unwrap_or_default(),
    })
  }
}
