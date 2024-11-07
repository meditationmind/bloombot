use anyhow::Result;
use poise::serenity_prelude::{ChannelId, GuildId, MessageId};
use sqlx::{postgres::PgArguments, query::Query, FromRow, Postgres};
use ulid::Ulid;

use crate::handlers::database::{DeleteQuery, InsertQuery};

#[derive(FromRow)]
pub struct StarMessage {
  #[sqlx(rename = "record_id")]
  pub id: String,
  #[sqlx(rename = "starred_channel_id")]
  pub starred_channel: String,
  #[sqlx(rename = "starred_message_id")]
  pub starred_message: String,
  #[sqlx(rename = "board_message_id")]
  pub board_message: String,
}

impl StarMessage {
  pub fn new(
    starred_channel: ChannelId,
    starred_message: MessageId,
    board_message: MessageId,
  ) -> Self {
    Self {
      id: Ulid::new().to_string(),
      starred_channel: starred_channel.to_string(),
      starred_message: starred_message.to_string(),
      board_message: board_message.to_string(),
    }
  }

  pub fn board_message(&self) -> Result<MessageId> {
    Ok(MessageId::new(self.board_message.parse::<u64>()?))
  }
}

impl InsertQuery for StarMessage {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO star (record_id, starred_message_id, board_message_id, starred_channel_id) VALUES ($1, $2, $3, $4) ON CONFLICT (starred_message_id) DO UPDATE SET board_message_id = $3",
      self.id,
      self.starred_message,
      self.board_message,
      self.starred_channel,
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
