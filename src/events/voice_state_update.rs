#![allow(clippy::cast_possible_truncation)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use poise::serenity_prelude::{AutoArchiveDuration, ButtonStyle, ChannelId, ChannelType};
use poise::serenity_prelude::{ComponentInteractionCollector, Context, CreateActionRow};
use poise::serenity_prelude::{CreateAllowedMentions, CreateButton, CreateInteractionResponse};
use poise::serenity_prelude::{CreateInteractionResponseMessage, CreateMessage, CreateThread};
use poise::serenity_prelude::{EditMessage, EditThread, GuildId, Member, Mentionable, MessageId};
use poise::serenity_prelude::{MessageReference, MessageReferenceKind, UserId, VoiceState};
use sqlx::{Connection, PgConnection, Postgres, Transaction};
use tracing::info;
use ulid::Ulid;

use crate::commands::helpers::{self, tracking};
use crate::config::{CHANNELS, MEDITATION_MIND, ROLES, StreakRoles, TimeSumRoles};
use crate::data::bloom::Data;
use crate::data::meditation::Meditation;
use crate::data::tracking_profile::privacy;
use crate::data::tracking_profile::{Privacy, PrivateNotifications, Status, TrackingProfile};
use crate::events;
use crate::handlers::database::DatabaseHandler;

enum Destination {
  DirectMessage,
  PrivateThread,
  PublicChannel,
}

