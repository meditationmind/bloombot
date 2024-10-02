use crate::commands::{commit_and_say, MessageType};
use crate::config::{BloomBotEmbed, StreakRoles, TimeSumRoles, CHANNELS, EMOJI};
use crate::database::{DatabaseHandler, TrackingProfile};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use chrono::Duration;
use log::error;
use poise::serenity_prelude::{self as serenity, builder::*, Mentionable};
use poise::CreateReply;

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

async fn update_time_roles(
  ctx: Context<'_>,
  member: &serenity::Member,
  sum: i64,
  privacy: bool,
) -> Result<()> {
  let current_time_roles = TimeSumRoles::get_users_current_roles(&member.roles);
  let updated_time_role = TimeSumRoles::from_sum(sum);

  if let Some(updated_time_role) = updated_time_role {
    if !current_time_roles.contains(&updated_time_role.to_role_id()) {
      for role in current_time_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");
            ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_time_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");
          ctx.send(CreateReply::default()
            .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true)).await?;

          return Ok(());
        }
      }

      ctx.send(CreateReply::default()
        .content(format!(":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the <@&{}> role!", member.mention(), updated_time_role.to_role_id()))
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .ephemeral(privacy)).await?;
    }
  }

  Ok(())
}

async fn update_streak_roles(
  ctx: Context<'_>,
  member: &serenity::Member,
  streak: i32,
  privacy: bool,
) -> Result<()> {
  let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);
  #[allow(clippy::cast_sign_loss)]
  let updated_streak_role = StreakRoles::from_streak(streak as u64);

  if let Some(updated_streak_role) = updated_streak_role {
    if !current_streak_roles.contains(&updated_streak_role.to_role_id()) {
      for role in current_streak_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");

            ctx.send(CreateReply::default()
                .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_streak_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");

          ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

          return Ok(());
        }
      }

      ctx.send(CreateReply::default()
          .content(format!(":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the <@&{}> role!", member.mention(), streak, updated_streak_role.to_role_id()))
          .allowed_mentions(serenity::CreateAllowedMentions::new())
          .ephemeral(privacy)).await?;
    }
  }

  Ok(())
}

