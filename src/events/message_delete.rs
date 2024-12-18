use anyhow::Result;
use poise::serenity_prelude::MessageId;

use crate::database::DatabaseHandler;

pub async fn message_delete(
  database: &DatabaseHandler,
  deleted_message_id: &MessageId,
) -> Result<()> {
  let mut transaction = database.start_transaction().await?;

  let star_message =
    DatabaseHandler::get_star_message(&mut transaction, deleted_message_id).await?;

  if let Some(star_message) = star_message {
    let star_message_id = star_message.id;
    DatabaseHandler::remove_star_message(&mut transaction, &star_message_id).await?;
  }

  transaction.commit().await?;

  Ok(())
}