pub async fn voice_state_update(
  ctx: &Context,
  data: &Data,
  old: Option<&VoiceState>,
  new: &VoiceState,
) -> Result<()> {
  let user_id = new.user_id;
  let old_channel_id = old
    .and_then(|v| v.channel_id.map(ChannelId::get))
    .unwrap_or(0);
  let new_channel_id = new.channel_id.map_or(0, ChannelId::get);
  let voice_state = Arc::clone(&data.voice_state);
  let key = user_id.get();

  let meditation_vcs = [
    CHANNELS.group_meditation.id(),
    CHANNELS.meditate_with_me_1.id(),
    CHANNELS.meditate_with_me_2.id(),
    CHANNELS.meditation_hall.id(),
  ];

  if !meditation_vcs.contains(&old_channel_id) && !meditation_vcs.contains(&new_channel_id)
    || old_channel_id == new_channel_id
  {
    return Ok(());
  }

  // Joined meditation VC
  if !meditation_vcs.contains(&old_channel_id) && meditation_vcs.contains(&new_channel_id) {
    let mut voice_state = voice_state.lock().await;
    voice_state.insert(key, Instant::now());
    return Ok(());
  }

  // Switched to different meditation VC
  if meditation_vcs.contains(&old_channel_id) && meditation_vcs.contains(&new_channel_id) {
    let mut voice_state = voice_state.lock().await;
    if let Some(time) = voice_state.get(&key) {
      // If less than five minutes before switch, reset timer.
      if time.elapsed() < Duration::from_secs(60 * 5) {
        voice_state.entry(key).insert_entry(Instant::now());
      }
    } else {
      // Key-value should exist, but insert if some kind of error prevented insertion.
      voice_state.insert(key, Instant::now());
    }
    return Ok(());
  }

  let mut elapsed = Duration::new(0, 0);

  // Left meditation VC
  if meditation_vcs.contains(&old_channel_id) && !meditation_vcs.contains(&new_channel_id) {
    let mut voice_state = voice_state.lock().await;
    if let Some(time) = voice_state.remove(&key) {
      elapsed = time.elapsed();
    }
  }

  if elapsed >= Duration::from_secs(60 * 5) {
    let guild_id = new.guild_id.unwrap_or(MEDITATION_MIND);
    let tracking_channel = ChannelId::from(CHANNELS.tracking);
    let mut transaction = data.db.start_transaction_with_retry(5).await?;
    let tracking_profile = if let Some(existing_profile) =
      DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await?
    {
      existing_profile
    } else {
      let new_profile = TrackingProfile::new(guild_id, user_id);
      DatabaseHandler::add_tracking_profile(&mut transaction, &new_profile).await?;
      new_profile
    };
    let privacy = privacy!(tracking_profile.tracking.privacy);
    let member = if let Some(member) = &new.member {
      member
    } else {
      &guild_id.member(ctx, user_id).await?
    };

    if let Some(vc_tracking) = tracking_profile.vc_tracking {
      match vc_tracking {
        Status::Disabled => return Ok(()),
        Status::Enabled => {
          let (add_with_quote, user_streak, guild_hours, time, user_sum) = add_time(
            guild_id,
            user_id,
            &tracking_profile,
            &mut transaction,
            elapsed,
            privacy,
          )
          .await?;
          DatabaseHandler::commit_transaction(transaction).await?;

          let public_handle =
            notify_add(ctx, member, tracking_channel, add_with_quote, guild_hours).await?;

          if privacy {
            let message_builder = CreateMessage::default()
              .content(format!(
                "{} Added **{time}** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:",
                user_id.mention()
              ));
            let thread_builder = CreateThread::new(format!(
              "VC Meditation Notifications for {}",
              member.display_name()
            ))
            .kind(ChannelType::PrivateThread);

            let (channel, message, destination) = match tracking_profile.notifications {
              PrivateNotifications::DirectMessage => {
                if let Ok(dm) = user_id.dm(ctx, message_builder).await {
                  (Some(dm.channel_id), dm.id, Destination::DirectMessage)
                } else {
                  (None, MessageId::default(), Destination::DirectMessage)
                }
              }
              PrivateNotifications::PrivateThread => match tracking_profile.thread_id {
                Some(thread) => match thread.send_message(ctx, message_builder.clone()).await {
                  Ok(message) => (Some(thread), message.id, Destination::PrivateThread),
                  Err(_) => match tracking_channel.create_thread(ctx, thread_builder).await {
                    Ok(thread) => match thread.send_message(ctx, message_builder).await {
                      Ok(message) => (Some(thread.id), message.id, Destination::PrivateThread),
                      Err(_) => (None, MessageId::default(), Destination::PrivateThread),
                    },
                    Err(_) => (None, MessageId::default(), Destination::PrivateThread),
                  },
                },
                None => match tracking_channel.create_thread(ctx, thread_builder).await {
                  Ok(thread) => match thread.send_message(ctx, message_builder).await {
                    Ok(message) => (Some(thread.id), message.id, Destination::PrivateThread),
                    Err(_) => (None, MessageId::default(), Destination::PrivateThread),
                  },
                  Err(_) => (None, MessageId::default(), Destination::PrivateThread),
                },
              },
              PrivateNotifications::Disabled => {
                (None, MessageId::default(), Destination::PrivateThread)
              }
            };
            let private_handle = (channel.unwrap_or(ChannelId::default()), message);

            process_time_roles(ctx, member, channel, &destination, user_sum, private_handle)
              .await?;

            if tracking_profile.streak.status == Status::Enabled {
              process_streak_roles(
                ctx,
                member,
                channel,
                &destination,
                user_streak,
                Some(private_handle),
              )
              .await?;
            }

            if matches!(destination, Destination::PrivateThread)
              && channel
                .is_some_and(|c_id| tracking_profile.thread_id.is_none_or(|t_id| t_id != c_id))
            {
              let mut transaction = data.db.start_transaction_with_retry(5).await?;
              DatabaseHandler::update_tracking_profile(
                &mut transaction,
                &tracking_profile.with_thread_id(channel),
              )
              .await?;
              DatabaseHandler::commit_transaction(transaction).await?;
            }
          } else {
            let channel = Some(tracking_channel);
            let destination = Destination::PublicChannel;

            process_time_roles(ctx, member, channel, &destination, user_sum, public_handle).await?;

            if tracking_profile.streak.status == Status::Enabled {
              if privacy!(tracking_profile.streak.privacy) {
                match tracking_profile.notifications {
                  PrivateNotifications::DirectMessage => match user_id
                    .dm(
                      ctx,
                      CreateMessage::default().content("Awesome! You did it! :heart:"),
                    )
                    .await
                  {
                    Ok(dm) => {
                      process_streak_roles(
                        ctx,
                        member,
                        Some(dm.channel_id),
                        &Destination::DirectMessage,
                        user_streak,
                        Some((dm.channel_id, dm.id)),
                      )
                      .await?;
                    }
                    Err(_) => {
                      process_streak_roles(ctx, member, None, &destination, user_streak, None)
                        .await?;
                    }
                  },
                  PrivateNotifications::PrivateThread => {
                    if let Some(thread) = tracking_profile.thread_id {
                      process_streak_roles(
                        ctx,
                        member,
                        Some(thread),
                        &Destination::PrivateThread,
                        user_streak,
                        None,
                      )
                      .await?;
                    } else {
                      process_streak_roles(ctx, member, None, &destination, user_streak, None)
                        .await?;
                    }
                  }
                  PrivateNotifications::Disabled => {
                    process_streak_roles(ctx, member, None, &destination, user_streak, None)
                      .await?;
                  }
                }
              } else {
                process_streak_roles(
                  ctx,
                  member,
                  channel,
                  &destination,
                  user_streak,
                  Some(public_handle),
                )
                .await?;
              }
            }
          }

          // Refresh leaderboards every 10th add.
          if guild_hours.is_some() {
            tokio::spawn(events::leaderboards::update(
              module_path!(),
              data.db.clone(),
            ));
          }

          return Ok(());
        }
      }
    }

    let unique_id = Ulid::new();
    let confirm = format!("{unique_id}confirm");
    let cancel = format!("{unique_id}cancel");
    let always = format!("{unique_id}always");
    let never = format!("{unique_id}never");

    let time_elapsed = tracking::format_time(
      (elapsed.as_secs() / 60) as i32,
      (elapsed.as_secs() % 60) as i32,
    );
    let msg = format!(
      "{} Looks like you just finished a meditation. \
      Would you like to add **{time_elapsed}** to your meditation time?",
      user_id.mention()
    );

    let button_message =
      CreateMessage::default()
        .content(msg)
        .components(vec![CreateActionRow::Buttons(vec![
          CreateButton::new(confirm.as_str())
            .label("Yes")
            .style(ButtonStyle::Success),
          CreateButton::new(cancel.as_str())
            .label("No")
            .style(ButtonStyle::Danger),
          CreateButton::new(always.as_str())
            .label("Always")
            .style(ButtonStyle::Secondary),
          CreateButton::new(never.as_str())
            .label("Never")
            .style(ButtonStyle::Secondary),
        ])]);

    let (notify_channel, mut message, destination) = if privacy {
      let thread = if let Some(thread_id) = tracking_profile.thread_id {
        thread_id
      } else {
        tracking_channel
          .create_thread(
            ctx,
            CreateThread::new(format!(
              "VC Meditation Notifications for {}",
              member.display_name()
            ))
            .kind(ChannelType::PrivateThread),
          )
          .await?
          .id
      };
      (
        thread,
        thread.send_message(ctx, button_message).await?,
        Destination::PrivateThread,
      )
    } else {
      (
        tracking_channel,
        tracking_channel.send_message(ctx, button_message).await?,
        Destination::PublicChannel,
      )
    };

    let customize_vc = helpers::print_command(&data.commands, "customize vc");

    while let Some(press) = ComponentInteractionCollector::new(ctx)
      .message_id(message.id)
      .timeout(Duration::from_secs(120))
      .await
    {
      // Acknowledge presses from other users to prevent interaction failed messages.
      if press.user.id != user_id {
        press
          .create_response(ctx, CreateInteractionResponse::Acknowledge)
          .await?;
        continue;
      }

      if press.data.custom_id == cancel || press.data.custom_id == never {
        let thread = if matches!(destination, Destination::PrivateThread) {
          Some(notify_channel)
        } else {
          tracking_profile.thread_id
        };
        let no_or_never = if press.data.custom_id == never {
          DatabaseHandler::update_tracking_profile(
            &mut transaction,
            &tracking_profile
              .with_vc_tracking(Status::Disabled)
              .with_thread_id(thread),
          )
          .await?;

          format!(
            "You will no longer receive these notifications. You can change this setting at any time using {customize_vc}."
          )
        } else {
          if tracking_profile.thread_id.is_none() {
            DatabaseHandler::update_tracking_profile(
              &mut transaction,
              &tracking_profile.with_thread_id(thread),
            )
            .await?;
          }

          format!(
            "Use {customize_vc} to automatically add or ignore VC meditation time without receiving these messages."
          )
        };
        DatabaseHandler::commit_transaction(transaction).await?;

        press
          .create_response(
            ctx,
            CreateInteractionResponse::Message(
              CreateInteractionResponseMessage::new()
                .content(format!("No time added. {no_or_never}"))
                .components(Vec::new())
                .allowed_mentions(CreateAllowedMentions::new())
                .ephemeral(true),
            ),
          )
          .await?;

        message.delete(ctx).await?;
        if matches!(destination, Destination::PrivateThread) {
          notify_channel
            .edit_thread(
              ctx,
              EditThread::new().auto_archive_duration(AutoArchiveDuration::OneHour),
            )
            .await?;
        }

        return Ok(());
      }

      if press.data.custom_id == confirm || press.data.custom_id == always {
        let (add_with_quote, user_streak, guild_hours, time, user_sum) = add_time(
          guild_id,
          user_id,
          &tracking_profile,
          &mut transaction,
          elapsed,
          privacy,
        )
        .await?;

        let streak_status = tracking_profile.streak.status;
        let streak_privacy = tracking_profile.streak.privacy;
        let notifications = tracking_profile.notifications;
        let thread = if matches!(destination, Destination::PrivateThread) {
          Some(notify_channel)
        } else {
          tracking_profile.thread_id
        };
        let always_response = if press.data.custom_id == always {
          DatabaseHandler::update_tracking_profile(
            &mut transaction,
            &tracking_profile
              .with_vc_tracking(Status::Enabled)
              .with_thread_id(thread),
          )
          .await?;
          format!(
            "\n\nYour VC meditation time is now set to be added automatically. You can change this setting at any time using {customize_vc}."
          )
        } else {
          DatabaseHandler::update_tracking_profile(
            &mut transaction,
            &tracking_profile.with_thread_id(thread),
          )
          .await?;
          String::new()
        };
        DatabaseHandler::commit_transaction(transaction).await?;

        let added_msg = if matches!(destination, Destination::PublicChannel) {
          String::new()
        } else {
          format!("{} ", user_id.mention())
        };

        press
          .create_response(
            ctx,
            CreateInteractionResponse::Message(
              CreateInteractionResponseMessage::new()
                .content(format!(
                  "{added_msg}Added **{time}** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:{always_response}",
                ))
                .components(Vec::new())
                .allowed_mentions(CreateAllowedMentions::new())
                .ephemeral(matches!(destination, Destination::PublicChannel)),
            ),
          )
          .await?;

        let public_handle =
          notify_add(ctx, member, tracking_channel, add_with_quote, guild_hours).await?;
        let handle = if matches!(destination, Destination::PrivateThread) {
          (notify_channel, message.id)
        } else {
          public_handle
        };

        process_time_roles(
          ctx,
          member,
          Some(notify_channel),
          &destination,
          user_sum,
          handle,
        )
        .await?;
        if streak_status == Status::Enabled {
          if privacy!(streak_privacy) && matches!(destination, Destination::PublicChannel) {
            if notifications == PrivateNotifications::Disabled {
              process_streak_roles(ctx, member, None, &destination, user_streak, None).await?;
            } else {
              process_streak_roles(ctx, member, thread, &destination, user_streak, None).await?;
            }
          } else {
            process_streak_roles(
              ctx,
              member,
              Some(notify_channel),
              &destination,
              user_streak,
              Some(handle),
            )
            .await?;
          }
        }

        // Refresh leaderboards every 10th add.
        if guild_hours.is_some() {
          tokio::spawn(events::leaderboards::update(
            module_path!(),
            data.db.clone(),
          ));
        }

        message.delete(ctx).await?;
        if matches!(destination, Destination::PrivateThread) {
          notify_channel
            .edit_thread(
              ctx,
              EditThread::new().auto_archive_duration(AutoArchiveDuration::OneWeek),
            )
            .await?;
        }

        return Ok(());
      }
    }

    // Edit initial message since collector timed out with no response.
    let add = helpers::print_command(&data.commands, "add");
    if matches!(destination, Destination::PublicChannel) {
      message
        .edit(
          ctx,
          EditMessage::new()
            .content(format!(
              "{} You were in a meditation VC for **{time_elapsed}**. You can use {add} to log the session.",
              user_id.mention()
            ))
            .components(Vec::new()),
        )
        .await?;
    } else {
      message
        .edit(
          ctx,
          EditMessage::new()
            .content(format!(
              "**Request timed out. No action taken.**\n\n\
              You were in a meditation VC for **{time_elapsed}**. This was an attempt to \
              automatically log that time. You can manually log using {add} in {}.\n\n\
              Use {customize_vc} to enable fully automated tracking for meditation VCs, or \
              disable to prevent automatic tracking and suppress these messages. You can \
              also customize if and how you receive related private notifications.",
              CHANNELS.tracking
            ))
            .components(Vec::new()),
        )
        .await?;

      notify_channel
        .edit_thread(
          ctx,
          EditThread::new().auto_archive_duration(AutoArchiveDuration::OneDay),
        )
        .await?;

      if tracking_profile.thread_id.is_none() {
        let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
          transaction
        } else {
          info!("DB connection closed. Reconnecting.");
          data.db.start_transaction_with_retry(5).await?
        };
        DatabaseHandler::update_tracking_profile(
          &mut transaction,
          &tracking_profile.with_thread_id(Some(notify_channel)),
        )
        .await?;
        DatabaseHandler::commit_transaction(transaction).await?;
      }
    }
  }

  Ok(())
}

