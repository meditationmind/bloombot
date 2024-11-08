use poise::serenity_prelude::{ChannelId, GuildId, MessageId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{FromRow, Postgres, Row};
use ulid::Ulid;

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
  fn from_row(row: &'_ PgRow) -> sqlx::Result<Self, sqlx::Error> {
    let starred_channel: String = row.try_get("starred_channel_id").unwrap_or("1".to_string());
    let starred_channel = match starred_channel.parse::<u64>() {
      Ok(id) => ChannelId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "starred_channel_id".to_string(),
          source: Box::new(e),
        })
      }
    };
    let starred_message: String = row.try_get("starred_message_id").unwrap_or("1".to_string());
    let starred_message = match starred_message.parse::<u64>() {
      Ok(id) => MessageId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "starred_message_id".to_string(),
          source: Box::new(e),
        })
      }
    };
    let board_message: String = row.try_get("board_message_id").unwrap_or("1".to_string());
    let board_message = match board_message.parse::<u64>() {
      Ok(id) => MessageId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "board_message_id".to_string(),
          source: Box::new(e),
        })
      }
    };

    Ok(Self {
      id: row.try_get("record_id").unwrap_or_default(),
      starred_channel,
      starred_message,
      board_message,
    })
  }
}
