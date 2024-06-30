use crate::config::BloomBotEmbed;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use log::info;
use poise::{serenity_prelude as serenity, CreateReply};
use std::sync::atomic::Ordering;

pub mod add;
pub mod challenge;
pub mod coffee;
pub mod complete;
pub mod courses;
pub mod customize;
pub mod erase;
pub mod glossary;
pub mod hello;
pub mod help;
pub mod keys;
pub mod manage;
pub mod pick_winner;
pub mod ping;
pub mod quote;
pub mod quotes;
pub mod recent;
pub mod remove_entry;
pub mod report_message;
pub mod stats;
pub mod streak;
pub mod suggest;
pub mod terms;
pub mod whatis;

#[allow(clippy::large_enum_variant)]
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
    MessageType::TextOnly(message) => {
      ctx
        .send(CreateReply::default().content(message).ephemeral(ephemeral))
        .await
    }
    MessageType::EmbedOnly(message) => {
      ctx
        .send(CreateReply { embeds: vec![message], ..Default::default() }.ephemeral(ephemeral))
        .await
    }
  };

  match response {
    Ok(sent_message) => {
      match DatabaseHandler::commit_transaction(transaction).await {
        Ok(()) => {}
        Err(e) => {
          let _ = sent_message.edit(ctx, CreateReply::default()
            .content("<:mminfo:1194141918133768234> A fatal error occurred while trying to save your changes. Please contact staff for assistance.")
            .ephemeral(true)).await;
          return Err(anyhow::anyhow!("Could not send message: {e}"));
        }
      };
    }
    Err(e) => {
      DatabaseHandler::rollback_transaction(transaction).await?;
      // As it's very likely that when this happens the interaction has timed out,
      // we don't want to send a response to the interaction, but rather to the channel.
      // The alternative is that there is a second instance of the bot running, which we can detect by checking if the interaction has already been responded to.

      match ctx {
        poise::Context::Application(app_ctx) => {
          let has_sent_initial_response = app_ctx.has_sent_initial_response.load(Ordering::SeqCst);
          if !has_sent_initial_response {
            let _ = ctx
              .channel_id()
              .say(&ctx, "<:mminfo:1194141918133768234> An error may have occurred. If your command failed, please contact staff for assistance.")
              .await;
            info!("Issued rollback transaction error for slash command with no initial response.");
          }
        }
        poise::Context::Prefix(_) => {
          let _ = ctx
            .channel_id()
            .say(&ctx, "<:mminfo:1194141918133768234> An error may have occurred. If your command failed, please contact staff for assistance.")
            .await;
          info!("Issued rollback transaction error for prefix command.");
        }
      };

      return Err(anyhow::anyhow!("Could not send message: {e}"));
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
        .send(
          poise::CreateReply::default()
            .content(format!(
              ":x: Course does not exist. Did you mean `{}`?",
              possible_course.course_name
            ))
            .ephemeral(true),
        )
        .await?;
    } else {
      ctx
        .send(
          poise::CreateReply::default()
            .content(":x: Course does not exist.")
            .ephemeral(true),
        )
        .await?;
    }
  } else {
    ctx
      .send(
        poise::CreateReply::default()
          .content(":x: Course does not exist.")
          .ephemeral(true),
      )
      .await?;
  }

  Ok(())
}
