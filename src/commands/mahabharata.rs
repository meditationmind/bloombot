use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use chrono::{Duration as ChronoDuration, Utc};
use poise::CreateReply;
use poise::serenity_prelude::{ButtonStyle, ChannelId, ComponentInteractionCollector, builder::*};
use poise::serenity_prelude::{FormattedTimestamp, FormattedTimestampStyle};
use poise::serenity_prelude::{Mentionable, RoleId, ScheduledEventStatus};

use crate::Context;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ROLES};

async fn is_host(ctx: Context<'_>) -> Result<bool> {
  let community_book_club_host = RoleId::from(ROLES.community_book_club_host);
  let has_role = ctx
    .author_member()
    .await
    .is_some_and(|member| member.roles.contains(&community_book_club_host));

  if !has_role {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} This command requires the {} role.",
            EMOJI.mminfo,
            community_book_club_host.mention()
          ))
          .allowed_mentions(CreateAllowedMentions::new().empty_roles())
          .ephemeral(true),
      )
      .await?;
  }

  Ok(has_role)
}

/// Manage community book club events
///
/// Commands for managing community book club events. Requires Community Book Club Host role.
#[poise::command(
  slash_command,
  check = "is_host",
  category = "Secret",
  subcommands("start", "end"),
  subcommand_required,
  hide_in_help,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn mahabharata(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Start a community book club event
///
/// Starts a scheduled community book club event.
#[poise::command(slash_command)]
async fn start(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let events = guild_id.scheduled_events(ctx, false).await?;
  for event in events {
    if event.name.as_str().ends_with("Mahabharata")
      && event.status == ScheduledEventStatus::Scheduled
      && (event.start_time.to_utc() - Utc::now()).abs() < ChronoDuration::hours(24)
    {
      let mut embed = BloomBotEmbed::new().description(format!(
        "Starting Event:\n## {}\n{}\n-# Scheduled to begin {}.",
        event.name,
        event.description.unwrap_or(String::new()),
        FormattedTimestamp::new(
          event.start_time,
          Some(FormattedTimestampStyle::RelativeTime)
        )
      ));

      if let Some(image_hash) = event.image {
        embed = embed.image(format!(
          "https://cdn.discordapp.com/guild-events/{}/{}?size=2048",
          event.id, image_hash
        ));
      }

      let ctx_id = ctx.id();
      let confirm_id = format!("{ctx_id}confirm");
      let cancel_id = format!("{ctx_id}cancel");

      ctx
        .send(
          CreateReply::default()
            .embed(embed)
            .ephemeral(true)
            .components(vec![CreateActionRow::Buttons(vec![
              CreateButton::new(confirm_id.as_str())
                .label("Start Event")
                .style(ButtonStyle::Success),
              CreateButton::new(cancel_id.as_str())
                .label("Cancel")
                .style(ButtonStyle::Danger),
            ])]),
        )
        .await?;

      // Loop through incoming interactions with the navigation buttons.
      while let Some(press) = ComponentInteractionCollector::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed in one minute.
        .timeout(Duration::from_secs(60))
        .await
      {
        if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
          // This is an unrelated button interaction.
          continue;
        }

        let confirmed = press.data.custom_id == confirm_id;

        if confirmed {
          match press
            .create_response(
              ctx,
              CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                  .content(format!("{} Event started. Enjoy!", EMOJI.mminfo))
                  .ephemeral(true)
                  .embeds(Vec::new())
                  .components(Vec::new()),
              ),
            )
            .await
          {
            Ok(()) => {
              guild_id
                .edit_scheduled_event(
                  ctx,
                  event.id,
                  EditScheduledEvent::new().status(ScheduledEventStatus::Active),
                )
                .await?;

              let log_message = match event.channel_id {
                Some(channel) => format!(
                  "**Event**: {}\n**Channel:** {}",
                  event.name,
                  channel.mention()
                ),
                None => format!("**Event**: {}\n**Channel:** N/A", event.name),
              };

              let log_embed = BloomBotEmbed::new()
                .title("Event Started")
                .description(log_message)
                .footer(
                  CreateEmbedFooter::new(format!(
                    "Started by {} ({})",
                    ctx.author().name,
                    ctx.author().id
                  ))
                  .icon_url(ctx.author().avatar_url().unwrap_or_default()),
                );

              let log_channel = ChannelId::new(CHANNELS.bloomlogs);

              log_channel
                .send_message(ctx, CreateMessage::new().embed(log_embed))
                .await?;

              return Ok(());
            }
            Err(e) => {
              return Err(anyhow::anyhow!(
                "Failed to start \"{}\" due to error: {e}",
                event.name
              ));
            }
          }
        }

        press
          .create_response(
            ctx,
            CreateInteractionResponse::UpdateMessage(
              CreateInteractionResponseMessage::new()
                .content(format!("{} Cancelled.", EMOJI.mmx))
                .ephemeral(true)
                .embeds(Vec::new())
                .components(Vec::new()),
            ),
          )
          .await?;
      }

      // This happens when the user didn't press any button for 60 seconds.
      return Ok(());
    }
  }

  let msg = format!(
    "{} No eligible community book club event found. Please try again on the day of the event.",
    EMOJI.mminfo
  );
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}

