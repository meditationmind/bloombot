use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{CreatesInDatabase, DeletesInDatabase};
use chrono::Utc;
use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::Postgres;
use ulid::Ulid;

pub struct Bookmark {
  pub id: String,
  guild_id: serenity::GuildId,
  user_id: serenity::UserId,
  pub link: String,
  pub description: Option<String>,
  pub added: Some(chrono::DateTime<Utc>),
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
      guild_id,
      user_id,
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

impl CreatesInDatabase for Bookmark {
  fn create_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      r#"
        INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc) VALUES ($1, $2, $3, $4, $5)
      "#,
      self.id,
      self.user_id.to_string(),
      self.guild_id.to_string(),
      self.link,
      self.description,
    )
  }
}

impl DeletesInDatabase for Bookmark {
  fn delete_query(id: String) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      r#"
        DELETE FROM bookmarks WHERE record_id = $1
      "#,
      id,
    )
  }
}
