use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::CreateInteractionResponseMessage;
use poise::serenity_prelude::{ButtonStyle, ComponentInteractionCollector, CreateActionRow};
use poise::serenity_prelude::{CreateButton, CreateEmbedAuthor, CreateInteractionResponse};
use poise::{ChoiceParameter, CreateReply};
use tracing::error;

use crate::Context;
use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::time::{self, MinusOffsetChoice, PlusOffsetChoice};
use crate::config::{BloomBotEmbed, EMOJI, StreakRoles};
use crate::data::tracking_profile::{Privacy, PrivateNotifications, Status, TrackingProfile};
use crate::database::DatabaseHandler;

#[derive(ChoiceParameter)]
enum OnOff {
  #[name = "on"]
  On,
  #[name = "off"]
  Off,
}

/// Customize your tracking experience
///
/// Customize your meditation tracking experience.
///
/// Set a UTC offset, make your stats or streak private, turn streak reporting off, or enable anonymous tracking.
#[poise::command(
  slash_command,
  subcommands("show", "offset", "tracking", "streak", "stats", "vc"),
  category = "Meditation Tracking",
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn customize(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Show your current customization settings
///
/// Show your current settings for meditation tracking experience customization.
#[poise::command(slash_command)]
async fn show(ctx: Context<'_>) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let tracking_profile =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id)
      .await?
      .unwrap_or_default();

  ctx
    .send(
      CreateReply::default()
        .embed(
          BloomBotEmbed::new()
            .author(
              CreateEmbedAuthor::new("Meditation Tracking Customization Settings")
                .icon_url(ctx.author().face()),
            )
            .description(format!(
              "```UTC Offset:          {}\nAnonymous Tracking:  {}\nStreak Reporting:    {}\nStreak Visibility:   {}\nStats Visibility:    {}\n\nVC Tracking:         {}\n → Notifications:    {}```",
              time::name_from_offset(tracking_profile.utc_offset)?,
              if matches!(tracking_profile.tracking.privacy, Privacy::Private) { "On" } else { "Off" },
              if matches!(tracking_profile.streak.status, Status::Enabled) { "Enabled" } else { "Disabled" },
              if matches!(tracking_profile.streak.privacy, Privacy::Private) { "Private" } else { "Public" },
              if matches!(tracking_profile.stats.privacy, Privacy::Private) { "Private" } else { "Public" },
              if let Some(vc_tracking) = tracking_profile.vc_tracking { if matches!(vc_tracking, Status::Enabled) { "Enabled" } else { "Disabled" } } else { "***" },
              match tracking_profile.notifications {
                PrivateNotifications::DirectMessage => "DM",
                PrivateNotifications::PrivateThread => "Thread",
                PrivateNotifications::Disabled => "Disabled",
              },
            )),
        )
        .ephemeral(true),
    )
    .await?;

  Ok(())
}

/// Set a UTC offset to be used for tracking
///
/// Set a UTC offset to be used for tracking. Times will be adjusted to your local time. Note that daylight savings time adjustments will need to be made manually, if necessary.
#[poise::command(slash_command)]
async fn offset(
  ctx: Context<'_>,
  #[description = "Specify a UTC offset for a Western Hemisphere time zone"]
  #[rename = "western_hemisphere_offset"]
  minus_offset: Option<MinusOffsetChoice>,
  #[description = "Specify a UTC offset for an Eastern Hemisphere time zone"]
  #[rename = "eastern_hemisphere_offset"]
  plus_offset: Option<PlusOffsetChoice>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let utc_offset = match time::offset_from_choice(minus_offset, plus_offset, 0) {
    Ok(offset) => offset,
    Err(e) => {
      let msg = format!(
        "{} Unable to determine UTC offset based on your choice: {e}",
        EMOJI.mminfo
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }
  };

  if let Some(existing_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    if utc_offset == existing_profile.utc_offset {
      let msg = "Your current UTC offset already matches the specified offset. No changes made.";
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &existing_profile.with_offset(utc_offset),
    )
    .await?;
  } else {
    DatabaseHandler::add_tracking_profile(
      &mut transaction,
      &TrackingProfile::new(guild_id, user_id).with_offset(utc_offset),
    )
    .await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} UTC offset successfully updated.",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Turn anonymous tracking on or off
///
/// Turn anonymous tracking on or off.
///
/// When anonymous tracking is turned on, the anonymous entry is displayed in the channel to motivate others, but personal information (total meditation time, streak and role info) is shared with you privately via ephemeral messages.
#[poise::command(slash_command)]
async fn tracking(
  ctx: Context<'_>,
  #[description = "Turn anonymous tracking on or off (Default is off)"] anonymous: OnOff,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let tracking_privacy = match anonymous {
    OnOff::On => Privacy::Private,
    OnOff::Off => Privacy::Public,
  };

  if let Some(existing_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    if tracking_privacy == existing_profile.tracking.privacy {
      let msg = format!(
        "Anonymous tracking already turned **{}**. No changes made.",
        anonymous.name()
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }

    let change_all_privacy = change_all_privacy(ctx, &anonymous).await.unwrap_or(true);

    let tracking_profile = if change_all_privacy {
      &existing_profile
        .with_tracking_privacy(tracking_privacy)
        .with_stats_privacy(tracking_privacy)
        .with_streak_privacy(tracking_privacy)
    } else {
      &existing_profile.with_tracking_privacy(tracking_privacy)
    };

    DatabaseHandler::update_tracking_profile(&mut transaction, tracking_profile).await?;
  } else {
    let change_all_privacy = change_all_privacy(ctx, &anonymous).await.unwrap_or(true);

    let tracking_profile = if change_all_privacy {
      &TrackingProfile::new(guild_id, user_id)
        .with_tracking_privacy(tracking_privacy)
        .with_stats_privacy(tracking_privacy)
        .with_streak_privacy(tracking_privacy)
    } else {
      &TrackingProfile::new(guild_id, user_id).with_tracking_privacy(tracking_privacy)
    };

    DatabaseHandler::add_tracking_profile(&mut transaction, tracking_profile).await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Anonymous tracking successfully turned **{}**.",
      EMOJI.mmcheck,
      anonymous.name()
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Enable/disable streaks or set streak privacy
///
/// Enable/disable streak reporting or set your streak privacy.
///
/// Streak reporting is enabled by default. When disabled, any existing streak role will be removed and you will no longer receive streak-related notifications when adding time. Your streak will also be hidden from your stats. However, your streak status will still be tracked and you will still be able to check your current streak using the /streak command.
///
/// When streaks are set to private, other members will be unable to view your streak using the /streak command. When you view your own streak using the /streak command, the response will be shown privately in an ephemeral message by default. This can be overridden by setting privacy to "public" when using the command.
#[poise::command(slash_command)]
async fn streak(
  ctx: Context<'_>,
  #[description = "Set streak privacy (Defaults to public)"] privacy: Option<Privacy>,
  #[description = "Turn streak reporting on or off (Defaults to on)"] reporting: Option<Status>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(existing_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    let streak_status = reporting.unwrap_or(existing_profile.streak.status);
    let streak_privacy = privacy.unwrap_or(existing_profile.streak.privacy);

    if (streak_status == existing_profile.streak.status)
      && (streak_privacy == existing_profile.streak.privacy)
    {
      let msg = "Current settings already match specified settings. No changes made.";
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }

    let streak_disabled =
      existing_profile.streak.status == Status::Enabled && streak_status == Status::Disabled;
    let streak_enabled =
      existing_profile.streak.status == Status::Disabled && streak_status == Status::Enabled;

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &existing_profile
        .with_streak_status(streak_status)
        .with_streak_privacy(streak_privacy),
    )
    .await?;

    if streak_disabled {
      let member = guild_id.member(ctx, user_id).await?;
      let current_streak_roles = StreakRoles::current(&member.roles);

      for role in current_streak_roles {
        if let Err(err) = member.remove_role(ctx, role).await {
          error!("Error removing role: {err}");
          let msg = format!(
            "{} An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.",
            EMOJI.mminfo
          );
          ctx
            .send(CreateReply::default().content(msg).ephemeral(true))
            .await?;
        }
      }
    }

    if streak_enabled {
      let user_streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;
      let member = guild_id.member(ctx, user_id).await?;
      let current_streak_roles = StreakRoles::current(&member.roles);
      let earned_streak_role = StreakRoles::from_streak(user_streak.current.unsigned_abs().into());

      if let Some(earned_streak_role) = earned_streak_role {
        if !current_streak_roles.contains(&earned_streak_role.to_role_id()) {
          if let Err(err) = member.add_role(ctx, earned_streak_role.to_role_id()).await {
            error!("Error adding role: {err}");
            let msg = format!(
              "{} An error occured while adding your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.",
              EMOJI.mminfo
            );
            ctx
              .send(CreateReply::default().content(msg).ephemeral(true))
              .await?;
          }
        }
      }
    }
  } else {
    let streak_status = reporting.unwrap_or_default();
    let streak_privacy = privacy.unwrap_or_default();

    DatabaseHandler::add_tracking_profile(
      &mut transaction,
      &TrackingProfile::new(guild_id, user_id)
        .with_streak_status(streak_status)
        .with_streak_privacy(streak_privacy),
    )
    .await?;

    if streak_status == Status::Disabled {
      let member = guild_id.member(ctx, user_id).await?;
      let current_streak_roles = StreakRoles::current(&member.roles);

      for role in current_streak_roles {
        if let Err(err) = member.remove_role(ctx, role).await {
          error!("Error removing role: {err}");
          let msg = format!(
            "{} An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.",
            EMOJI.mminfo
          );
          ctx
            .send(CreateReply::default().content(msg).ephemeral(true))
            .await?;
        }
      }
    }
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Streak settings successfully updated.",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Set stats privacy
///
/// Set your stats privacy.
///
/// When stats are set to private, other members will be unable to view your stats using the /stats user command. When you view your own stats using the /stats user command, the response will be shown privately in an ephemeral message by default. This can be overridden by setting privacy to "public" when using the command.
#[poise::command(slash_command)]
async fn stats(
  ctx: Context<'_>,
  #[description = "Set stats privacy (Defaults to public)"] privacy: Privacy,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(existing_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    if privacy == existing_profile.stats.privacy {
      let msg = format!(
        "Stats already set to **{}**. No changes made.",
        privacy.name()
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &existing_profile.with_stats_privacy(privacy),
    )
    .await?;
  } else {
    DatabaseHandler::add_tracking_profile(
      &mut transaction,
      &TrackingProfile::new(guild_id, user_id).with_stats_privacy(privacy),
    )
    .await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Stats successfully set to **{}**.",
      EMOJI.mmcheck,
      privacy.name()
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Enable/disable automatic VC tracking
///
/// Enable/disable automatic tracking of time spent in meditation voice channels.
///
/// When enabled, any amount of time greater than or equal to five minutes will automatically be added to your tracked time upon leaving the VC.
///
/// Privacy settings are honored, and private stats updates are sent via private thread, by default. Optionally, choose to receive them via DM, or disable them.
///
/// Disabling VC tracking will prevent Bloom from asking if you would like to track eligible times spent in meditation VCs.
#[poise::command(slash_command)]
async fn vc(
  ctx: Context<'_>,
  #[description = "Turn VC tracking on or off"] tracking: Status,
  #[description = "Private notifications for VC tracking (Defaults to private thread)"]
  notifications: Option<PrivateNotifications>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(existing_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    if existing_profile
      .vc_tracking
      .is_some_and(|vc_tracking| vc_tracking == tracking)
      && notifications.is_none_or(|n| n == existing_profile.notifications)
    {
      let msg = "Current settings already match specified settings. No changes made.";
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }
    let notifications = notifications.unwrap_or(existing_profile.notifications);
    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &existing_profile
        .with_vc_tracking(tracking)
        .with_notifications(notifications),
    )
    .await?;
  } else {
    DatabaseHandler::add_tracking_profile(
      &mut transaction,
      &TrackingProfile::new(guild_id, user_id)
        .with_vc_tracking(tracking)
        .with_notifications(notifications.unwrap_or(PrivateNotifications::PrivateThread)),
    )
    .await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} VC tracking settings successfully updated.",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

async fn change_all_privacy(ctx: Context<'_>, anonymous: &OnOff) -> Result<bool> {
  let ctx_id = ctx.id();
  let confirm_id = format!("{ctx_id}confirm");
  let cancel_id = format!("{ctx_id}cancel");

  let check = ctx
    .send(
      CreateReply::default()
        .content(format!(
          "You are about to {} anonymous tracking. \
          Would you like to set your stats and streak to {} as well?",
          match anonymous {
            OnOff::On => "enable",
            OnOff::Off => "disable",
          },
          match anonymous {
            OnOff::On => "private",
            OnOff::Off => "public",
          }
        ))
        .ephemeral(true)
        .components(vec![CreateActionRow::Buttons(vec![
          CreateButton::new(confirm_id.as_str())
            .label("Yes")
            .style(ButtonStyle::Success),
          CreateButton::new(cancel_id.as_str())
            .label("No")
            .style(ButtonStyle::Danger),
        ])]),
    )
    .await?;

  while let Some(press) = ComponentInteractionCollector::new(ctx)
    .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
    .timeout(Duration::from_secs(60))
    .await
  {
    if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
      // This is an unrelated button interaction.
      continue;
    }

    if press.data.custom_id == confirm_id {
      let msg = CreateInteractionResponseMessage::new()
        .content(format!(
          "{} Updating stats and streak privacy.",
          EMOJI.mmcheck
        ))
        .components(Vec::new());
      press
        .create_response(ctx, CreateInteractionResponse::UpdateMessage(msg))
        .await?;
      return Ok(true);
    }

    if press.data.custom_id == cancel_id {
      let msg = CreateInteractionResponseMessage::new()
        .content(format!(
          "{} Stats and streak privacy will remain unchanged.",
          EMOJI.mmcheck
        ))
        .components(Vec::new());
      press
        .create_response(ctx, CreateInteractionResponse::UpdateMessage(msg))
        .await?;
      return Ok(false);
    }
  }

  let reply = CreateReply::default()
    .content(format!(
      "{} No response received. Updating stats and streak privacy.",
      EMOJI.mminfo
    ))
    .components(Vec::new());
  check.edit(ctx, reply).await?;

  Ok(true)
}
