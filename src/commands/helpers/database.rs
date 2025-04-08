use anyhow::{Result, anyhow};
use poise::CreateReply;
use poise::serenity_prelude::CreateEmbed;
use sqlx::{Postgres, Transaction};

use crate::Context;
use crate::commands::helpers::common::Visibility;
use crate::config::EMOJI;
use crate::database::DatabaseHandler;

pub enum MessageType {
  TextOnly(String),
  EmbedOnly(Box<CreateEmbed>),
}

/// Takes a transaction and a response, committing the transaction if the message is sent successfully,
/// and rolling the transaction back otherwise. This prevents changes from being committed when the user
/// cannot be informed of the changes, e.g., when an interaction has timed out.
///
/// # Errors
/// If the message is sent but the commit fails, the message will be edited to inform the user and the
/// error resulting in the failure will be logged.
///
/// If the message can't be sent, a notification is sent to the channel where the command was used and
/// the error is logged. However, note there are two exceptions:
/// - `Interaction has already been acknowledged.`
/// - `Unknown interaction`
///
/// Errors of these types are assumed to be caused by multiple instances of the bot, which occurs due
/// to the Kubernetes-style hosting used by the bot. These errors are not reported to the user or
/// channel, but are still logged.
pub async fn commit_and_say(
  ctx: Context<'_>,
  transaction: Transaction<'_, Postgres>,
  message: MessageType,
  visibility: Visibility,
) -> Result<()> {
  let ephemeral = match visibility {
    Visibility::Public => false,
    Visibility::Ephemeral => true,
  };

  let response = match message {
    MessageType::TextOnly(message) => {
      ctx
        .send(CreateReply::default().content(message).ephemeral(ephemeral))
        .await
    }
    MessageType::EmbedOnly(message) => {
      ctx
        .send(CreateReply::default().embed(*message).ephemeral(ephemeral))
        .await
    }
  };

  match response {
    Ok(sent_message) => {
      if let Err(e) = DatabaseHandler::commit_transaction(transaction).await {
        _ = sent_message.edit(ctx, CreateReply::default()
            .content(format!("{} A fatal error occurred while trying to save your changes. Please contact staff for assistance.", EMOJI.mminfo))
            .ephemeral(true)).await;
        return Err(anyhow!("Could not send message: {e}"));
      }
    }
    Err(e) => {
      DatabaseHandler::rollback_transaction(transaction).await?;

      // This usually happens when two instances of the bot are running.
      // If interaction is unknown or has already been acknowledged, assume
      // such and ignore error since command will have been successful.
      if e.to_string() == "Interaction has already been acknowledged."
        || e.to_string() == "Unknown interaction"
      {
        return Err(anyhow!("Multiple instances assumed. Ignoring error: {e}"));
      }

      // Otherwise, it's likely the interaction has timed out for some reason.
      // We'll send a response to the channel to inform the user.
      _ = ctx
        .channel_id()
        .say(&ctx, format!("{} An error may have occurred. If your command failed, please contact staff for assistance.", EMOJI.mminfo))
        .await;

      return Err(anyhow!("Could not send message: {e}"));
    }
  }

  Ok(())
}