/// Performs database operations related to adding a meditation time.
///
/// After calling [`add_meditation_entry`][add], returns `add_with_quote`
/// from [`show_add_with_quote`][show],`user_streak` from [`get_streak`][streak],
/// `guild_hours` from [`get_guild_hours`][hrs], the user's `time` meditated, formatted for
/// end-user use, and a `user_sum` from [`get_user_meditation_sum()`][sum].
///
/// [add]: DatabaseHandler::add_meditation_entry()
/// [show]: tracking::show_add_with_quote()
/// [streak]: DatabaseHandler::get_streak()
/// [hrs]: tracking::get_guild_hours
/// [sum]: DatabaseHandler::get_user_meditation_sum()
async fn add_time(
  guild_id: GuildId,
  user_id: UserId,
  tracking_profile: &TrackingProfile,
  transaction: &mut Transaction<'_, Postgres>,
  elapsed: Duration,
  privacy: bool,
) -> Result<(String, i32, Option<i64>, String, i64)> {
  let offset = tracking_profile.utc_offset;
  let datetime = match offset {
    0 => Utc::now(),
    _ => Utc::now() + ChronoDuration::minutes(i64::from(offset)),
  };
  let minutes = (elapsed.as_secs() / 60) as i32;
  let seconds = (elapsed.as_secs() % 60) as i32;
  let time = tracking::format_time(minutes, seconds);
  let meditation = Meditation::new(guild_id, user_id, minutes, seconds, &datetime);
  DatabaseHandler::add_meditation_entry(transaction, &meditation).await?;
  let user_sum = DatabaseHandler::get_user_meditation_sum(transaction, &guild_id, &user_id).await?;
  let add_with_quote = tracking::show_add_with_quote(
    "vc",
    transaction,
    &guild_id,
    &user_id,
    time.as_str(),
    &user_sum,
    privacy,
  )
  .await?;
  let user_streak = match tracking_profile.streak.status {
    Status::Enabled => {
      let streak = DatabaseHandler::get_streak(transaction, &guild_id, &user_id).await?;
      streak.current
    }
    Status::Disabled => 0,
  };
  let guild_hours = tracking::get_guild_hours(transaction, &guild_id).await?;

  Ok((add_with_quote, user_streak, guild_hours, time, user_sum))
}

