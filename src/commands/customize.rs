use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::time::{self, MinusOffsetChoice, PlusOffsetChoice};
use crate::commands::helpers::tracking::{privacy, Privacy};
use crate::config::{BloomBotEmbed, StreakRoles, EMOJI};
use crate::data::tracking_profile::TrackingProfile;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use log::error;
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::{ChoiceParameter, CreateReply};

#[derive(poise::ChoiceParameter)]
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
  subcommands("show", "offset", "tracking", "streak", "stats"),
  category = "Meditation Tracking",
  //hide_in_help,
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  //let tracking_profile = DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?;
  let tracking_profile =
    match DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await? {
      Some(tracking_profile) => tracking_profile,
      None => TrackingProfile {
        ..Default::default()
      },
    };

  let utc_offset = match time::choice_from_offset(tracking_profile.utc_offset) {
    (Some(minus_offset), None) => minus_offset.name().to_string(),
    (None, Some(plus_offset)) => plus_offset.name().to_string(),
    (None, None) => "UTC".to_string(),
    _ => {
      ctx
          .send(
            CreateReply::default()
                .content(
                  "Matched both plus and minus offsets from the given offset. This should never happen."
                      .to_string(),
                )
                .ephemeral(true),
          )
          .await?;
      return Ok(());
    }
  };

  ctx
    .send(CreateReply::default()
    .embed(BloomBotEmbed::new()
        .author(CreateEmbedAuthor::new("Meditation Tracking Customization Settings").icon_url(ctx.author().face()))
        //.title("Meditation Tracking Customization Settings")
        .description(format!(
          //"**UTC Offset**: {}\n**Anonymous Tracking**: {}\n**Streak Reporting**: {}\n**Streak Visibility**: {}\n**Stats Visibility**: {}",
          "```UTC Offset:           {}\nAnonymous Tracking:   {}\nStreak Reporting:     {}\nStreak Visibility:    {}\nStats Visibility:     {}```",
          //Only show the offset (no time zone abbreviations)
          utc_offset.split_whitespace().next().with_context(|| "Failed to retrieve offset portion of time zone choice")?,
          if tracking_profile.anonymous_tracking { "On" } else { "Off" },
          if tracking_profile.streaks_active { "On" } else { "Off" },
          if tracking_profile.streaks_private { "Private" } else { "Public" },
          if tracking_profile.stats_private { "Private" } else { "Public" },
        ))
    )
    .ephemeral(true))
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let choice_offset = time::offset_from_choice(minus_offset, plus_offset, 0);
  let Ok(utc_offset) = choice_offset else {
    ctx
      .send(
        CreateReply::default()
          .content("Cannot determine UTC offset based on the choice selected.".to_string())
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  if let Some(tracking_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    let existing_profile = tracking_profile;

    if utc_offset == existing_profile.utc_offset {
      ctx
        .send(
          CreateReply::default()
            .content(
              "Your current UTC offset already matches the specified offset. No changes made."
                .to_string(),
            )
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      utc_offset,
      existing_profile.anonymous_tracking,
      existing_profile.streaks_active,
      existing_profile.streaks_private,
      existing_profile.stats_private,
    )
    .await?;
  } else {
    let default = TrackingProfile {
      ..Default::default()
    };

    DatabaseHandler::create_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      utc_offset,
      default.anonymous_tracking,
      default.streaks_active,
      default.streaks_private,
      default.stats_private,
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
    true,
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let anonymous_tracking = match anonymous {
    OnOff::On => true,
    OnOff::Off => false,
  };

  if let Some(tracking_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    let existing_profile = tracking_profile;

    if anonymous_tracking == existing_profile.anonymous_tracking {
      ctx
        .send(
          CreateReply::default()
            .content(format!(
              "Anonymous tracking already turned **{}**. No changes made.",
              anonymous.name()
            ))
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      existing_profile.utc_offset,
      anonymous_tracking,
      existing_profile.streaks_active,
      existing_profile.streaks_private,
      existing_profile.stats_private,
    )
    .await?;
  } else {
    let default = TrackingProfile {
      ..Default::default()
    };

    DatabaseHandler::create_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      default.utc_offset,
      anonymous_tracking,
      default.streaks_active,
      default.streaks_private,
      default.stats_private,
    )
    .await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Anonymous tracking successfully turned **{}**.",
      EMOJI.mmcheck,
      anonymous.name()
    )),
    true,
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
  #[description = "Turn streak reporting on or off (Defaults to on)"] reporting: Option<OnOff>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  if let Some(tracking_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    let existing_profile = tracking_profile;

    let streaks_active = match reporting {
      Some(reporting) => match reporting {
        OnOff::On => true,
        OnOff::Off => false,
      },
      None => existing_profile.streaks_active,
    };

    let streaks_private = privacy!(privacy, existing_profile.streaks_private);

    if (streaks_active == existing_profile.streaks_active)
      && (streaks_private == existing_profile.streaks_private)
    {
      ctx
        .send(
          CreateReply::default()
            .content(
              "Current settings already match specified settings. No changes made.".to_string(),
            )
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      existing_profile.utc_offset,
      existing_profile.anonymous_tracking,
      streaks_active,
      streaks_private,
      existing_profile.stats_private,
    )
    .await?;

    if existing_profile.streaks_active && !streaks_active {
      let member = guild_id.member(ctx, user_id).await?;

      let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);

      for role in current_streak_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");

            ctx.send(CreateReply::default()
              .content(format!("{} An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;
          }
        }
      }
    }

    if !existing_profile.streaks_active && streaks_active {
      let user_streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;

      let member = guild_id.member(ctx, user_id).await?;

      let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);
      #[allow(clippy::cast_sign_loss)]
      let earned_streak_role = StreakRoles::from_streak(user_streak.current as u64);

      if let Some(earned_streak_role) = earned_streak_role {
        if !current_streak_roles.contains(&earned_streak_role.to_role_id()) {
          match member.add_role(ctx, earned_streak_role.to_role_id()).await {
            Ok(()) => {}
            Err(err) => {
              error!("Error adding role: {err}");

              ctx.send(CreateReply::default()
                .content(format!("{} An error occured while adding your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;
            }
          }
        }
      }
    }
  } else {
    let default = TrackingProfile {
      ..Default::default()
    };

    let streaks_active = match reporting {
      Some(reporting) => match reporting {
        OnOff::On => true,
        OnOff::Off => false,
      },
      None => default.streaks_active,
    };

    let streaks_private = privacy!(privacy, default.streaks_private);

    DatabaseHandler::create_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      default.utc_offset,
      default.anonymous_tracking,
      streaks_active,
      streaks_private,
      default.stats_private,
    )
    .await?;

    if default.streaks_active && !streaks_active {
      let member = guild_id.member(ctx, user_id).await?;

      let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);

      for role in current_streak_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");

            ctx.send(CreateReply::default()
              .content(format!("{} An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;
          }
        }
      }
    }

    if !default.streaks_active && streaks_active {
      let user_streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;

      let member = guild_id.member(ctx, user_id).await?;

      let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);
      #[allow(clippy::cast_sign_loss)]
      let earned_streak_role = StreakRoles::from_streak(user_streak.current as u64);

      if let Some(earned_streak_role) = earned_streak_role {
        if !current_streak_roles.contains(&earned_streak_role.to_role_id()) {
          match member.add_role(ctx, earned_streak_role.to_role_id()).await {
            Ok(()) => {}
            Err(err) => {
              error!("Error adding role: {err}");

              ctx.send(CreateReply::default()
                .content(format!("{} An error occured while adding your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;
            }
          }
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
    true,
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let stats_private = privacy!(privacy);

  if let Some(tracking_profile) =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
  {
    let existing_profile = tracking_profile;

    if stats_private == existing_profile.stats_private {
      ctx
        .send(
          CreateReply::default()
            .content(format!(
              "Stats already set to **{}**. No changes made.",
              privacy.name()
            ))
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }

    DatabaseHandler::update_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      existing_profile.utc_offset,
      existing_profile.anonymous_tracking,
      existing_profile.streaks_active,
      existing_profile.streaks_private,
      stats_private,
    )
    .await?;
  } else {
    let default = TrackingProfile {
      ..Default::default()
    };

    DatabaseHandler::create_tracking_profile(
      &mut transaction,
      &guild_id,
      &user_id,
      default.utc_offset,
      default.anonymous_tracking,
      default.streaks_active,
      default.streaks_private,
      stats_private,
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
    true,
  )
  .await?;

  Ok(())
}
