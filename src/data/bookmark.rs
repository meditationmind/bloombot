use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::{FromRow, Postgres};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{DeleteQuery, InsertQuery};

#[derive(Default, FromRow)]
#[sqlx(default)]
pub struct Bookmark {
  #[sqlx(rename = "record_id")]
  id: String,
  guild_id: String,
  user_id: String,
  #[sqlx(rename = "message_link")]
  pub link: String,
  #[sqlx(rename = "user_desc")]
  pub description: Option<String>,
  #[sqlx(rename = "occurred_at")]
  added: Option<DateTime<Utc>>,
}

impl Bookmark {
  pub(crate) fn new(
    guild_id: GuildId,
    user_id: UserId,
    link: String,
    description: Option<String>,
  ) -> Self {
    Self {
      id: Ulid::new().to_string(),
      guild_id: guild_id.to_string(),
      user_id: user_id.to_string(),
      link,
      description,
      added: None,
    }
  }

  pub fn id(&self) -> &str {
    &self.id
  }

  pub fn added(&self) -> Option<&DateTime<Utc>> {
    self.added.as_ref()
  }
}

impl PageRow for Bookmark {
  fn title(&self, _page_type: PageType) -> String {
    self.link.clone()
  }

  fn body(&self) -> String {
    let desc = match &self.description {
      Some(description) => format!("> {description}\n"),
      None => String::new(),
    };
    let ts = match self.added {
      Some(added) => added.timestamp(),
      None => 0i64,
    };
    format!(
      "{desc}> -# Added: <t:{}:f>\n> -# ID: [{}](discord://{} \"For copying a bookmark ID on mobile. Not a working link.\")\n** **",
      ts,
      self.id,
      self.id,
    )
  }
}

impl InsertQuery for Bookmark {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc) VALUES ($1, $2, $3, $4, $5)",
      self.id,
      self.user_id,
      self.guild_id,
      self.link,
      self.description,
    )
  }
}

impl DeleteQuery for Bookmark {
  fn delete_query<'a>(
    _guild_id: GuildId,
    id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!("DELETE FROM bookmarks WHERE record_id = $1", id.into())
  }
}
