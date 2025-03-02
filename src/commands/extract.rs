use std::time::Duration;

use anyhow::Result;
use poise::CreateReply;
use poise::serenity_prelude::futures::StreamExt;
use poise::serenity_prelude::{ButtonStyle, ComponentInteractionCollector, CreateActionRow};
use poise::serenity_prelude::{CreateButton, CreateInteractionResponse, Message, MessageType};

use crate::Context;
use crate::config::EMOJI;

/// Extract text from an AutoMod report or embed
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
  let (msg1, msg2) = extract_content(message);

  ctx.send(CreateReply::default().content(msg1)).await?;

  if !msg2.is_empty() {
    ctx.send(CreateReply::default().content(msg2)).await?;
  }

  Ok(())
}

/// Commands for extracting message text
///
/// Commands for extracting body text from messages, making it possible to copy and paste on mobile.
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

/// Extract body text from AutoMod reports
///
/// Cycles through AutoMod reports and extracts the body text for each report, making it possible to copy and paste on mobile.
#[poise::command(slash_command, ephemeral)]
async fn automod(ctx: Context<'_>) -> Result<()> {
  let initial_response = ctx
    .send(CreateReply::default().content("Fetching AutoMod messages..."))
    .await?;

  let author_id = ctx.author().id;
  let channel_id = ctx.channel_id();
  let mut messages = channel_id.messages_iter(&ctx).boxed();
  'stream: while let Some(message_result) = messages.next().await {
    if let Ok(message) = message_result {
      if matches!(message.kind, MessageType::AutoModAction) {
        let custom_id = ctx.id() + message.id.get();
        let next_id = format!("{custom_id}next");
        let stop_id = format!("{custom_id}stop");

        let (msg1, msg2) = extract_content(message);

        initial_response
          .edit(
            ctx,
            CreateReply::default()
              .content(msg1)
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

        if !msg2.is_empty() {
          ctx.send(CreateReply::default().content(msg2)).await?;
        }

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
            break;
          }
        }
        initial_response
          .edit(ctx, CreateReply::default().components(vec![]))
          .await?;
        return Ok(());
      }
    }
  }

  let msg = format!("{} No message found.", EMOJI.mminfo);
  initial_response
    .edit(ctx, CreateReply::default().content(msg).components(vec![]))
    .await?;

  Ok(())
}

fn extract_content(message: Message) -> (String, String) {
  let description = if message.embeds.is_empty() {
    ""
  } else {
    message.embeds[0].description.as_deref().unwrap_or_default()
  };

  let content = if !description.is_empty() {
    description.to_owned()
  } else if !message.content.is_empty() {
    message.content
  } else {
    format!("{} No message body.", EMOJI.mminfo)
  };

  if content.chars().count().le(&2000) {
    (content, String::new())
  } else {
    let (msg1, msg2) = if let Some((split1, split2)) = content.split_at_checked(2000) {
      (split1, split2)
    } else {
      (content.as_ref(), "")
    };
    (
      format!("{}...", msg1.chars().take(1997).collect::<String>()),
      if msg2.chars().count().le(&2000) {
        msg2.to_owned()
      } else {
        format!("{}...", msg2.chars().take(1997).collect::<String>())
      },
    )
  }
}
