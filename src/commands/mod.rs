use crate::config::BloomBotEmbed;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

pub mod add;
pub mod add_key;
pub mod coffee;
pub mod complete;
pub mod courses;
pub mod erase;
pub mod glossary;
pub mod hello;
pub mod list_keys;
pub mod manage;
pub mod pick_winner;
pub mod ping;
pub mod quote;
pub mod quotes;
pub mod recent;
pub mod remove_entry;
pub mod remove_key;
pub mod remove_quote;
pub mod stats;
pub mod streak;
pub mod suggest;
pub mod terms;
pub mod use_key;

enum MessageType {
  TextOnly(String),
  EmbedOnly(serenity::CreateEmbed),
}

/// Takes a transaction and a response, committing the transaction if we can successfully send a message.
/// This is useful because we don't always know whether the interaction has timed out or not,
/// and we don't want to commit any changes if we can't inform the user of the result.
/// If we could not commit the transaction but were able to send a message, we will edit the message to inform the user.
///
/// # Arguments
/// ctx - The context of the interaction
/// transaction - The transaction to commit
/// message - The message to send
/// ephemeral - Whether the message should be ephemeral
///
/// # Returns
/// Result<()> - Whether the message was sent successfully
///
/// # Errors
///
async fn commit_and_say(
  ctx: Context<'_>,
  transaction: sqlx::Transaction<'_, sqlx::Postgres>,
  message: MessageType,
  ephemeral: bool,
) -> Result<()> {
  let response = match message {
    MessageType::TextOnly(message) => ctx.send(|f| f.content(message).ephemeral(ephemeral)).await,
    MessageType::EmbedOnly(message) => {
      ctx
        .send(|f| {
          f.ephemeral(ephemeral);

          f.embeds = vec![message];

          f
        })
        .await
    }
  };

  match response {
    Ok(sent_message) => {
      match DatabaseHandler::commit_transaction(transaction).await {
        Ok(_) => {}
        Err(e) => {
          let _ = sent_message.edit(ctx, |f| f
            .content(":bangbang: A fatal error occured while trying to save your changes. Nothing has been saved.")
            .ephemeral(true));
          return Err(anyhow::anyhow!("Could not send message: {}", e));
        }
      };
    }
    Err(e) => {
      DatabaseHandler::rollback_transaction(transaction).await?;
      // As it's very likely that when this happens the interaction has timed out,
      // we don't want to send a response to the interaction, but rather to the channel.
      let channel = ctx.channel_id();
      let _ = channel
        .say(ctx, ":x: An error occured. Nothing has been saved.")
        .await;
      return Err(anyhow::anyhow!("Could not send message: {}", e));
    }
  };

  Ok(())
}

pub async fn course_not_found(
  ctx: Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: serenity::GuildId,
  course_name: String,
) -> Result<()> {
  let possible_course =
    DatabaseHandler::get_possible_course(transaction, &guild_id, course_name.as_str(), 0.8).await?;

  if let Some(possible_course) = possible_course {
    // Check if user is in the course
    if ctx
      .author()
      .has_role(ctx, guild_id, possible_course.participant_role)
      .await?
    {
      ctx
        .send(|f| {
          f.content(format!(
            ":x: Course does not exist. Did you mean `{}`?",
            possible_course.course_name
          ))
          .ephemeral(true)
        })
        .await?;
    } else {
      ctx
        .send(|f| f.content(":x: Course does not exist.").ephemeral(true))
        .await?;
    }
  } else {
    ctx
      .send(|f| f.content(":x: Course does not exist.").ephemeral(true))
      .await?;
  }

  Ok(())
}