use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{CreatesInDatabase, DeletesInDatabase};
use chrono::{DateTime, Utc};
use poise::serenity_prelude::{self as serenity};
use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::Postgres;
use ulid::Ulid;

#[derive(Default, sqlx::FromRow)]
#[sqlx(default)]
pub struct Bookmark {
  #[sqlx(rename = "record_id")]
  pub id: String,
  guild_id: String,
  user_id: String,
  #[sqlx(rename = "message_link")]
  pub link: String,
  #[sqlx(rename = "user_desc")]
  pub description: Option<String>,
  #[sqlx(rename = "occurred_at")]
  pub added: Option<DateTime<Utc>>,
}

impl Bookmark {
  pub(crate) fn new(
    guild_id: serenity::GuildId,
    user_id: serenity::UserId,
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

impl CreatesInDatabase for Bookmark {
  fn create_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      r#"
        INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc) VALUES ($1, $2, $3, $4, $5)
      "#,
      self.id,
      self.user_id,
      self.guild_id,
      self.link,
      self.description,
    )
  }
}

impl DeletesInDatabase for Bookmark {
  fn delete_query<'a>(id: String) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      r#"
        DELETE FROM bookmarks WHERE record_id = $1
      "#,
      id,
    )
  }
}