/// Issues notification of a newly added session, and a guild total on every 10th add,
/// to the specified [`ChannelId`]. Returns a `(ChannelId, MessageId)` tuple, which can
/// be used to create a reply [`MessageReference`].
async fn notify_add(
  ctx: &Context,
  member: &Member,
  tracking_channel: ChannelId,
  add_with_quote: String,
  guild_hours: Option<i64>,
) -> Result<(ChannelId, MessageId)> {
  let mentions = if member.roles.contains(&ROLES.no_pings.into()) {
    CreateAllowedMentions::new()
  } else {
    CreateAllowedMentions::new().users([member.user.id])
  };
  let message = tracking_channel
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(add_with_quote)
        .allowed_mentions(mentions),
    )
    .await?;

  if let Some(guild_hours) = guild_hours {
    tracking_channel
      .send_message(
        &ctx,
        CreateMessage::new()
          .content(format!(
            "Awesome sauce! This server has collectively generated {guild_hours} hours of realmbreaking meditation!"
          ))
          .reference_message(
            MessageReference::new(MessageReferenceKind::Default, message.channel_id)
              .message_id(message.id)
              .fail_if_not_exists(false),
          ),
      )
      .await?;
  }
  Ok((message.channel_id, message.id))
}

/// Updates a [`Member`]'s meditation time roles and notifies upon advancement to a new role,
/// with formatting catered to the specified [`Destination`]. Uses a `(ChannelId, MessageId)`
/// tuple to generate a reply [`MessageReference`].
async fn process_time_roles(
  ctx: &Context,
  member: &Member,
  notify_channel: Option<ChannelId>,
  destination: &Destination,
  user_sum: i64,
  handle: (ChannelId, MessageId),
) -> Result<()> {
  if let Some(updated_time_role) = TimeSumRoles::from_sum(user_sum) {
    let current_time_roles = TimeSumRoles::current(&member.roles);
    if !current_time_roles.contains(&updated_time_role.to_role_id()) {
      for role in current_time_roles {
        member.remove_role(ctx, role).await?;
      }
      member.add_role(ctx, updated_time_role.to_role_id()).await?;

      if let Some(notify_channel) = notify_channel {
        let congrats = match destination {
          Destination::DirectMessage => format!(
            ":tada: Congrats {}, your hard work is paying off! Your total meditation minutes have given you the @{} role!",
            member.mention(),
            updated_time_role.to_role_icon()
          ),
          Destination::PrivateThread => format!(
            ":tada: Congrats {}, your hard work is paying off! Your total meditation minutes have given you the {} role!",
            member.mention(),
            updated_time_role.to_role_id().mention()
          ),
          Destination::PublicChannel => format!(
            ":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the {} role!",
            member.mention(),
            updated_time_role.to_role_id().mention()
          ),
        };

        notify_channel
          .send_message(
            &ctx,
            CreateMessage::new()
              .content(congrats)
              .reference_message(
                MessageReference::new(MessageReferenceKind::Default, handle.0)
                  .message_id(handle.1)
                  .fail_if_not_exists(false),
              )
              .allowed_mentions(CreateAllowedMentions::new()),
          )
          .await?;
      }
    }
  }
  Ok(())
}

