use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::{Context, Data as AppData, Error as AppError};
use anyhow::{Context as AnyhowContext, Result};
use chrono::Duration;
use poise::serenity_prelude::{
  self as serenity, builder::*, FormattedTimestamp, FormattedTimestampStyle, ScheduledEventStatus,
};
use poise::CreateReply;

async fn is_coleader(ctx: Context<'_>) -> Result<bool> {
  let community_sit_coleader = serenity::RoleId::from(1285275266549158050);
  let has_role = match ctx.author_member().await {
    Some(member) => member.roles.contains(&community_sit_coleader),
    None => false,
  };

  Ok(has_role)
}

/// Manage community sit events
///
/// Commands for managing community sit events. Requires Community Sit Co-Leader role.
#[poise::command(
  slash_command,
  check = "is_coleader",
  category = "Secret",
  rename = "communitysit",
  subcommands("start", "end"),
  subcommand_required,
  hide_in_help,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn community_sit(_: poise::Context<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// Start a community sit event
///
/// Starts a scheduled community sit event.
#[poise::command(slash_command, check = "is_coleader")]
pub async fn start(ctx: Context<'_>) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let events = guild_id.scheduled_events(ctx, false).await?;
  for event in events {
    if event.name.as_str().ends_with("Silent Sit")
      && event.status == ScheduledEventStatus::Scheduled
      && event.start_time.to_utc() - chrono::Utc::now() < Duration::minutes(15)
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
              CreateButton::new(confirm_id.clone())
                .label("Start Event")
                .style(serenity::ButtonStyle::Success),
              CreateButton::new(cancel_id.clone())
                .label("Cancel")
                .style(serenity::ButtonStyle::Danger),
            ])]),
        )
        .await?;

      // Loop through incoming interactions with the navigation buttons
      while let Some(press) = serenity::ComponentInteractionCollector::new(ctx)
        // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
        // button was pressed
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed in one minute
        .timeout(std::time::Duration::from_secs(60))
        .await
      {
        if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
          // This is an unrelated button interaction
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
                    "{} Event started. May your practice be fruitful!",
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
                  EditScheduledEvent::new().status(ScheduledEventStatus::Active),
                )
                .await?;

              let log_embed = BloomBotEmbed::new()
                .title("Event Started")
                .description(format!("**Event**: {}", event.name))
                .footer(
                  CreateEmbedFooter::new(format!(
                    "Started by {} ({})",
                    ctx.author().name,
                    ctx.author().id
                  ))
                  .icon_url(ctx.author().avatar_url().unwrap_or_default()),
                )
                .clone();

              let log_channel = serenity::ChannelId::new(CHANNELS.bloomlogs);

              log_channel
                .send_message(ctx, CreateMessage::new().embed(log_embed))
                .await?;

              return Ok(());
            }
            Err(e) => {
              return Err(anyhow::anyhow!(
                "Failed to start \"{}\" due to error: {}",
                event.name,
                e
              ));
            }
          }
        }

        press
          .create_response(
            ctx,
            CreateInteractionResponse::UpdateMessage(
              CreateInteractionResponseMessage::new()
                .content("Cancelled.")
                .ephemeral(true)
                .embeds(Vec::new())
                .components(Vec::new()),
            ),
          )
          .await?;
      }

      // This happens when the user didn't press any button for 60 seconds
      return Ok(());
    }
  }

  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "{} No eligible community sit event found. Please try again within 15 minutes of starting time.",
          EMOJI.mminfo
        ))
        .ephemeral(true),
    )
    .await?;

  Ok(())
}

/// End a community sit event
///
/// Ends an active community sit event.
#[poise::command(slash_command, check = "is_coleader")]
pub async fn end(ctx: Context<'_>) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let events = guild_id.scheduled_events(ctx, false).await?;
  for event in events {
    if event.name.as_str().ends_with("Silent Sit") && event.status == ScheduledEventStatus::Active {
      let ctx_id = ctx.id();
      let confirm_id = format!("{ctx_id}confirm");
      let cancel_id = format!("{ctx_id}cancel");

      ctx
        .send(
          CreateReply::default()
            .content(format!("Are you sure you want to end **{}**?", event.name))
            .ephemeral(true)
            .components(vec![CreateActionRow::Buttons(vec![
              CreateButton::new(confirm_id.clone())
                .label("End Event")
                .style(serenity::ButtonStyle::Success),
              CreateButton::new(cancel_id.clone())
                .label("Cancel")
                .style(serenity::ButtonStyle::Danger),
            ])]),
        )
        .await?;

      // Loop through incoming interactions with the navigation buttons
      while let Some(press) = serenity::ComponentInteractionCollector::new(ctx)
        // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
        // button was pressed
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        // Timeout when no navigation button has been pressed in one minute
        .timeout(std::time::Duration::from_secs(60))
        .await
      {
        if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
          // This is an unrelated button interaction
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
                    "{} Event ended. Thank you for your assistance with the community sits!",
                    EMOJI.mminfo
                  ))
                  .ephemeral(true)
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

              let log_embed = BloomBotEmbed::new()
                .title("Event Ended")
                .description(format!("**Event**: {}", event.name))
                .footer(
                  CreateEmbedFooter::new(format!(
                    "Ended by {} ({})",
                    ctx.author().name,
                    ctx.author().id
                  ))
                  .icon_url(ctx.author().avatar_url().unwrap_or_default()),
                )
                .clone();

              let log_channel = serenity::ChannelId::new(CHANNELS.bloomlogs);

              log_channel
                .send_message(ctx, CreateMessage::new().embed(log_embed))
                .await?;

              return Ok(());
            }
            Err(e) => {
              return Err(anyhow::anyhow!(
                "Failed to end \"{}\" due to error: {}",
                event.name,
                e
              ));
            }
          }
        }

        press
          .create_response(
            ctx,
            CreateInteractionResponse::UpdateMessage(
              CreateInteractionResponseMessage::new()
                .content("Cancelled.")
                .ephemeral(true)
                .components(Vec::new()),
            ),
          )
          .await?;
      }

      // This happens when the user didn't press any button for 60 seconds
      return Ok(());
    }
  }

  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "{} No eligible community sit event found. Please try again within 15 minutes of starting time.",
          EMOJI.mminfo
        ))
        .ephemeral(true),
    )
    .await?;

  Ok(())
}
