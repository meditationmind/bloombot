#![allow(clippy::too_many_arguments)]

use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{BloomBotEmbed, CHANNELS, ENTRIES_PER_PAGE};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use chrono::{Datelike, Timelike};
use poise::serenity_prelude::{self as serenity, builder::*, Mentionable};
use poise::{ChoiceParameter, CreateReply};

#[derive(poise::ChoiceParameter)]
enum DataType {
  #[name = "meditation entries"]
  MeditationEntries,
  #[name = "customization settings"]
  CustomizationSettings,
}

/// Commands for managing meditation entries
///
/// Commands to create, list, update, or delete meditation entries for a user, or completely reset a user's data.
///
/// Requires `Ban Members` permissions.
#[poise::command(
  slash_command,
  subcommands("create", "list", "update", "delete", "reset", "migrate"),
  subcommand_required,
  required_permissions = "BAN_MEMBERS",
  default_member_permissions = "BAN_MEMBERS",
  category = "Moderator Commands",
  //hide_in_help,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn manage(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Create a new meditation entry for a user. Note that all times are in UTC.
///
/// Creates a new meditation entry for the user. Note that all times are in UTC.
#[poise::command(slash_command)]
async fn create(
  ctx: Context<'_>,
  #[description = "The user to create the entry for"] user: serenity::User,
  #[description = "The number of minutes for the entry"]
  #[min = 0]
  minutes: i32,
  #[description = "The number of seconds for the entry (defaults to 0)"]
  #[min = 0]
  seconds: Option<i32>,
  // Message will not be older than Discord itself
  #[min = 2015]
  #[description = "The year of the entry"]
  year: i32,
  #[description = "The month of the entry"]
  #[min = 1]
  #[max = 12]
  month: u32,
  #[description = "The day of the entry"]
  #[min = 1]
  #[max = 31]
  day: u32,
  #[description = "The hour of the entry (defaults to 0)"]
  #[min = 0]
  #[max = 23]
  hour: Option<u32>,
  #[description = "The minute of the entry (defaults to 0)"]
  #[min = 0]
  #[max = 59]
  minute: Option<u32>,
) -> Result<()> {
  let Some(entry_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .title("Error")
              .description(format!("Invalid date provided: {year}-{month}-{day}"))
              .color(serenity::Color::RED),
          )
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  let Some(entry_time) = chrono::NaiveTime::from_hms_opt(hour.unwrap_or(0), minute.unwrap_or(0), 0)
  else {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .title("Error")
              .description(format!(
                "Invalid time provided: {}:{}",
                hour.unwrap_or(0),
                minute.unwrap_or(0)
              ))
              .color(serenity::Color::RED),
          )
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  let datetime = chrono::NaiveDateTime::new(entry_date, entry_time).and_utc();
  let seconds = seconds.unwrap_or(0);

  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  DatabaseHandler::create_meditation_entry(
    &mut transaction,
    &guild_id,
    &user.id,
    minutes,
    seconds,
    datetime,
  )
  .await?;

  let description = if seconds > 0 {
    format!(
      "**User**: <@{}>\n**Date**: {}\n**Time**: {} minute(s) {} second(s)",
      user.id,
      datetime.format("%B %d, %Y"),
      minutes,
      seconds,
    )
  } else {
    format!(
      "**User**: <@{}>\n**Date**: {}\n**Time**: {} minute(s)",
      user.id,
      datetime.format("%B %d, %Y"),
      minutes,
    )
  };

  let success_embed = BloomBotEmbed::new()
    .title("Meditation Entry Created")
    .description(&description)
    .clone();

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::EmbedOnly(Box::new(success_embed)),
    true,
  )
  .await?;

  let log_embed = BloomBotEmbed::new()
    .title("Meditation Entry Created")
    .description(description)
    .footer(
      CreateEmbedFooter::new(format!(
        "Created by {} ({})",
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

  Ok(())
}

/// List all meditation entries for a user
///
/// Lists all meditation entries for a user.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The user to list the entries for"] user: serenity::User,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let entries =
    DatabaseHandler::get_user_meditation_entries(&mut transaction, &guild_id, &user.id).await?;
  let entries: Vec<PageRowRef> = entries.iter().map(|entry| entry as _).collect();

  drop(transaction);

  Paginator::new("Meditation Entries", &entries, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, true)
    .await?;

  Ok(())
}

/// Update a meditation entry for a user. Note that all times are in UTC.
///
/// Updates a meditation entry for a user. Note that all times are in UTC.
#[poise::command(slash_command)]
async fn update(
  ctx: Context<'_>,
  #[description = "The entry to update"] entry_id: String,
  #[description = "The number of minutes for the entry"]
  #[min = 0]
  minutes: Option<i32>,
  #[description = "The number of seconds for the entry"]
  #[min = 0]
  seconds: Option<i32>,
  #[description = "The year of the entry"] year: Option<i32>,
  #[description = "The month of the entry"]
  #[min = 1]
  #[max = 12]
  month: Option<u32>,
  #[description = "The day of the entry"]
  #[min = 1]
  #[max = 31]
  day: Option<u32>,
  #[description = "The hour of the entry (defaults to 0)"]
  #[min = 0]
  #[max = 23]
  hour: Option<u32>,
  #[description = "The minute of the entry (defaults to 0)"]
  #[min = 0]
  #[max = 59]
  minute: Option<u32>,
) -> Result<()> {
  let existing_entry = {
    let data = ctx.data();
    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;

    let mut transaction = data.db.start_transaction_with_retry(5).await?;

    DatabaseHandler::get_meditation_entry(&mut transaction, &guild_id, &entry_id).await?
  };

  if minutes.is_none()
    && seconds.is_none()
    && year.is_none()
    && month.is_none()
    && day.is_none()
    && hour.is_none()
    && minute.is_none()
  {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .title("Error")
              .description("You must provide at least one option to update the entry.")
              .color(serenity::Color::RED),
          )
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  if let Some(existing_entry) = existing_entry {
    let minutes = minutes.unwrap_or(existing_entry.minutes);
    let seconds = seconds.unwrap_or(existing_entry.seconds);

    let existing_date = existing_entry.occurred_at;
    let year = year.unwrap_or(existing_date.year());
    let month = month.unwrap_or(existing_date.month());
    let day = day.unwrap_or(existing_date.day());
    let hour = hour.unwrap_or(existing_date.hour());
    let minute = minute.unwrap_or(existing_date.minute());

    let Some(entry_date) = chrono::NaiveDate::from_ymd_opt(year, month, day) else {
      ctx
        .send(
          CreateReply::default()
            .embed(
              CreateEmbed::new()
                .title("Error")
                .description(format!("Invalid date provided: {year}-{month}-{day}"))
                .color(serenity::Color::RED),
            )
            .ephemeral(true),
        )
        .await?;
      return Ok(());
    };

    let Some(entry_time) = chrono::NaiveTime::from_hms_opt(hour, minute, 0) else {
      ctx
        .send(
          CreateReply::default()
            .embed(
              CreateEmbed::new()
                .title("Error")
                .description(format!("Invalid time provided: {hour}:{minute}"))
                .color(serenity::Color::RED),
            )
            .ephemeral(true),
        )
        .await?;
      return Ok(());
    };

    let datetime = chrono::NaiveDateTime::new(entry_date, entry_time).and_utc();

    let data = ctx.data();

    let mut transaction = data.db.start_transaction_with_retry(5).await?;

    DatabaseHandler::update_meditation_entry(
      &mut transaction,
      &entry_id,
      minutes,
      seconds,
      datetime,
    )
    .await?;

    let description = if existing_entry.seconds > 0 || seconds > 0 {
      format!(
        "**User**: <@{}>\n**ID**: {}\n\n__**Before**__\n**Date**: {}\n**Time**: {} minute(s) {} second(s)\n\n__**After**__\n**Date**: {}\n**Time**: {} minute(s) {} second(s)",
        existing_entry.user_id,
        entry_id,
        existing_date.format("%B %d, %Y at %l:%M %P"),
        existing_entry.minutes,
        existing_entry.seconds,
        datetime.format("%B %d, %Y at %l:%M %P"),
        minutes,
        seconds,
      )
    } else {
      format!(
        "**User**: <@{}>\n**ID**: {}\n\n__**Before**__\n**Date**: {}\n**Time**: {} minute(s)\n\n__**After**__\n**Date**: {}\n**Time**: {} minute(s)",
        existing_entry.user_id,
        entry_id,
        existing_date.format("%B %d, %Y at %l:%M %P"),
        existing_entry.minutes,
        datetime.format("%B %d, %Y at %l:%M %P"),
        minutes,
        )
    };

    let success_embed = BloomBotEmbed::new()
      .title("Meditation Entry Updated")
      .description(&description)
      .clone();

    database::commit_and_say(
      ctx,
      transaction,
      MessageType::EmbedOnly(Box::new(success_embed)),
      true,
    )
    .await?;

    let log_embed = BloomBotEmbed::new()
      .title("Meditation Entry Updated")
      .description(description)
      .footer(
        CreateEmbedFooter::new(format!(
          "Updated by {} ({})",
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

    Ok(())
  } else {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .title("Error")
              .description(format!("No meditation entry found with ID `{entry_id}`."))
              .footer(CreateEmbedFooter::new(
                "Use `/manage list` to see a user's entries.",
              ))
              .color(serenity::Color::RED),
          )
          .ephemeral(true),
      )
      .await?;

    Ok(())
  }
}

/// Delete a meditation entry for a user
///
/// Deletes a meditation entry for the user.
#[poise::command(slash_command)]
async fn delete(
  ctx: Context<'_>,
  #[description = "The entry to delete"] entry_id: String,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let Some(entry) =
    DatabaseHandler::get_meditation_entry(&mut transaction, &guild_id, &entry_id).await?
  else {
    ctx
      .send(
        CreateReply::default()
          .embed(
            CreateEmbed::new()
              .title("Error")
              .description(format!("No meditation entry found with ID `{entry_id}`."))
              .footer(CreateEmbedFooter::new(
                "Use `/manage list` to see a user's entries.",
              ))
              .color(serenity::Color::RED),
          )
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  DatabaseHandler::delete_meditation_entry(&mut transaction, &entry_id).await?;

  let description = if entry.seconds > 0 {
    format!(
      "**User**: <@{}>\n**ID**: {}\n**Date**: {}\n**Time**: {} minute(s) {} second(s)",
      entry.user_id,
      entry.id,
      entry.occurred_at.format("%B %d, %Y"),
      entry.minutes,
      entry.seconds,
    )
  } else {
    format!(
      "**User**: <@{}>\n**ID**: {}\n**Date**: {}\n**Time**: {} minute(s)",
      entry.user_id,
      entry.id,
      entry.occurred_at.format("%B %d, %Y"),
      entry.minutes,
    )
  };

  let success_embed = BloomBotEmbed::new()
    .title("Meditation Entry Deleted")
    .description(&description)
    .clone();

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::EmbedOnly(Box::new(success_embed)),
    true,
  )
  .await?;

  let log_embed = BloomBotEmbed::new()
    .title("Meditation Entry Deleted")
    .description(description)
    .footer(
      CreateEmbedFooter::new(format!(
        "Deleted by {} ({})",
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

  Ok(())
}

/// Reset meditation entries or customization settings
///
/// Resets all meditation entries or customization settings for a user.
#[poise::command(slash_command)]
async fn reset(
  ctx: Context<'_>,
  #[description = "The user to reset the entries for"] user: serenity::User,
  #[description = "The type of data to reset (Defaults to meditation entries)"]
  #[rename = "type"]
  data_type: Option<DataType>,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  //Default to meditation entries
  let data_type = match data_type {
    Some(data_type) => data_type,
    None => DataType::MeditationEntries,
  };

  match data_type {
    DataType::CustomizationSettings => {
      DatabaseHandler::remove_tracking_profile(&mut transaction, &guild_id, &user.id).await?;
    }
    DataType::MeditationEntries => {
      DatabaseHandler::reset_user_meditation_entries(&mut transaction, &guild_id, &user.id).await?;
    }
  }

  let ctx_id = ctx.id();

  let confirm_id = format!("{ctx_id}confirm");
  let cancel_id = format!("{ctx_id}cancel");

  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "Are you sure you want to reset all {} for {}?",
          data_type.name(),
          user.mention()
        ))
        .ephemeral(true)
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

    let confirmed = press.data.custom_id == confirm_id;

    // Update the message with the new page contents
    if confirmed {
      match press
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .content("Confirmed.")
              .components(Vec::new()),
          ),
        )
        .await
      {
        Ok(()) => {
          DatabaseHandler::commit_transaction(transaction).await?;

          let log_embed = BloomBotEmbed::new()
            .title(format!(
              "{} Reset",
              match data_type {
                DataType::CustomizationSettings => "Customization Settings",
                DataType::MeditationEntries => "Meditation Entries",
              }
            ))
            .description(format!("**User**: <@{}>", user.id))
            .footer(
              CreateEmbedFooter::new(format!(
                "Reset by {} ({})",
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
          DatabaseHandler::rollback_transaction(transaction).await?;
          return Err(anyhow::anyhow!(
            "Failed to tell user that the {} were reset: {}",
            data_type.name(),
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
            .components(Vec::new()),
        ),
      )
      .await?;
  }

  // This happens when the user didn't press any button for 60 seconds
  Ok(())
}

/// Migrates meditation entries or customization settings
///
/// Migrates all meditation entries or customization settings from one user account to another.
#[poise::command(slash_command)]
async fn migrate(
  ctx: Context<'_>,
  #[description = "The user to migrate data from"] old_user: serenity::User,
  #[description = "The user to migrate data to"] new_user: serenity::User,
  #[description = "The type of data to migrate (Defaults to meditation entries)"]
  #[rename = "type"]
  data_type: Option<DataType>,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  //Default to meditation entries
  let data_type = match data_type {
    Some(data_type) => data_type,
    None => DataType::MeditationEntries,
  };

  match data_type {
    DataType::CustomizationSettings => {
      DatabaseHandler::migrate_tracking_profile(
        &mut transaction,
        &guild_id,
        &old_user.id,
        &new_user.id,
      )
      .await?;
    }
    DataType::MeditationEntries => {
      DatabaseHandler::migrate_meditation_entries(
        &mut transaction,
        &guild_id,
        &old_user.id,
        &new_user.id,
      )
      .await?;
    }
  }

  let ctx_id = ctx.id();

  let confirm_id = format!("{ctx_id}confirm");
  let cancel_id = format!("{ctx_id}cancel");

  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "Are you sure you want to migrate all {} from {} to {}?",
          data_type.name(),
          old_user.mention(),
          new_user.mention(),
        ))
        .ephemeral(true)
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

    let confirmed = press.data.custom_id == confirm_id;

    // Update the message with the new page contents
    if confirmed {
      match press
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .content("Confirmed.")
              .components(Vec::new()),
          ),
        )
        .await
      {
        Ok(()) => {
          DatabaseHandler::commit_transaction(transaction).await?;

          let log_embed = BloomBotEmbed::new()
            .title(format!(
              "{} Migrated",
              match data_type {
                DataType::CustomizationSettings => "Customization Settings",
                DataType::MeditationEntries => "Meditation Entries",
              }
            ))
            .description(format!(
              "**From**: <@{}>\n**To**: <@{}>",
              old_user.id, new_user.id,
            ))
            .footer(
              CreateEmbedFooter::new(format!(
                "Migrated by {} ({})",
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
          DatabaseHandler::rollback_transaction(transaction).await?;
          return Err(anyhow::anyhow!(
            "Failed to tell user that the {} were migrated: {}",
            data_type.name(),
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
            .components(Vec::new()),
        ),
      )
      .await?;
  }

  // This happens when the user didn't press any button for 60 seconds
  Ok(())
}
