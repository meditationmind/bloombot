use std::str::FromStr;

use crate::commands::{commit_and_say, MessageType};
use crate::config::{self, BloomBotEmbed, CHANNELS};
use crate::database::DatabaseHandler;
use crate::pagination::{PageRowRef, Pagination};
use crate::Context;
use anyhow::Result;
use poise::serenity_prelude::{self as serenity, builder::*, ChannelId, MessageId};
use poise::CreateReply;

/// Commands for erasing and erase logs
///
/// Commands to delete a message with private notification or review and update deletion logs.
///
/// Requires `Manage Messages` permissions.
#[poise::command(
  slash_command,
  required_permissions = "MANAGE_MESSAGES",
  default_member_permissions = "MANAGE_MESSAGES",
  category = "Moderator Commands",
  subcommands("message", "list", "populate"),
  //hide_in_help,
  guild_only
)]
pub async fn erase(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Delete a message and notify the user
///
/// Deletes a message and notifies the user via DM or private thread with an optional reason.
#[poise::command(slash_command)]
pub async fn message(
  ctx: Context<'_>,
  #[description = "The message ID of the message to delete"] message: String, // serenity::Message,
  #[max_length = 512] // Max length for audit log reason
  #[description = "The reason for deleting the message"]
  reason: Option<String>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let message_id = MessageId::from_str(message.as_str()).unwrap();
  let channel_id = ctx.channel_id();
  let message = channel_id.message(ctx, message_id).await?;

  // let channel_id: ChannelId = message.channel_id;
  // let message_id: MessageId = message.id;
  let reason = reason.unwrap_or("No reason provided.".to_string());
  let audit_log_reason: Option<&str> = Some(reason.as_str());

  ctx
    .http()
    .delete_message(channel_id, message_id, audit_log_reason)
    .await?;

  let occurred_at = chrono::Utc::now();

  let data = ctx.data();
  let guild_id = ctx.guild_id().unwrap();
  let user_id = message.author.id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let erase_count = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user_id)
    .await?
    .len()
    + 1;
  let erase_count_message = if erase_count == 1 {
    "1 erase recorded".to_string()
  } else {
    format!("{} erases recorded", erase_count)
  };

  let mut log_embed = BloomBotEmbed::new();
  let mut dm_embed = BloomBotEmbed::new();

  log_embed = log_embed.title("Message Deleted").description(format!(
    "**Channel**: <#{}>\n**Author**: {} ({})\n**Reason**: {}",
    message.channel_id, message.author, erase_count_message, reason,
  ));
  dm_embed = dm_embed
    .title("A message you sent has been deleted.")
    .description(format!("**Reason**: {}", reason));

  if let Some(attachment) = message.attachments.first() {
    log_embed = log_embed.field("Attachment", attachment.url.clone(), false);
    dm_embed = dm_embed.field("Attachment", attachment.url.clone(), false);
  }

  if !message.content.is_empty() {
    // If longer than 1024 - 6 characters for the embed, truncate to 1024 - 3 for "..."
    let content = match message.content.len() > 1018 {
      true => format!(
        "{}...",
        message.content.chars().take(1015).collect::<String>()
      ),
      false => message.content.clone(),
    };

    log_embed = log_embed.field("Message Content", format!("```{}```", content), false);
    dm_embed = dm_embed.field("Message Content", format!("```{}```", content), false);
  }

  log_embed = log_embed.footer(
    CreateEmbedFooter::new(format!(
      "Deleted by {} ({})",
      ctx.author().name,
      ctx.author().id
    ))
    .icon_url(ctx.author().avatar_url().unwrap_or_default()),
  );
  dm_embed = dm_embed.footer(CreateEmbedFooter::new(
    "If you have any questions or concerns regarding this action, please contact a moderator. Replies sent to Bloom are not viewable by staff."
  ));

  let log_channel = serenity::ChannelId::new(CHANNELS.logs);

  let log_message = log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  let message_link = log_message.link();

  DatabaseHandler::add_erase(
    &mut transaction,
    &guild_id,
    &user_id,
    &message_link,
    occurred_at,
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      ":white_check_mark: Message deleted. User will be notified via DM or private thread."
    )),
    true,
  )
  .await?;

  match message
    .author
    .direct_message(ctx, CreateMessage::new().embed(dm_embed.to_owned()))
    .await
  {
    Ok(_) => {
      //  commit_and_say(
      //    ctx,
      //    transaction,
      //    MessageType::TextOnly(format!(":white_check_mark: Message deleted. Sent the reason in DMs.")),
      //    true,
      //  )
      //  .await?;
    }
    Err(_) => {
      let thread_channel: ChannelId = match message
        .channel_id
        .to_channel(&ctx)
        .await
        .unwrap()
        .guild()
        .unwrap()
        .kind
      {
        serenity::ChannelType::Text => channel_id,
        // If not a text channel, then create private thread in lounge to avoid failure
        _ => ChannelId::from(501464482996944909),
      };

      let mut notification_thread = thread_channel
        .create_thread(
          ctx,
          CreateThread::new(format!("Private Notification: Message Deleted")),
        )
        .await?;

      notification_thread
        .edit_thread(ctx, EditThread::new().invitable(false).locked(true))
        .await?;

      dm_embed = dm_embed.footer(CreateEmbedFooter::new(
        "If you have any questions or concerns regarding this action, please contact staff via ModMail."
      ));

      let thread_initial_message = format!("Private notification for <@{}>:", message.author.id);

      notification_thread
        .send_message(
          ctx,
          CreateMessage::new()
            .content(thread_initial_message)
            .embed(dm_embed.to_owned())
            .allowed_mentions(CreateAllowedMentions::new().users([message.author.id])),
        )
        .await?;

      //commit_and_say(
      //  ctx,
      //  transaction,
      //  MessageType::TextOnly(format!(
      //    ":white_check_mark: Message deleted. Could not send the reason in DMs. Private thread created: <#{}>",
      //    notification_thread.id
      //  )),
      //  true,
      //)
      //.await?;
    }
  };

  Ok(())
}

