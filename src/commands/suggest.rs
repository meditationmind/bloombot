use anyhow::Result;
use poise::CreateReply;
use poise::serenity_prelude::{AutoArchiveDuration, ChannelId, ChannelType, CreateEmbedAuthor};
use poise::serenity_prelude::{CreateEmbedFooter, CreateMessage, CreateThread};

use crate::Context;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};

/// Submit an anonymous server suggestion
///
/// Submits an anonymous suggestion to the server suggestions channel, with voting reactions and a thread for discussion.
///
/// *Note: Suggestions are posted anonymously, but server staff will be able to see who created a suggestion.*
#[poise::command(
  slash_command,
  category = "Utilities",
  member_cooldown = 3600,
  guild_only
)]
pub async fn suggest(
  ctx: Context<'_>,
  #[description = "The suggestion to add"] suggestion: String,
) -> Result<()> {
  // Log suggestion in staff channel
  let log_embed = BloomBotEmbed::new()
    .title("New Suggestion")
    .description(&suggestion)
    .author(CreateEmbedAuthor::new(&ctx.author().name).icon_url(ctx.author().face()))
    .footer(CreateEmbedFooter::new(format!(
      "Author ID: {}",
      &ctx.author().id
    )));

  let log_channel = ChannelId::new(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  // Post suggestion and reactions
  let channel_id = ChannelId::new(CHANNELS.suggestion);

  let suggestion_message = channel_id
    .send_message(
      ctx,
      CreateMessage::new().embed(BloomBotEmbed::new().description(&suggestion)),
    )
    .await?;

  suggestion_message.react(ctx, '✅').await?;
  suggestion_message.react(ctx, '❌').await?;

  // Start thread for suggestion
  channel_id
    .create_thread_from_message(
      ctx,
      suggestion_message.id,
      CreateThread::new({
        if suggestion.chars().count() > 85 {
          format!(
            "Discussion: {}...",
            suggestion.chars().take(85).collect::<String>()
          )
        } else {
          format!("Discussion: {suggestion}")
        }
      })
      .kind(ChannelType::PublicThread)
      .auto_archive_duration(AutoArchiveDuration::OneWeek),
    )
    .await?;

  let msg = format!(
    "{} Your suggestion has been added to <#{channel_id}>.",
    EMOJI.mminfo
  );
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}
