use poise::serenity_prelude::{ChannelId, GuildId, MessageId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::data::common;
use crate::handlers::database::{DeleteQuery, InsertQuery};

pub struct StarMessage {
  pub id: String,
  pub starred_channel: ChannelId,
  pub starred_message: MessageId,
  pub board_message: MessageId,
}

impl StarMessage {
  pub fn new(
    starred_channel: ChannelId,
    starred_message: MessageId,
    board_message: MessageId,
  ) -> Self {
    Self {
      id: Ulid::new().to_string(),
      starred_channel,
      starred_message,
      board_message,
    }
  }

  /// Retrieves all [`Recipient`]s from the database.
  pub fn retrieve<'a>(message_id: MessageId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, starred_message_id, board_message_id, starred_channel_id FROM star WHERE starred_message_id = $1",
    )
    .bind(message_id.to_string())
  }
}

impl InsertQuery for StarMessage {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO star (record_id, starred_message_id, board_message_id, starred_channel_id) VALUES ($1, $2, $3, $4) ON CONFLICT (starred_message_id) DO UPDATE SET board_message_id = $3",
      self.id,
      self.starred_message.to_string(),
      self.board_message.to_string(),
      self.starred_channel.to_string(),
    )
  }
}

impl DeleteQuery for StarMessage {
  fn delete_query<'a>(
    _guild_id: GuildId,
    record_id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!("DELETE FROM star WHERE record_id = $1", record_id.into())
  }
}

impl FromRow<'_, PgRow> for StarMessage {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let starred_channel = ChannelId::new(common::decode_id_row(row, "starred_channel_id")?);
    let starred_message = MessageId::new(common::decode_id_row(row, "starred_message_id")?);
    let board_message = MessageId::new(common::decode_id_row(row, "board_message_id")?);

    Ok(Self {
      id: row.try_get("record_id").unwrap_or_default(),
      starred_channel,
      starred_message,
      board_message,
    })
  }
}