/// Updates a [`Member`]'s meditation streak roles and notifies upon advancement to a new role,
/// with formatting catered to the specified [`Destination`]. Optionally, uses a `(ChannelId,
/// MessageId)` tuple to generate a reply [`MessageReference`].
async fn process_streak_roles(
  ctx: &Context,
  member: &Member,
  notify_channel: Option<ChannelId>,
  destination: &Destination,
  user_streak: i32,
  handle: Option<(ChannelId, MessageId)>,
) -> Result<()> {
  if let Some(updated_streak_role) = StreakRoles::from_streak(user_streak.cast_unsigned().into()) {
    let current_streak_roles = StreakRoles::current(&member.roles);
    if !current_streak_roles.contains(&updated_streak_role.to_role_id()) {
      for role in current_streak_roles {
        member.remove_role(ctx, role).await?;
      }
      member
        .add_role(ctx, updated_streak_role.to_role_id())
        .await?;

      if let Some(notify_channel) = notify_channel {
        let congrats = match destination {
          Destination::DirectMessage => format!(
            ":tada: Congrats {}, your hard work is paying off! Your current streak is {user_streak}, giving you the @{} role!",
            member.mention(),
            updated_streak_role.to_role_icon()
          ),
          Destination::PrivateThread => format!(
            ":tada: Congrats {}, your hard work is paying off! Your current streak is {user_streak}, giving you the {} role!",
            member.mention(),
            updated_streak_role.to_role_id().mention()
          ),
          Destination::PublicChannel => format!(
            ":tada: Congrats to {}, your hard work is paying off! Your current streak is {user_streak}, giving you the {} role!",
            member.mention(),
            updated_streak_role.to_role_id().mention()
          ),
        };
        let builder = if let Some(handle) = handle {
          CreateMessage::new().reference_message(
            MessageReference::new(MessageReferenceKind::Default, handle.0)
              .message_id(handle.1)
              .fail_if_not_exists(false),
          )
        } else {
          CreateMessage::new()
        };
        notify_channel
          .send_message(
            &ctx,
            builder
              .content(congrats)
              .allowed_mentions(CreateAllowedMentions::new()),
          )
          .await?;
      }
    }
  }
  Ok(())
}
