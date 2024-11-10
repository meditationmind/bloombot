use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::data::common;
use crate::handlers::database::{DeleteQuery, InsertQuery};

#[derive(Default)]
pub struct Bookmark {
  id: String,
  guild_id: GuildId,
  user_id: UserId,
  pub link: String,
  pub description: Option<String>,
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
      guild_id,
      user_id,
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

  /// Retrieves the total number of [`Bookmark`]s for the specified `user_id`.
  pub fn user_total<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as(
      "SELECT COUNT(record_id) AS count FROM bookmarks WHERE user_id = $1 AND guild_id = $2",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves all [`Bookmark`]s for the specified `user_id`.
  pub fn retrieve_all<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, message_link, user_desc, occurred_at FROM bookmarks WHERE guild_id = $1 AND user_id = $2 ORDER BY occurred_at ASC",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
  }

  /// Searches a user's [`Bookmark`]s using a [PostgreSQL websearch query][ws] defined in `keyword`.
  ///
  /// [ws]: https://www.postgresql.org/docs/17/textsearch-controls.html#TEXTSEARCH-PARSING-QUERIES
  pub fn search<'a>(
    guild_id: GuildId,
    user_id: UserId,
    keyword: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, message_link, user_desc, occurred_at FROM bookmarks WHERE user_id = $1 AND guild_id = $2 AND (desc_tsv @@ websearch_to_tsquery('english', $3)) ORDER BY ts_rank(desc_tsv, websearch_to_tsquery('english', $3)) DESC",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
    .bind(keyword.to_string())
  }
}

impl InsertQuery for Bookmark {
  /// Adds a [`Bookmark`] to the database.
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO bookmarks (record_id, user_id, guild_id, message_link, user_desc) VALUES ($1, $2, $3, $4, $5)",
      self.id,
      self.user_id.to_string(),
      self.guild_id.to_string(),
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

impl FromRow<'_, PgRow> for Bookmark {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);
    let user_id = UserId::new(common::decode_id_row(row, "user_id")?);

    Ok(Self {
      id: row.try_get("record_id").unwrap_or_default(),
      guild_id,
      user_id,
      link: row.try_get("message_link").unwrap_or_default(),
      description: row.try_get("user_desc").unwrap_or_default(),
      added: row.try_get("occurred_at").unwrap_or_default(),
    })
  }
}
