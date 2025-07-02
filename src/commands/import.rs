use std::borrow::Cow;

use anyhow::{Result, anyhow};
use csv::ReaderBuilder;
use poise::serenity_prelude::{Attachment, ChannelId, Message, User, UserId, builder::*};
use poise::{ChoiceParameter, CreateReply};
use tokio::{fs, fs::File, io::AsyncWriteExt};
use tracing::{error, warn};
use ulid::Ulid;

use crate::Context;
use crate::commands::helpers::common::{self, Visibility};
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::import::{FinchBreathingSession, FinchTimerSession};
use crate::commands::helpers::import::{Source, SqlQueries};
use crate::commands::helpers::tracking;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, MEDITATION_MIND, ROLES};
use crate::data::tracking_profile::{Privacy, Status, privacy};
use crate::database::DatabaseHandler;

// 256KiB
const MAX_SIZE: u32 = 262_144;

#[derive(ChoiceParameter)]
pub enum Type {
  #[name = "new entries"]
  NewEntries,
  #[name = "all entries"]
  AllEntries,
}

enum ImportError {
  AttachmentMissing,
  ImportOtherUser,
  FilesizeExceedsLimit,
  DownloadFailed,
  UnrecognizedFormat,
  NoQualifyingEntries,
  ZeroEntriesAdded,
}

/// Import meditation entries from an app
///
/// Imports meditation entries from a CSV or JSON file uploaded by the user.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  subcommands("file_upload", "message")
)]
#[allow(clippy::unused_async)]
pub async fn import(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Import meditation entries by uploading a log file directly
///
/// Imports meditation entries from a CSV or JSON file uploaded directly via the command.
///
/// Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser).
#[poise::command(slash_command, category = "Meditation Tracking", rename = "file")]
pub async fn file_upload(
  ctx: Context<'_>,
  #[description = "Select a CSV/JSON file to upload"]
  #[rename = "file"]
  attachment: Attachment,
  #[description = "The type of import (defaults to new entries)"]
  #[rename = "type"]
  import_type: Option<Type>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  if !common::has_role(ctx, ROLES.staff).await && attachment.size > MAX_SIZE {
    notify_error(ctx, ImportError::FilesizeExceedsLimit).await?;
    return Ok(());
  }

  if let Err(e) = process_import(ctx, &attachment, import_type, ctx.author().id).await {
    // If error is not from autodetect, bubble up to error handler.
    if e.to_string().ne("autodetect") {
      return Err(e);
    }
  }

  Ok(())
}

/// Import meditation entries from a message that contains a log file attachment
///
/// Imports meditation entries from a CSV or JSON file uploaded as a message attachment within Discord.
///
/// Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser).
#[poise::command(slash_command, category = "Meditation Tracking")]
pub async fn message(
  ctx: Context<'_>,
  #[description = "The message with the CSV/JSON file"] message: Message,
  #[description = "The type of import (defaults to new entries)"]
  #[rename = "type"]
  import_type: Option<Type>,
  #[description = "The user to import for (staff only)"] user: Option<User>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let Some(attachment) = message.attachments.first() else {
    notify_error(ctx, ImportError::AttachmentMissing).await?;
    return Ok(());
  };

  let staff = common::has_role(ctx, ROLES.staff).await;

  if !staff {
    // Can only import own attachments, unless staff.
    if message.author.id != ctx.author().id {
      notify_error(ctx, ImportError::ImportOtherUser).await?;
      return Ok(());
    }
    if attachment.size > MAX_SIZE {
      notify_error(ctx, ImportError::FilesizeExceedsLimit).await?;
      return Ok(());
    }
  }

  let user_id = user.map_or(message.author.id, |user| {
    if staff { user.id } else { message.author.id }
  });

  if let Err(e) = process_import(ctx, attachment, import_type, user_id).await {
    // If own message, clean up following error. Otherwise, leave message
    // and let staff choose whether to troubleshoot or delete.
    if message.author.id == ctx.author().id {
      message.delete(ctx).await?;
    }
    // If error is not from autodetect, bubble up to error handler.
    if e.to_string().ne("autodetect") {
      return Err(e);
    }
    return Ok(());
  }

  // Don't delete if in DM.
  if ctx.guild_id().is_some() {
    message.delete(ctx).await?;
  }

  Ok(())
}