/// List erases for a user
///
/// List erases for a specified user, with dates and links to notification messages, when available.
#[poise::command(slash_command)]
pub async fn list(
  ctx: Context<'_>,
  #[description = "The user to show erase data for"] user: serenity::User,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();

  // We unwrap here, because we know that the command is guild-only.
  let guild_id = ctx.guild_id().unwrap();
  let user_nick_or_name = match user.nick_in(&ctx, guild_id).await {
    Some(nick) => nick,
    None => user.name.clone(),
  };

  let privacy = if ctx.channel_id() == config::CHANNELS.logs {
    false
  } else {
    true
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{}prev", ctx_id);
  let next_button_id = format!("{}next", ctx_id);

  let mut current_page = page.unwrap_or(0);

  if current_page > 0 {
    current_page = current_page - 1
  }

  let erases = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user.id).await?;
  let erases: Vec<PageRowRef> = erases.iter().map(|erase| erase as _).collect();
  drop(transaction);
  let pagination = Pagination::new(format!("Erases for {}", user_nick_or_name), erases).await?;

  if pagination.get_page(current_page).is_none() {
    current_page = pagination.get_last_page_number();
  }

  let first_page = pagination.create_page_embed(current_page);

  ctx
    .send({
      let mut f = CreateReply::default();
      if pagination.get_page_count() > 1 {
        f = f.components(vec![CreateActionRow::Buttons(vec![
          CreateButton::new(&prev_button_id).label("Previous"),
          CreateButton::new(&next_button_id).label("Next"),
        ])])
      }
      f.embeds = vec![first_page];
      f.ephemeral(privacy)
    })
    .await?;

  // Loop through incoming interactions with the navigation buttons
  while let Some(press) = serenity::ComponentInteractionCollector::new(ctx)
    // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
    // button was pressed
    .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
    // Timeout when no navigation button has been pressed for 24 hours
    .timeout(std::time::Duration::from_secs(3600 * 24))
    .await
  {
    // Depending on which button was pressed, go to next or previous page
    if press.data.custom_id == next_button_id {
      current_page = pagination.update_page_number(current_page, 1);
    } else if press.data.custom_id == prev_button_id {
      current_page = pagination.update_page_number(current_page, -1);
    } else {
      // This is an unrelated button interaction
      continue;
    }

    // Update the message with the new page contents
    press
      .create_response(
        ctx,
        CreateInteractionResponse::UpdateMessage(
          CreateInteractionResponseMessage::new().embed(pagination.create_page_embed(current_page)),
        ),
      )
      .await?;
  }

  Ok(())
}

/// Populate past erases for a user
///
/// Populate the database with past erases for a user.
#[poise::command(slash_command)]
pub async fn populate(
  ctx: Context<'_>,
  #[description = "The user to populate erase data for"] user: serenity::User,
  #[description = "The link for the erase notification message"] message_link: String,
  #[description = "The year of the erase"] year: u32,
  #[description = "The month of the erase"]
  #[min = 1]
  #[max = 12]
  month: u32,
  #[description = "The day of the erase"]
  #[min = 1]
  #[max = 31]
  day: u32,
  #[description = "The hour of the erase (defaults to 0)"]
  #[min = 0]
  #[max = 23]
  hour: Option<u32>,
  #[description = "The minute of the erase (defaults to 0)"]
  #[min = 0]
  #[max = 59]
  minute: Option<u32>,
) -> Result<()> {
  let data = ctx.data();

  // We unwrap here, because we know that the command is guild-only.
  let guild_id = ctx.guild_id().unwrap();

  let date = match chrono::NaiveDate::from_ymd_opt(year as i32, month, day) {
    Some(date) => date,
    None => {
      ctx
        .send(
          CreateReply::default()
            .embed(
              CreateEmbed::new()
                .title("Error")
                .description(format!("Invalid date provided: {}-{}-{}", year, month, day))
                .color(serenity::Color::RED),
            )
            .ephemeral(true),
        )
        .await?;
      return Ok(());
    }
  };

  let time = match chrono::NaiveTime::from_hms_opt(hour.unwrap_or(0), minute.unwrap_or(0), 0) {
    Some(time) => time,
    None => {
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
    }
  };

  let datetime = chrono::NaiveDateTime::new(date, time).and_utc();

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  DatabaseHandler::add_erase(
    &mut transaction,
    &guild_id,
    &user.id,
    &message_link,
    datetime,
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Erase data has been added.".to_string()),
    true,
  )
  .await?;

  Ok(())
}
