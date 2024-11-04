use crate::database::DatabaseHandler;
use anyhow::Result;
use poise::serenity_prelude as serenity;

pub async fn message_delete(
  database: &DatabaseHandler,
  deleted_message_id: &serenity::MessageId,
) -> Result<()> {
  let mut transaction = database.start_transaction().await?;

  let star_message =
    DatabaseHandler::get_star_message_by_message_id(&mut transaction, deleted_message_id).await?;

  if let Some(star_message) = star_message {
    let star_message_id = star_message.record_id;
    DatabaseHandler::remove_star_message(&mut transaction, &star_message_id).await?;
  }

  transaction.commit().await?;

  Ok(())
}
