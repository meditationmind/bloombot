use std::time::Duration;

use anyhow::Result;
use poise::serenity_prelude::futures::StreamExt;
use poise::serenity_prelude::{ButtonStyle, ComponentInteractionCollector, CreateActionRow};
use poise::serenity_prelude::{CreateButton, CreateInteractionResponse, Message, MessageType};
use poise::CreateReply;

use crate::config::EMOJI;
use crate::Context;

/// Extract body text from an AutoMod report or embedded message
///
/// Extracts body text from an AutoMod report or embedded message, making it possible to copy and paste on mobile.
///
/// To use, right-click a message, then go to "Apps" > "Extract Text".
#[poise::command(
  ephemeral,
  required_permissions = "MANAGE_ROLES",
  default_member_permissions = "MANAGE_ROLES",
  context_menu_command = "Extract Text",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn extract_text(
  ctx: Context<'_>,
  #[description = "Message to extract text from"] message: Message,
) -> Result<()> {
  let msg = if let Some(description) = &message.embeds[0].description {
    description
  } else if !message.content.is_empty() {
    &message.content
  } else {
    &format!("{} No message body.", EMOJI.mminfo)
  };
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}

#[poise::command(
  ephemeral,
  slash_command,
  required_permissions = "MANAGE_ROLES",
  default_member_permissions = "MANAGE_ROLES",
  category = "Moderator Commands",
  subcommands("automod"),
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn extract(_: Context<'_>) -> Result<()> {
  Ok(())
}

#[poise::command(slash_command)]
async fn automod(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;
  let initial_response = ctx
    .send(CreateReply::default().content("Fetching AutoMod messages..."))
    .await?;

  let author_id = ctx.author().id;
  let channel_id = ctx.channel_id();
  let mut messages = channel_id.messages_iter(&ctx).boxed();
  'stream: while let Some(message_result) = messages.next().await {
    if let Ok(message) = message_result {
      if matches!(message.kind, MessageType::AutoModAction) {
        let msg = if let Some(description) = &message.embeds[0].description {
          description
        } else if !message.content.is_empty() {
          &message.content
        } else {
          &format!("{} No message body.", EMOJI.mminfo)
        };

        let custom_id = ctx.id() + message.id.get();
        let next_id = format!("{custom_id}next");
        let stop_id = format!("{custom_id}stop");

        initial_response
          .edit(
            ctx,
            CreateReply::default()
              .content(msg)
              .components(vec![CreateActionRow::Buttons(vec![
                CreateButton::new(next_id.as_str())
                  .label("Next")
                  .style(ButtonStyle::Success),
                CreateButton::new(stop_id.as_str())
                  .label("Stop")
                  .style(ButtonStyle::Danger),
              ])]),
          )
          .await?;

        while let Some(press) = ComponentInteractionCollector::new(ctx)
          .author_id(author_id)
          .filter(move |press| press.data.custom_id.starts_with(&custom_id.to_string()))
          .timeout(Duration::from_secs(60))
          .await
        {
          if press.data.custom_id != next_id && press.data.custom_id != stop_id {
            // This is an unrelated button interaction.
            continue;
          }

          let next = press.data.custom_id == next_id;
          let stop = press.data.custom_id == stop_id;

          if next {
            press
              .create_response(ctx, CreateInteractionResponse::Acknowledge)
              .await?;
            continue 'stream;
          } else if stop {
            break 'stream;
          }
        }
      }
    }
  }

  let msg = format!("{} No message found.", EMOJI.mminfo);
  initial_response
    .edit(ctx, CreateReply::default().content(msg).components(vec![]))
    .await?;

  Ok(())
}