async fn process_import(
  ctx: Context<'_>,
  attachment: &Attachment,
  import_type: Option<Type>,
  user_id: UserId,
) -> Result<()> {
  let guild_id = ctx.guild_id().unwrap_or(MEDITATION_MIND);

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
      warn!("Error downloading attachment: {e}");
      notify_error(ctx, ImportError::DownloadFailed).await?;
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
      notify_error(ctx, ImportError::UnrecognizedFormat).await?;
      error!(
        "\x1B[1m/{}\x1B[0m failed with error: Failed to autodetect CSV source: {e}",
        ctx.command().qualified_name
      );
      error!(
        "\tSource: {} ({})",
        ctx
          .channel_id()
          .name(ctx)
          .await
          .unwrap_or("unknown".to_string()),
        ctx.channel_id()
      );
      error!("\tUser: {} ({})", ctx.author().name, ctx.author().id);
      return Err(anyhow!("autodetect"));
    }
  };

  let import = source.import(&mut rdr, &current_data, latest_time, &import_type)?;

  drop(content);
  drop(current_data);

  if import.data.is_empty() {
    notify_error(ctx, ImportError::NoQualifyingEntries).await?;
    return Ok(());
  }

  let sql = SqlQueries::generate(guild_id, user_id, import.data.as_slice());

  let result = DatabaseHandler::add_meditation_entry_batch(&mut transaction, &sql.insert).await?;
  if result < 1 {
    notify_error(ctx, ImportError::ZeroEntriesAdded).await?;
    return Ok(());
  }

  let time = tracking::format_time(import.minutes, import.seconds);

  let user_sum =
    DatabaseHandler::get_user_meditation_sum(&mut transaction, &guild_id, &user_id).await?;

  let response = tracking::show_add_with_quote(
    ctx.command().name.as_str(),
    &mut transaction,
    &guild_id,
    &user_id,
    time.as_str(),
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

  let success_response = format!(
    "{} Successfully added a total of {time} from {result} {} imported from {}.",
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

  let member = if user_id == ctx.author().id && ctx.guild_id().is_some() {
    ctx.author_member().await
  } else {
    guild_id.member(ctx, user_id).await.ok().map(Cow::Owned)
  };

  let mentions = if let Some(member) = &member
    && member.roles.contains(&ROLES.no_pings.into())
  {
    CreateAllowedMentions::new()
  } else {
    CreateAllowedMentions::new().users([user_id])
  };

  let notify = ChannelId::new(CHANNELS.tracking)
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(response)
        .allowed_mentions(mentions),
    )
    .await?;

  let reference = (notify.channel_id, notify.id);

  tracking::post_guild_hours(&ctx, guild_time_in_hours).await?;

  if let Some(member) = member {
    tracking::update_time_roles(&ctx, &member, user_sum, privacy, Some(reference)).await?;
    if tracking_profile.streak.status == Status::Enabled {
      let privacy = privacy!(tracking_profile.streak.privacy);
      tracking::update_streak_roles(&ctx, &member, user_streak, privacy, Some(reference)).await?;
    }
  } else {
    warn!("Unable to update roles for user: {user_id}");
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
    warn!("Error removing file: {e:?}");
  }

  Ok(())
}

async fn notify_error(ctx: Context<'_>, error: ImportError) -> Result<()> {
  let error_message = match error {
    ImportError::AttachmentMissing => "No attachment found.",
    ImportError::ImportOtherUser => "You cannot import files uploaded by other users.",
    ImportError::FilesizeExceedsLimit => {
      "File exceeds size limit. Please contact staff for assistance with importing large files."
    }
    ImportError::DownloadFailed => "Failed to download attachment. Please try again.",
    ImportError::UnrecognizedFormat => {
      "**Unrecognized file format.**\n-# Please use an unaltered data export. \
        Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, \
        Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing \
        with [Bloom Parser](<https://meditationmind.org/bloom/#bloom-parser>)). \
        If you would like support for another format, please contact staff."
    }
    ImportError::NoQualifyingEntries => "No qualifying entries found.",
    ImportError::ZeroEntriesAdded => {
      "No entries added. Please try again or contact staff for assistance."
    }
  };
  let msg = format!("{} {error_message}", EMOJI.mminfo);
  ctx.send(CreateReply::default().content(msg)).await?;
  Ok(())
}