/// End a community book club event
///
/// Ends an active community book club event.
#[poise::command(slash_command)]
async fn end(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let events = guild_id.scheduled_events(ctx, false).await?;
  for event in events {
    if event.name.as_str().ends_with("Mahabharata") && event.status == ScheduledEventStatus::Active
    {
      let ctx_id = ctx.id();
      let confirm_id = format!("{ctx_id}confirm");
      let cancel_id = format!("{ctx_id}cancel");

      let mut embed = BloomBotEmbed::new().description(format!(
        "Ending Event:\n## {}\n{}\n-# Event began {}.",
        event.name,
        event.description.unwrap_or(String::new()),
        FormattedTimestamp::new(
          event.start_time,
          Some(FormattedTimestampStyle::RelativeTime)
        )
      ));

      if let Some(image_hash) = event.image {
        embed = embed.image(format!(
          "https://cdn.discordapp.com/guild-events/{}/{}?size=2048",
          event.id, image_hash
        ));
      }

      ctx
        .send(
          CreateReply::default()
            .embed(embed)
            .ephemeral(true)
            .components(vec![CreateActionRow::Buttons(vec![
              CreateButton::new(confirm_id.as_str())
                .label("End Event")
                .style(ButtonStyle::Success),
              CreateButton::new(cancel_id.as_str())
                .label("Cancel")
                .style(ButtonStyle::Danger),
            ])]),
        )
        .await?;

      // Loop through incoming interactions with the navigation buttons.
      while let Some(press) = ComponentInteractionCollector::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed in one minute.
        .timeout(Duration::from_secs(60))
        .await
      {
        if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
          // This is an unrelated button interaction.
          continue;
        }

        let confirmed = press.data.custom_id == confirm_id;

        if confirmed {
          match press
            .create_response(
              ctx,
              CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                  .content(format!(
                    "{} Event ended. Thank you for hosting!",
                    EMOJI.mminfo
                  ))
                  .ephemeral(true)
                  .embeds(Vec::new())
                  .components(Vec::new()),
              ),
            )
            .await
          {
            Ok(()) => {
              guild_id
                .edit_scheduled_event(
                  ctx,
                  event.id,
                  EditScheduledEvent::new().status(ScheduledEventStatus::Completed),
                )
                .await?;

              let log_message = match event.channel_id {
                Some(channel) => format!(
                  "**Event**: {}\n**Channel:** {}",
                  event.name,
                  channel.mention()
                ),
                None => format!("**Event**: {}\n**Channel:** N/A", event.name),
              };

              let log_embed = BloomBotEmbed::new()
                .title("Event Ended")
                .description(log_message)
                .footer(
                  CreateEmbedFooter::new(format!(
                    "Ended by {} ({})",
                    ctx.author().name,
                    ctx.author().id
                  ))
                  .icon_url(ctx.author().avatar_url().unwrap_or_default()),
                );

              let log_channel = ChannelId::new(CHANNELS.bloomlogs);

              log_channel
                .send_message(ctx, CreateMessage::new().embed(log_embed))
                .await?;

              return Ok(());
            }
            Err(e) => {
              return Err(anyhow!(
                "Failed to end \"{}\" due to error: {e}",
                event.name
              ));
            }
          }
        }

        press
          .create_response(
            ctx,
            CreateInteractionResponse::UpdateMessage(
              CreateInteractionResponseMessage::new()
                .content(format!("{} Cancelled.", EMOJI.mmx))
                .ephemeral(true)
                .embeds(Vec::new())
                .components(Vec::new()),
            ),
          )
          .await?;
      }

      // This happens when the user didn't press any button for 60 seconds.
      return Ok(());
    }
  }

  let msg = format!(
    "{} No active community book club event found.",
    EMOJI.mminfo
  );
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}
