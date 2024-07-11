use crate::commands::{commit_and_say, MessageType};
use crate::config::{BloomBotEmbed, StreakRoles};
use crate::database::{DatabaseHandler, TrackingProfile};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use log::error;
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::{ChoiceParameter, CreateReply};

#[derive(poise::ChoiceParameter)]
pub enum MinusOffsetChoices {
  #[name = "UTC-12 (BIT)"]
  UTCMinus12,
  #[name = "UTC-11 (NUT, SST)"]
  UTCMinus11,
  #[name = "UTC-10 (CKT, HAST, HST, TAHT)"]
  UTCMinus10,
  #[name = "UTC-9:30 (MART, MIT)"]
  UTCMinus9_30,
  #[name = "UTC-9 (AKST, GAMT, GIT, HADT)"]
  UTCMinus9,
  #[name = "UTC-8 (AKDT, CIST, PST)"]
  UTCMinus8,
  #[name = "UTC-7 (MST, PDT)"]
  UTCMinus7,
  #[name = "UTC-6 (CST, EAST, GALT, MDT)"]
  UTCMinus6,
  #[name = "UTC-5 (ACT, CDT, COT, CST, EASST, ECT, EST, PET)"]
  UTCMinus5,
  #[name = "UTC-4:30 (VET)"]
  UTCMinus4_30,
  #[name = "UTC-4 (AMT, AST, BOT, CDT, CLT, COST, ECT, EDT, FKT, GYT, PYT)"]
  UTCMinus4,
  #[name = "UTC-3:30 (NST, NT)"]
  UTCMinus3_30,
  #[name = "UTC-3 (ADT, AMST, ART, BRT, CLST, FKST, GFT, PMST, PYST, ROTT, SRT, UYT)"]
  UTCMinus3,
  #[name = "UTC-2:30 (NDT)"]
  UTCMinus2_30,
  #[name = "UTC-2 (BRST, FNT, GST, PMDT, UYST)"]
  UTCMinus2,
  #[name = "UTC-1 (AZOST, CVT, EGT)"]
  UTCMinus1,
}

#[derive(poise::ChoiceParameter)]
pub enum PlusOffsetChoices {
  #[name = "UTC+1 (BST, CET, IST, WAT, WEST)"]
  UTCPlus1,
  #[name = "UTC+2 (CAT, CEST, EET, IST, SAST, WAST)"]
  UTCPlus2,
  #[name = "UTC+3 (AST, EAT, EEST, FET, IDT, IOT, MSK, USZ1)"]
  UTCPlus3,
  #[name = "UTC+3:30 (IRST)"]
  UTCPlus3_30,
  #[name = "UTC+4 (AMT, AZT, GET, GST, MUT, RET, SAMT, SCT, VOLT)"]
  UTCPlus4,
  #[name = "UTC+4:30 (AFT, IRDT)"]
  UTCPlus4_30,
  #[name = "UTC+5 (HMT, MAWT, MVT, ORAT, PKT, TFT, TJT, TMT, UZT, YEKT)"]
  UTCPlus5,
  #[name = "UTC+5:30 (IST, SLST)"]
  UTCPlus5_30,
  #[name = "UTC+5:45 (NPT)"]
  UTCPlus5_45,
  #[name = "UTC+6 (BDT, BIOT, BST, BTT, KGT, OMST, VOST)"]
  UTCPlus6,
  #[name = "UTC+6:30 (CCT, MMT, MST)"]
  UTCPlus6_30,
  #[name = "UTC+7 (CXT, DAVT, HOVT, ICT, KRAT, THA, WIT)"]
  UTCPlus7,
  #[name = "UTC+8 (ACT, AWST, BDT, CHOT, CIT, CST, CT, HKT, IRKT, MST, MYT, PST, SGT, SST, ULAT, WST)"]
  UTCPlus8,
  #[name = "UTC+8:45 (CWST)"]
  UTCPlus8_45,
  #[name = "UTC+9 (AWDT, EIT, JST, KST, TLT, YAKT)"]
  UTCPlus9,
  #[name = "UTC+9:30 (ACST, CST)"]
  UTCPlus9_30,
  #[name = "UTC+10 (AEST, ChST, CHUT, DDUT, EST, PGT, VLAT)"]
  UTCPlus10,
  #[name = "UTC+10:30 (ACDT, CST, LHST)"]
  UTCPlus10_30,
  #[name = "UTC+11 (AEDT, BST, KOST, LHST, MIST, NCT, PONT, SAKT, SBT, SRET, VUT, NFT)"]
  UTCPlus11,
  #[name = "UTC+12 (FJT, GILT, MAGT, MHT, NZST, PETT, TVT, WAKT)"]
  UTCPlus12,
  #[name = "UTC+12:45 (CHAST)"]
  UTCPlus12_45,
  #[name = "UTC+13 (NZDT, PHOT, TKT, TOT)"]
  UTCPlus13,
  #[name = "UTC+13:45 (CHADT)"]
  UTCPlus13_45,
  #[name = "UTC+14 (LINT)"]
  UTCPlus14,
}