/// Add a meditation entry
///
/// Adds a specified number of minutes to your meditation time. You can add minutes each time you meditate or add the combined minutes for multiple sessions.
///
/// You may wish to add large amounts of time on occasion, e.g., after a silent retreat. Time tracking is based on the honor system and members are welcome to track any legitimate time spent practicing.
///
/// Vanity roles are purely cosmetic, so there is nothing to be gained from cheating. Furthermore, exceedingly large false entries will skew the server stats, which is unfair to other members. Please be considerate.
#[poise::command(slash_command, category = "Meditation Tracking", guild_only)]
pub async fn add(
  ctx: Context<'_>,
  #[description = "Number of minutes to add"]
  #[min = 1]
  minutes: i32,
  #[description = "Number of seconds to add (defaults to 0)"]
  #[min = 0]
  seconds: Option<i32>,
  #[description = "Specify a UTC offset for a Western Hemisphere time zone"]
  #[rename = "western_hemisphere_offset"]
  minus_offset: Option<MinusOffsetChoices>,
  #[description = "Specify a UTC offset for an Eastern Hemisphere time zone"]
  #[rename = "eastern_hemisphere_offset"]
  plus_offset: Option<PlusOffsetChoices>,
  #[description = "Set visibility of response (defaults to public)"] privacy: Option<Privacy>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let tracking_profile =
    match DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await? {
      Some(tracking_profile) => tracking_profile,
      None => TrackingProfile {
        ..Default::default()
      },
    };

  let privacy = match privacy {
    Some(privacy) => match privacy {
      Privacy::Private => true,
      Privacy::Public => false,
    },
    None => tracking_profile.anonymous_tracking,
  };

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

  let seconds = seconds.unwrap_or(0);

  // If no offset is specified in the command or tracking profile, add using UTC.
  // Check for this first since it's the most common usage. Otherwise, check if multiple
  // offsets were specified in the command and abort if so. Then, add using the specified
  // offset. Prioritize command parameters so that the user can override their tracking
  // profile offset, if they choose to do so.
  if minus_offset == 0 && plus_offset == 0 && tracking_profile.utc_offset == 0 {
    DatabaseHandler::add_minutes(&mut transaction, &guild_id, &user_id, minutes, seconds).await?;
  } else if minus_offset != 0 && plus_offset != 0 {
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
  } else if minus_offset != 0 {
    let adjusted_datetime = chrono::Utc::now() + Duration::minutes(minus_offset);
    DatabaseHandler::create_meditation_entry(
      &mut transaction,
      &guild_id,
      &user_id,
      minutes,
      seconds,
      adjusted_datetime,
    )
    .await?;
  } else if plus_offset != 0 {
    let adjusted_datetime = chrono::Utc::now() + Duration::minutes(plus_offset);
    DatabaseHandler::create_meditation_entry(
      &mut transaction,
      &guild_id,
      &user_id,
      minutes,
      seconds,
      adjusted_datetime,
    )
    .await?;
  } else {
    let adjusted_datetime =
      chrono::Utc::now() + Duration::minutes(i64::from(tracking_profile.utc_offset));
    DatabaseHandler::create_meditation_entry(
      &mut transaction,
      &guild_id,
      &user_id,
      minutes,
      seconds,
      adjusted_datetime,
    )
    .await?;
  }

  let random_quote = DatabaseHandler::get_random_quote(&mut transaction, &guild_id).await?;
  let user_sum =
    DatabaseHandler::get_user_meditation_sum(&mut transaction, &guild_id, &user_id).await?;

  let response = match random_quote {
    Some(quote) => {
      // Strip non-alphanumeric characters from the quote
      let quote = quote
        .quote
        .chars()
        //.filter(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation() || matches!(c, '’' | '‘' | '“' | '”' | '—' | '…' | 'ā'))
        .filter(|c| !matches!(c, '*'))
        .map(|c| {
          if c.is_ascii_punctuation() {
            if matches!(c, '_' | '~') {
              c.to_string()
            } else {
              format!("\\{c}")
            }
          } else {
            c.to_string()
          }
        })
        .collect::<String>();

      if privacy {
        format!(
          "Someone just added **{minutes} minutes** to their meditation time! :tada:\n*{quote}*"
        )
      } else {
        format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:\n*{quote}*")
      }
    }
    None => {
      if privacy {
        format!("Someone just added **{minutes} minutes** to their meditation time! :tada:")
      } else {
        format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:")
      }
    }
  };

  if minutes > 300 {
    let ctx_id = ctx.id();

    let confirm_id = format!("{ctx_id}confirm");
    let cancel_id = format!("{ctx_id}cancel");

    let check = ctx
      .send(
        CreateReply::default()
          .content(format!(
            "Are you sure you want to add **{minutes}** minutes to your meditation time?"
          ))
          .ephemeral(privacy)
          .components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new(confirm_id.clone())
              .label("Yes")
              .style(serenity::ButtonStyle::Success),
            CreateButton::new(cancel_id.clone())
              .label("No")
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
      // Depending on which button was pressed, go to next or previous page
      if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
        // This is an unrelated button interaction
        continue;
      }

      let confirm = press.data.custom_id == confirm_id;

      // Update the message to reflect the action
      match press
        .create_response(ctx, CreateInteractionResponse::UpdateMessage(
          {
              if confirm {
                if privacy {
                  CreateInteractionResponseMessage::new().content(format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:"))
                    .ephemeral(privacy)
                    .components(Vec::new())
                } else {
                  CreateInteractionResponseMessage::new().content(&response)
                    .ephemeral(privacy)
                    .components(Vec::new())
                }
              } else {
                CreateInteractionResponseMessage::new().content("Cancelled.")
                  .ephemeral(privacy)
                  .components(Vec::new())
              }
            })
    )
        .await
      {
        Ok(()) => {
          if confirm {
            match DatabaseHandler::commit_transaction(transaction).await {
              Ok(()) => {}
              Err(e) => {
                check.edit(ctx, CreateReply::default()
                  .content(format!("{} A fatal error occurred while trying to save your changes. Please contact staff for assistance.", EMOJI.mminfo))
                  .ephemeral(privacy)).await?;
                return Err(anyhow::anyhow!("Could not send message: {e}"));
              }
            }
          }
        }
        Err(e) => {
          check
            .edit(ctx, CreateReply::default()
              .content(format!("{} An error may have occurred. If your command failed, please contact staff for assistance.", EMOJI.mminfo))
                .ephemeral(privacy)
            )
            .await?;
          return Err(anyhow::anyhow!("Could not send message: {e}"));
        }
      }

      if confirm && privacy {
        ctx
          .channel_id()
          .send_message(ctx, CreateMessage::new().content(response))
          .await?;
      }

      if confirm {
        // Log large add in Bloom logs channel
        let description = if seconds > 0 {
          format!(
            "**User**: {}\n**Time**: {} minutes {} second(s)",
            ctx.author(),
            minutes,
            seconds,
          )
        } else {
          format!("**User**: {}\n**Time**: {} minutes", ctx.author(), minutes,)
        };
        let log_embed = BloomBotEmbed::new()
          .title("Large Meditation Entry Added")
          .description(description)
          .footer(
            CreateEmbedFooter::new(format!(
              "Added by {} ({})",
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
      }

      return Ok(());
    }
  }

  // We only need to get the streak if streaks are active. If inactive,
  // this variable will be unused, so just assign a default value of 0.
  let user_streak = if tracking_profile.streaks_active {
    let streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;
    streak.current
  } else {
    0
  };

  // We only show the guild time every tenth add, so we can avoid getting
  // the guild sum and computing the hours if this is not the tenth add.
  // Return a string so we can use it to skip displaying the time later
  // without risking a default integer value matching the actual time.
  let guild_time_in_hours = {
    let guild_count =
      DatabaseHandler::get_guild_meditation_count(&mut transaction, &guild_id).await?;
    if guild_count % 10 == 0 {
      let guild_sum =
        DatabaseHandler::get_guild_meditation_sum(&mut transaction, &guild_id).await?;
      (guild_sum / 60).to_string()
    } else {
      "skip".to_owned()
    }
  };

  if privacy {
    let private_response = format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:");
    commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(private_response),
      true,
    )
    .await?;

    ctx
      .channel_id()
      .send_message(ctx, CreateMessage::new().content(response))
      .await?;
  } else {
    commit_and_say(ctx, transaction, MessageType::TextOnly(response), false).await?;
  }

  if guild_time_in_hours != "skip" {
    ctx.say(format!("Awesome sauce! This server has collectively generated {guild_time_in_hours} hours of realmbreaking meditation!")).await?;
  }

  let member = guild_id.member(ctx, user_id).await?;
  update_time_roles(ctx, &member, user_sum, privacy).await?;
  if tracking_profile.streaks_active {
    update_streak_roles(ctx, &member, user_streak, privacy).await?;
  }

  Ok(())
}
