use std::borrow::Cow;

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use log::info;
use poise::serenity_prelude::{builder::*, ChannelId, Message, RoleId, User};
use poise::{ChoiceParameter, CreateReply};
use tokio::{fs, fs::File, io::AsyncWriteExt};
use ulid::Ulid;

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::import::{FinchBreathingSession, FinchTimerSession};
use crate::commands::helpers::import::{Source, SqlQueries};
use crate::commands::helpers::tracking;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, MEDITATION_MIND, ROLES};
use crate::data::tracking_profile::{privacy, Privacy, Status};
use crate::database::DatabaseHandler;
use crate::Context;

#[derive(ChoiceParameter)]
pub enum Type {
  #[name = "new entries"]
  NewEntries,
  #[name = "all entries"]
  AllEntries,
}

/// Import meditation entries from an app
///
/// Imports meditation entries from a CSV or JSON file uploaded by the user.
///
/// Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser).
#[poise::command(slash_command, category = "Meditation Tracking")]
pub async fn import(
  ctx: Context<'_>,
  #[description = "The message with the CSV/JSON file"] message: Message,
  #[description = "The type of import (Defaults to new entries)"]
  #[rename = "type"]
  import_type: Option<Type>,
  #[description = "The user to import for (staff only)"] user: Option<User>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let Some(attachment) = message.attachments.first() else {
    let msg = format!("{} No attachment found.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  };

  let dm = ctx.guild_id().is_none();
  let guild_id = ctx.guild_id().unwrap_or(MEDITATION_MIND);

  let staff = ctx
    .author_member()
    .await
    .is_some_and(|member| member.roles.contains(&RoleId::from(ROLES.staff)));

  let user_id = user.map_or(message.author.id, |user| {
    if staff {
      user.id
    } else {
      message.author.id
    }
  });

  // Can only import own attachments, unless staff.
  if message.author.id != ctx.author().id && !staff {
    let msg = format!(
      "{} You cannot import files uploaded by other users.",
      EMOJI.mminfo
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  // Limit filesize to 256KiB, unless staff.
  if attachment.size > 262_144 && !staff {
    let msg = format!(
      "{} File exceeds size limit. Please contact staff for assistance with importing large files.",
      EMOJI.mminfo
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let content = match attachment.download().await {
    Ok(content) => {
      if attachment.filename == *"TimerSession.json" {
        FinchTimerSession::to_csv(&content).unwrap_or(content)
      } else if attachment.filename == *"BreathingSession.json" {
        FinchBreathingSession::to_csv(&content).unwrap_or(content)
      } else {
        content
      }
    }
    Err(e) => {
      info!("Error downloading attachment: {e}");
      let msg = format!("{} Unable to download attachment.", EMOJI.mminfo);
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Ok(());
    }
  };

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let tracking_profile =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id)
      .await?
      .unwrap_or_default();

  let privacy = privacy!(tracking_profile.tracking.privacy);

  let import_type = import_type.unwrap_or(Type::NewEntries);
  let latest_meditation =
    DatabaseHandler::get_latest_meditation_entry(&mut transaction, &guild_id, &user_id)
      .await?
      // Default is UNIX_EPOCH (1970-01-01 00:00:00 UTC)
      .unwrap_or_default();
  let latest_time = latest_meditation.occurred_at;
  let current_data = if matches!(import_type, Type::NewEntries) {
    vec![latest_meditation]
  } else {
    DatabaseHandler::get_user_meditation_entries(&mut transaction, &guild_id, &user_id).await?
  };

  let mut rdr = ReaderBuilder::new().from_reader(content.as_slice());

  let source = match Source::autodetect(&mut rdr) {
    Ok(source) => source,
    Err(e) => {
      info!("Failed to autodetect CSV source: {e}");
      let msg = format!(
        "{} **Unrecognized file format.**\n-# Please use an unaltered data export. \
        Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, \
        Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing \
        with Bloom Parser). If you would like support for another format, please contact staff.",
        EMOJI.mminfo
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      if message.author.id == ctx.author().id
        && message.channel_id == ChannelId::new(CHANNELS.tracking)
      {
        message.delete(ctx).await?;
      }
      return Ok(());
    }
  };

  let import = source.import(&mut rdr, &current_data, latest_time, &import_type)?;

  if !dm {
    message.delete(ctx).await?;
  }

  drop(content);
  drop(current_data);

  if import.data.is_empty() {
    let msg = format!("{} No qualifying entries found.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let sql = SqlQueries::generate(guild_id, user_id, import.data.as_slice());

  let result = DatabaseHandler::add_meditation_entry_batch(&mut transaction, &sql.insert).await?;
  if result < 1 {
    let msg = format!(
      "{} No entries added. Please try again or contact staff for assistance.",
      EMOJI.mminfo
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let user_sum =
    DatabaseHandler::get_user_meditation_sum(&mut transaction, &guild_id, &user_id).await?;

  let response = tracking::show_add_with_quote(
    &ctx,
    &mut transaction,
    &guild_id,
    &user_id,
    &(import.minutes + (import.seconds / 60)),
    &user_sum,
    privacy,
  )
  .await?;

  let user_streak = if tracking_profile.streak.status == Status::Enabled {
    let streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;
    streak.current
  } else {
    0
  };

  let guild_time_in_hours = tracking::get_guild_hours(&mut transaction, &guild_id).await?;

  let h = (import.minutes + (import.seconds / 60)) / 60;
  let m = (import.minutes + (import.seconds / 60)) % 60;
  let s = import.seconds % 60;

  let success_response = format!(
    "{} Successfully added a total of {h}h {m}m {s}s from {result} {} imported from {}.",
    EMOJI.mmcheck,
    if result == 1 { "entry" } else { "entries" },
    import.source
  );

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(success_response),
    Visibility::Ephemeral,
  )
  .await?;

  ChannelId::new(CHANNELS.tracking)
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(response)
        .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;

  tracking::post_guild_hours(&ctx, &guild_time_in_hours).await?;

  if let Some(member) = if user_id == ctx.author().id && ctx.guild_id().is_some() {
    ctx.author_member().await
  } else {
    guild_id.member(ctx, user_id).await.ok().map(Cow::Owned)
  } {
    tracking::update_time_roles(&ctx, &member, user_sum, privacy).await?;
    if tracking_profile.streak.status == Status::Enabled {
      tracking::update_streak_roles(&ctx, &member, user_streak, privacy).await?;
    }
  } else {
    info!("Unable to update roles for user: {user_id}");
  }

  let filename = format!("import_{}_{}.txt", user_id, Ulid::new().to_string());
  let mut file = File::create(&filename).await?;
  file.write_all(sql.delete.as_bytes()).await?;
  file.flush().await?;
  let f1 = File::open(&filename).await?;
  let return_file = [CreateAttachment::file(&f1, &filename).await?];

  let log_embed = BloomBotEmbed::new()
    .title("Meditation Tracking Data Import")
    .description(format!(
      "**User**: <@{user_id}>\n**Entries Added**: {result}\n**Total Time**: {} minutes\n**Source**: {}",
      import.minutes, import.source,
    ))
    .footer(
      CreateEmbedFooter::new(format!(
        "Added by {} ({})",
        ctx.author().name,
        ctx.author().id
      ))
      .icon_url(ctx.author().avatar_url().unwrap_or_default()),
    );

  let log_channel = ChannelId::new(CHANNELS.bloomlogs);
  log_channel
    .send_files(&ctx, return_file, CreateMessage::new().embed(log_embed))
    .await?;

  if let Err(e) = fs::remove_file(filename).await {
    return Err(anyhow!("Error removing file: {e:?}"));
  }

  Ok(())
}