#[derive(poise::ChoiceParameter)]
pub enum Privacy {
  #[name = "private"]
  Private,
  #[name = "public"]
  Public,
}

#[derive(poise::ChoiceParameter)]
pub enum OnOff {
  #[name = "on"]
  On,
  #[name = "off"]
  Off,
}

/// Customize your meditation tracking experience
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
pub async fn show(ctx: Context<'_>) -> Result<()> {
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

  let utc_offset = match tracking_profile.utc_offset {
    -720 => MinusOffsetChoices::UTCMinus12.name(),
    -660 => MinusOffsetChoices::UTCMinus11.name(),
    -600 => MinusOffsetChoices::UTCMinus10.name(),
    -570 => MinusOffsetChoices::UTCMinus9_30.name(),
    -540 => MinusOffsetChoices::UTCMinus9.name(),
    -480 => MinusOffsetChoices::UTCMinus8.name(),
    -420 => MinusOffsetChoices::UTCMinus7.name(),
    -360 => MinusOffsetChoices::UTCMinus6.name(),
    -300 => MinusOffsetChoices::UTCMinus5.name(),
    -270 => MinusOffsetChoices::UTCMinus4_30.name(),
    -240 => MinusOffsetChoices::UTCMinus4.name(),
    -210 => MinusOffsetChoices::UTCMinus3_30.name(),
    -180 => MinusOffsetChoices::UTCMinus3.name(),
    -150 => MinusOffsetChoices::UTCMinus2_30.name(),
    -120 => MinusOffsetChoices::UTCMinus2.name(),
    -60 => MinusOffsetChoices::UTCMinus1.name(),
    60 => PlusOffsetChoices::UTCPlus1.name(),
    120 => PlusOffsetChoices::UTCPlus2.name(),
    180 => PlusOffsetChoices::UTCPlus3.name(),
    210 => PlusOffsetChoices::UTCPlus3_30.name(),
    240 => PlusOffsetChoices::UTCPlus4.name(),
    270 => PlusOffsetChoices::UTCPlus4_30.name(),
    300 => PlusOffsetChoices::UTCPlus5.name(),
    330 => PlusOffsetChoices::UTCPlus5_30.name(),
    345 => PlusOffsetChoices::UTCPlus5_45.name(),
    360 => PlusOffsetChoices::UTCPlus6.name(),
    390 => PlusOffsetChoices::UTCPlus6_30.name(),
    420 => PlusOffsetChoices::UTCPlus7.name(),
    480 => PlusOffsetChoices::UTCPlus8.name(),
    525 => PlusOffsetChoices::UTCPlus8_45.name(),
    540 => PlusOffsetChoices::UTCPlus9.name(),
    570 => PlusOffsetChoices::UTCPlus9_30.name(),
    600 => PlusOffsetChoices::UTCPlus10.name(),
    630 => PlusOffsetChoices::UTCPlus10_30.name(),
    660 => PlusOffsetChoices::UTCPlus11.name(),
    720 => PlusOffsetChoices::UTCPlus12.name(),
    765 => PlusOffsetChoices::UTCPlus12_45.name(),
    780 => PlusOffsetChoices::UTCPlus13.name(),
    825 => PlusOffsetChoices::UTCPlus13_45.name(),
    840 => PlusOffsetChoices::UTCPlus14.name(),
    _ => "None",
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
pub async fn offset(
  ctx: Context<'_>,
  #[description = "Specify a UTC offset for a Western Hemisphere time zone"]
  #[rename = "western_hemisphere_offset"]
  minus_offset: Option<MinusOffsetChoices>,
  #[description = "Specify a UTC offset for an Eastern Hemisphere time zone"]
  #[rename = "eastern_hemisphere_offset"]
  plus_offset: Option<PlusOffsetChoices>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let minus_offset = match minus_offset {
    Some(minus_offset) => match minus_offset {
      MinusOffsetChoices::UTCMinus12 => -720,
      MinusOffsetChoices::UTCMinus11 => -660,
      MinusOffsetChoices::UTCMinus10 => -600,
      MinusOffsetChoices::UTCMinus9_30 => -570,
      MinusOffsetChoices::UTCMinus9 => -540,
      MinusOffsetChoices::UTCMinus8 => -480,
      MinusOffsetChoices::UTCMinus7 => -420,
      MinusOffsetChoices::UTCMinus6 => -360,
      MinusOffsetChoices::UTCMinus5 => -300,
      MinusOffsetChoices::UTCMinus4_30 => -270,
      MinusOffsetChoices::UTCMinus4 => -240,
      MinusOffsetChoices::UTCMinus3_30 => -210,
      MinusOffsetChoices::UTCMinus3 => -180,
      MinusOffsetChoices::UTCMinus2_30 => -150,
      MinusOffsetChoices::UTCMinus2 => -120,
      MinusOffsetChoices::UTCMinus1 => -60,
    },
    None => 0,
  };

  let plus_offset = match plus_offset {
    Some(plus_offset) => match plus_offset {
      PlusOffsetChoices::UTCPlus1 => 60,
      PlusOffsetChoices::UTCPlus2 => 120,
      PlusOffsetChoices::UTCPlus3 => 180,
      PlusOffsetChoices::UTCPlus3_30 => 210,
      PlusOffsetChoices::UTCPlus4 => 240,
      PlusOffsetChoices::UTCPlus4_30 => 270,
      PlusOffsetChoices::UTCPlus5 => 300,
      PlusOffsetChoices::UTCPlus5_30 => 330,
      PlusOffsetChoices::UTCPlus5_45 => 345,
      PlusOffsetChoices::UTCPlus6 => 360,
      PlusOffsetChoices::UTCPlus6_30 => 390,
      PlusOffsetChoices::UTCPlus7 => 420,
      PlusOffsetChoices::UTCPlus8 => 480,
      PlusOffsetChoices::UTCPlus8_45 => 525,
      PlusOffsetChoices::UTCPlus9 => 540,
      PlusOffsetChoices::UTCPlus9_30 => 570,
      PlusOffsetChoices::UTCPlus10 => 600,
      PlusOffsetChoices::UTCPlus10_30 => 630,
      PlusOffsetChoices::UTCPlus11 => 660,
      PlusOffsetChoices::UTCPlus12 => 720,
      PlusOffsetChoices::UTCPlus12_45 => 765,
      PlusOffsetChoices::UTCPlus13 => 780,
      PlusOffsetChoices::UTCPlus13_45 => 825,
      PlusOffsetChoices::UTCPlus14 => 840,
    },
    None => 0,
  };

  if minus_offset != 0 && plus_offset != 0 {
    ctx
      .send(
        CreateReply::default()
          .content(
            "Cannot specify multiple time zones. Please try again with only one offset."
              .to_string(),
          )
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let utc_offset = if minus_offset != 0 {
    minus_offset
  } else {
    plus_offset
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

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: UTC offset successfully updated.".to_string()),
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
pub async fn tracking(
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

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      ":white_check_mark: Anonymous tracking successfully turned **{}**.",
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
pub async fn streak(
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

    let streaks_private = match privacy {
      Some(privacy) => match privacy {
        Privacy::Private => true,
        Privacy::Public => false,
      },
      None => existing_profile.streaks_private,
    };

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
              .content(":x: An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.")
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
      let earned_streak_role = StreakRoles::from_streak(user_streak);

      if let Some(earned_streak_role) = earned_streak_role {
        if !current_streak_roles.contains(&earned_streak_role.to_role_id()) {
          match member.add_role(ctx, earned_streak_role.to_role_id()).await {
            Ok(()) => {}
            Err(err) => {
              error!("Error adding role: {err}");

              ctx.send(CreateReply::default()
                .content(":x: An error occured while adding your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.")
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

    let streaks_private = match privacy {
      Some(privacy) => match privacy {
        Privacy::Private => true,
        Privacy::Public => false,
      },
      None => default.streaks_private,
    };

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
              .content(":x: An error occured while removing your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.")
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
      let earned_streak_role = StreakRoles::from_streak(user_streak);

      if let Some(earned_streak_role) = earned_streak_role {
        if !current_streak_roles.contains(&earned_streak_role.to_role_id()) {
          match member.add_role(ctx, earned_streak_role.to_role_id()).await {
            Ok(()) => {}
            Err(err) => {
              error!("Error adding role: {err}");

              ctx.send(CreateReply::default()
                .content(":x: An error occured while adding your streak role. Your settings have been saved, but your roles have not been updated. Please contact a moderator.")
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;
            }
          }
        }
      }
    }
  }

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Streak settings successfully updated.".to_string()),
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
pub async fn stats(
  ctx: Context<'_>,
  #[description = "Set stats privacy (Defaults to public)"] privacy: Privacy,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let stats_private = match privacy {
    Privacy::Private => true,
    Privacy::Public => false,
  };

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

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      ":white_check_mark: Stats successfully set to **{}**.",
      privacy.name()
    )),
    true,
  )
  .await?;

  Ok(())
}
