use crate::commands::{commit_and_say, MessageType};
use crate::config::{BloomBotEmbed, StreakRoles, TimeSumRoles, CHANNELS, EMOJI};
use crate::database::{DatabaseHandler, TrackingProfile};
use crate::time::{offset_from_choice, MinusOffsetChoice, PlusOffsetChoice};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use chrono::Duration;
use log::error;
use poise::serenity_prelude::{self as serenity, builder::*, Mentionable};
use poise::CreateReply;

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
  minus_offset: Option<MinusOffsetChoice>,
  #[description = "Specify a UTC offset for an Eastern Hemisphere time zone"]
  #[rename = "eastern_hemisphere_offset"]
  plus_offset: Option<PlusOffsetChoice>,
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

  let offset = match offset_from_choice(minus_offset, plus_offset, tracking_profile.utc_offset) {
    Ok(offset) => offset,
    Err(e) => {
      ctx
          .send(
            CreateReply::default()
                .content(format!(
                  "A problem occurred while attempting to determine the UTC offset based on your choice: {e}"
                ))
                .ephemeral(true),
          )
          .await?;
      return Ok(()); // Return early to avoid further processing
    }
  };

  let seconds = seconds.unwrap_or(0);

  if offset == 0 {
    DatabaseHandler::add_minutes(&mut transaction, &guild_id, &user_id, minutes, seconds).await?;
  } else {
    let adjusted_datetime = chrono::Utc::now() + Duration::minutes(i64::from(offset));
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

  if guild_time_in_hours != "skip" {
    let task_http = ctx.serenity_context().http.clone();
    let task_conn = data.db.clone();
    let update_leaderboards = tokio::task::spawn(async move {
      log::info!("Leaderboard: Refreshing views");
      let refresh_start = std::time::Instant::now();
      if let Err(err) = crate::events::leaderboards::refresh(&task_conn).await {
        error!("Leaderboard: Error refreshing views: {:?}", err);
      }
      log::info!("Refresh completed in {:#?}", refresh_start.elapsed());

      tokio::time::sleep(std::time::Duration::from_secs(10)).await;

      log::info!("Leaderboard: Generating images");
      let generation_start = std::time::Instant::now();
      if let Err(err) =
        crate::events::leaderboards::generate(&task_http, &task_conn, &guild_id).await
      {
        error!("Leaderboard: Error generating images: {:?}", err);
      }
      log::info!("Leaderboard: Generation completed in {:#?}", generation_start.elapsed());
    });
    update_leaderboards.await?;
  }

  Ok(())
}
