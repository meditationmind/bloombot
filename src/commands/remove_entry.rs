use anyhow::{Context as AnyhowContext, Result};
use poise::CreateReply;
use poise::serenity_prelude::{ChannelId, CreateEmbedFooter, CreateMessage};

use crate::Context;
use crate::commands::helpers::common::{self, Visibility};
use crate::commands::helpers::database::{self, MessageType};
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::database::DatabaseHandler;

/// Remove one of your meditation entries
///
/// Removes one of your meditation entries.
///
/// Use `/recent` to retrieve the ID for the entry you wish to remove.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  rename = "remove",
  guild_only
)]
pub async fn remove_entry(
  ctx: Context<'_>,
  #[description = "The ID of the entry to remove"] id: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let Some(entry) =
    DatabaseHandler::get_meditation_entry(&mut transaction, &guild_id, id.as_str()).await?
  else {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} No entry found with that ID.\n-# Use {} to view a list of your entries and their IDs.",
            EMOJI.mminfo,
            common::print_command(ctx, guild_id, "recent").await?
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  if entry.user_id != ctx.author().id {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} You can only remove your own entries.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_meditation_entry(&mut transaction, id.as_str()).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Entry has been removed.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  let description = if entry.seconds > 0 {
    format!(
      "**User**: {}\n**ID**: {}\n**Date**: {}\n**Time**: {} minute(s) {} second(s)",
      ctx.author(),
      entry.id,
      entry.occurred_at.format("%B %d, %Y"),
      entry.minutes,
      entry.seconds,
    )
  } else {
    format!(
      "**User**: {}\n**ID**: {}\n**Date**: {}\n**Time**: {} minute(s)",
      ctx.author(),
      entry.id,
      entry.occurred_at.format("%B %d, %Y"),
      entry.minutes,
    )
  };

  let log_embed = BloomBotEmbed::new()
    .title("Meditation Entry Removed")
    .description(description)
    .footer(
      CreateEmbedFooter::new(format!(
        "Removed by {} ({})",
        ctx.author().name,
        ctx.author().id
      ))
      .icon_url(ctx.author().avatar_url().unwrap_or_default()),
    );

  let log_channel = ChannelId::new(CHANNELS.bloomlogs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}
