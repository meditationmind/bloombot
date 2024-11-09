use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::query::{Query, QueryAs};
use sqlx::{postgres::PgArguments, FromRow, Postgres};
use ulid::Ulid;

use crate::{
  commands::helpers::pagination::{PageRow, PageType},
  handlers::database::InsertQuery,
};

#[derive(FromRow)]
pub struct Erase {
  #[sqlx(rename = "record_id")]
  id: String,
  #[sqlx(skip)]
  guild_id: GuildId,
  #[sqlx(skip)]
  user_id: UserId,
  #[sqlx(default)]
  message_link: String,
  #[sqlx(default)]
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
