use std::time::Duration;

use anyhow::{anyhow, Context as AnyhowContext, Result};
use chrono::{Duration as ChronoDuration, Utc};
use poise::serenity_prelude::{builder::*, ButtonStyle, ChannelId, ComponentInteractionCollector};
use poise::CreateReply;

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::time::{self, MinusOffsetChoice, PlusOffsetChoice};
use crate::commands::helpers::tracking;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::data::tracking_profile::{privacy, Privacy, Status};
use crate::database::DatabaseHandler;
use crate::events;
use crate::Context;

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
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id)
      .await?
      .unwrap_or_default();

  let privacy = privacy!(privacy, tracking_profile.tracking.privacy);

  // Usually not necessary, but defer to avoid possible unknown interaction
  // errors due to slow DB lookups, workload redeployment, etc.
  if privacy {
    ctx.defer_ephemeral().await?;
  } else {
    ctx.defer().await?;
  }

  let offset = match time::offset_from_choice(
    minus_offset,
    plus_offset,
    tracking_profile.utc_offset,
  ) {
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
    let adjusted_datetime = Utc::now() + ChronoDuration::minutes(i64::from(offset));
    DatabaseHandler::add_meditation_entry(
      &mut transaction,
      &guild_id,
      &user_id,
      minutes,
      seconds,
      adjusted_datetime,
    )
    .await?;
  }

  let user_sum =
    DatabaseHandler::get_user_meditation_sum(&mut transaction, &guild_id, &user_id).await?;

  let response = tracking::show_add_with_quote(
    &ctx,
    &mut transaction,
    &guild_id,
    &user_id,
    &minutes,
    &user_sum,
    privacy,
  )
  .await?;

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
              .style(ButtonStyle::Success),
            CreateButton::new(cancel_id.clone())
              .label("No")
              .style(ButtonStyle::Danger),
          ])]),
      )
      .await?;

    // Loop through incoming interactions with the navigation buttons
    while let Some(press) = ComponentInteractionCollector::new(ctx)
      // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
      // button was pressed
      .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
      // Timeout when no navigation button has been pressed in one minute
      .timeout(Duration::from_secs(60))
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
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage({
            if confirm {
              if privacy {
                CreateInteractionResponseMessage::new()
                  .content(format!(
                    "Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:"
                  ))
                  .ephemeral(privacy)
                  .components(Vec::new())
              } else {
                CreateInteractionResponseMessage::new()
                  .content(&response)
                  .ephemeral(privacy)
                  .components(Vec::new())
              }
            } else {
              CreateInteractionResponseMessage::new()
                .content("Cancelled.")
                .ephemeral(privacy)
                .components(Vec::new())
            }
          }),
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
                return Err(anyhow!("Could not send message: {e}"));
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
          return Err(anyhow!("Could not send message: {e}"));
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

        let log_channel = ChannelId::new(CHANNELS.bloomlogs);

        log_channel
          .send_message(ctx, CreateMessage::new().embed(log_embed))
          .await?;
      }

      return Ok(());
    }
  }

  // We only need to get the streak if streaks are active. If inactive,
  // this variable will be unused, so just assign a default value of 0.
  let user_streak = if tracking_profile.streak.status == Status::Enabled {
    let streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;
    streak.current
  } else {
    0
  };

  let guild_time_in_hours = tracking::get_guild_hours(&mut transaction, &guild_id).await?;

  if privacy {
    let private_response = format!(
      "Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:"
    );
    database::commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(private_response),
      Visibility::Ephemeral,
    )
    .await?;

    ctx
      .channel_id()
      .send_message(ctx, CreateMessage::new().content(response))
      .await?;
  } else {
    database::commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(response),
      Visibility::Public,
    )
    .await?;
  }

  tracking::post_guild_hours(&ctx, &guild_time_in_hours).await?;

  let member = guild_id.member(ctx, user_id).await?;
  tracking::update_time_roles(&ctx, &member, user_sum, privacy).await?;
  if tracking_profile.streak.status == Status::Enabled {
    tracking::update_streak_roles(&ctx, &member, user_streak, privacy).await?;
  }

  // Spawn a Tokio task to update leaderboards every 10th add
  if guild_time_in_hours.is_some() {
    tokio::spawn(events::leaderboards::update(
      module_path!(),
      ctx.serenity_context().http.clone(),
      data.db.clone(),
      guild_id,
    ));
  }

  Ok(())
}
