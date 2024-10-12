use crate::commands::{commit_and_say, MessageType};
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ENTRIES_PER_PAGE};
use crate::database::DatabaseHandler;
use crate::pagination::{PageRowRef, PageType, Pagination};
use crate::{Context, Data as AppData, Error as AppError};
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*, ChannelId, MessageId};
use poise::{CreateReply, Modal};

#[derive(poise::ChoiceParameter)]
pub enum DateFormat {
  #[name = "YYYY-MM-DD (ISO 8601)"]
  Ymd,
  #[name = "DD Month YYYY"]
  Dmy,
}

#[derive(poise::ChoiceParameter)]
pub enum DefaultReasons {
  #[name = "Rule 1: Be kind"]
  Rule1BeKind,
  #[name = "Rule 2: Be respectful"]
  Rule2BeRespectful,
  #[name = "Rule 3: All-ages appropriate"]
  Rule3AllAges,
  #[name = "Rule 4: Respect boundaries"]
  Rule4RespectBoundaries,
  #[name = "Rule 8: No self-promo"]
  Rule8SelfPromo,
  #[name = "Rule 9: Respect IP rights"]
  Rule9IPRights,
  #[name = "Rule 10: No drug chat"]
  Rule10Drugs,
  #[name = "Rule 10: No politics"]
  Rule10Politics,
  #[name = "Non-discussion channel"]
  NonDiscussionChannel,
  #[name = "Unwholesome meme"]
  UnwholesomeMeme,
}

impl DefaultReasons {
  fn response(&self) -> String {
    match *self {
      DefaultReasons::Rule1BeKind =>
        "Please help us cultivate a warm and welcoming atmosphere by remaining civil \
        and treating others with kindness. Disagreeing with others, challenging views, \
        questioning actions/behavior, etc. can all be done respectfully and mindfully."
        .to_string(),
      DefaultReasons::Rule2BeRespectful =>
        "Please help us cultivate an atmosphere of respect. This is a diverse and inclusive \
        community; all ages, genders, religions, and traditions are welcome. Questioning \
        identities or views may done respectfully, as long as the other person exhibits a \
        willingness to engage."
        .to_string(),
      DefaultReasons::Rule3AllAges =>
        "This is an all-ages server. Please help us cultivate a wholesome atmosphere by being \
        mindful of excessive profanity and vulgar language. Try to limit mature themes, such as \
        sexual or potentially triggering topics, to the <#1020856801115246702> forum with the \
        `Mature Topic` tag applied."
        .to_string(),
      DefaultReasons::Rule4RespectBoundaries =>
        "We love having fun! A bit of jesting or banter is fine, as long as it is \
        good-natured and consensual. If the other party ever seems uncomfortable or asks you to \
        stop, respect their wishes, even if you were engaging with good intentions."
        .to_string(),
      DefaultReasons::Rule8SelfPromo =>
        "Advertising, recruitment, solicitation, and self-promo require approval. This server is \
        meant to be a safe space, where community members can feel confident they're interacting with \
        fellow members who care about the community, and self-promo can violate that sense of safety. \
        While exceptions are rare, you may contact <@575252669443211264> to request authorization."
        .to_string(),
      DefaultReasons::Rule9IPRights =>
        "Sharing content that violates intellectual property rights is prohibited by the Discord \
        Community Guidelines."
        .to_string(),
      DefaultReasons::Rule10Drugs =>
        "As an all-ages Discord Partner community accessible via Server Discovery, \
        discussion of drugs/controlled substances is prohibited by Discord's Discovery \
        guidelines. Please help us cultivate and maintain an environment that is appropriate \
        for all of our diverse membership. Thank you!"
        .to_string(),
      DefaultReasons::Rule10Politics =>
        "Due to the strong tendency for political discussions to become highly charged and promote \
        polarized views, we ask that members choose a different outlet. As long as they are relevant \
        to server themes and remain civil, discussions may include political elements, but discussing \
        politics directly is outside of the server scope."
        .to_string(),
      DefaultReasons::NonDiscussionChannel =>
        "Please note that this is a non-discussion channel. If you would like to discuss or respond to \
        a message, please create a thread, or you may DM the author if they have the DM-friendly tag."
        .to_string(),
      DefaultReasons::UnwholesomeMeme =>
        "Please help us ensure that memes are wholesome and appropriate for all ages. While we do \
        enjoy a wide range of humor, we also recognize that much of it does not fit the intention of \
        this channel. Please employ discretion when choosing where to share. Thank you!"
        .to_string(),
    }
  }
}

#[derive(Debug, Modal)]
#[name = "Erase Message"]
struct EraseMessageModal {
  #[name = "Reason"]
  #[paragraph]
  #[placeholder = "The reason for deleting the message"]
  #[max_length = 512]
  reason: Option<String>,
}

/// Delete a message and notify the user
///
/// Deletes a message and notifies the user via DM or private thread with an optional reason.
///
/// To use, right-click the message that you want to bookmark, then go to "Apps" > "Erase Message".
#[poise::command(
  ephemeral,
  required_permissions = "MANAGE_MESSAGES",
  default_member_permissions = "MANAGE_MESSAGES",
  context_menu_command = "Erase Message",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn erase_message(
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "The message to delete"] message: serenity::Message,
) -> Result<()> {
  let erase_data = EraseMessageModal::execute(ctx).await?;

  if let Some(erase_data) = erase_data {
    ctx.defer_ephemeral().await?;

    let channel_id: ChannelId = message.channel_id;
    let message_id: MessageId = message.id;
    let reason = erase_data
      .reason
      .unwrap_or("No reason provided.".to_string());
    let audit_log_reason: Option<&str> = Some(reason.as_str());

    ctx
      .http()
      .delete_message(channel_id, message_id, audit_log_reason)
      .await?;

    let occurred_at = chrono::Utc::now();

    let data = ctx.data();
    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;
    let user_id = message.author.id;

    let mut transaction = data.db.start_transaction_with_retry(5).await?;
    let erase_count = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user_id)
      .await?
      .len()
      + 1;
    let erase_count_message = if erase_count == 1 {
      "1 erase recorded".to_string()
    } else {
      format!("{erase_count} erases recorded")
    };

    let mut log_embed = BloomBotEmbed::new();
    let mut dm_embed = BloomBotEmbed::new();

    log_embed = log_embed.title("Message Deleted").description(format!(
      "**Channel**: <#{}>\n**Author**: {} ({})\n**Reason**: {}",
      message.channel_id, message.author, erase_count_message, reason,
    ));
    dm_embed = dm_embed
      .title("A message you sent has been deleted.")
      .description(format!("**Reason**: {reason}"));

    if let Some(attachment) = message.attachments.first() {
      log_embed = log_embed.field("Attachment", attachment.url.clone(), false);
      dm_embed = dm_embed.field("Attachment", attachment.url.clone(), false);
    }

    if !message.content.is_empty() {
      // If longer than 1024 - 6 characters for the embed, truncate to 1024 - 3 for "..."
      let content = if message.content.len() > 1018 {
        format!(
          "{}...",
          message.content.chars().take(1015).collect::<String>()
        )
      } else {
        message.content.clone()
      };

      log_embed = log_embed.field("Message Content", format!("```{content}```"), false);
      dm_embed = dm_embed.field("Message Content", format!("```{content}```"), false);
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
      Some(&reason),
      occurred_at,
    )
    .await?;

    commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(format!(
        "{} Message deleted. User will be notified via DM or private thread.",
        EMOJI.mmcheck
      )),
      true,
    )
    .await?;

    if message
      .author
      .direct_message(ctx, CreateMessage::new().embed(dm_embed.clone()))
      .await
      .is_ok()
    {
    } else {
      let thread_channel = match message.channel_id.to_channel(&ctx).await {
        Ok(channel) => {
          if let Some(guild_channel) = channel.guild() {
            if guild_channel.kind == serenity::ChannelType::Text {
              // If message channel is text channel, we can create thread there
              message.channel_id
            } else {
              // If not a text channel, then create private thread in lounge to avoid failure
              ChannelId::from(501464482996944909)
            }
          } else {
            // If we couldn't convert to GuildChannel, then just default to lounge
            ChannelId::from(501464482996944909)
          }
        }
        Err(_e) => {
          // Default to lounge if channel retrieval request failed
          ChannelId::from(501464482996944909)
        }
      };

      let mut notification_thread = thread_channel
        .create_thread(
          ctx,
          CreateThread::new("Private Notification: Message Deleted".to_string()),
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
            .embed(dm_embed.clone())
            .allowed_mentions(CreateAllowedMentions::new().users([message.author.id])),
        )
        .await?;
    }
  }

  Ok(())
}

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
#[allow(clippy::unused_async)]
pub async fn erase(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Delete a message and notify the user
///
/// Deletes a message and notifies the user via DM or private thread with an optional reason.
#[poise::command(slash_command)]
pub async fn message(
  ctx: Context<'_>,
  #[description = "The message to delete"] message: serenity::Message,
  #[max_length = 512] // Max length for audit log reason
  #[description = "The reason for deleting the message"]
  reason: Option<String>,
  #[description = "Choose a predefined default reason"] default_reason: Option<DefaultReasons>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let channel_id: ChannelId = message.channel_id;
  let message_id: MessageId = message.id;
  let reason = match reason {
    Some(custom_reason) => custom_reason,
    None => {
      if let Some(default_reason) = default_reason {
        default_reason.response()
      } else {
        "No reason provided.".to_string()
      }
    }
  };
  let audit_log_reason: Option<&str> = Some(reason.as_str());

  ctx
    .http()
    .delete_message(channel_id, message_id, audit_log_reason)
    .await?;

  let occurred_at = chrono::Utc::now();

  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = message.author.id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let erase_count = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user_id)
    .await?
    .len()
    + 1;
  let erase_count_message = if erase_count == 1 {
    "1 erase recorded".to_string()
  } else {
    format!("{erase_count} erases recorded")
  };

  let mut log_embed = BloomBotEmbed::new();
  let mut dm_embed = BloomBotEmbed::new();

  log_embed = log_embed.title("Message Deleted").description(format!(
    "**Channel**: <#{}>\n**Author**: {} ({})\n**Reason**: {}",
    message.channel_id, message.author, erase_count_message, reason,
  ));
  dm_embed = dm_embed
    .title("A message you sent has been deleted.")
    .description(format!("**Reason**: {reason}"));

  if let Some(attachment) = message.attachments.first() {
    log_embed = log_embed.field("Attachment", attachment.url.clone(), false);
    dm_embed = dm_embed.field("Attachment", attachment.url.clone(), false);
  }

  if !message.content.is_empty() {
    // If longer than 1024 - 6 characters for the embed, truncate to 1024 - 3 for "..."
    let content = if message.content.len() > 1018 {
      format!(
        "{}...",
        message.content.chars().take(1015).collect::<String>()
      )
    } else {
      message.content.clone()
    };

    log_embed = log_embed.field("Message Content", format!("```{content}```"), false);
    dm_embed = dm_embed.field("Message Content", format!("```{content}```"), false);
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
    Some(&reason),
    occurred_at,
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Message deleted. User will be notified via DM or private thread.",
      EMOJI.mmcheck
    )),
    true,
  )
  .await?;

  if message
    .author
    .direct_message(ctx, CreateMessage::new().embed(dm_embed.clone()))
    .await
    .is_ok()
  {
  } else {
    let thread_channel = match message.channel_id.to_channel(&ctx).await {
      Ok(channel) => {
        if let Some(guild_channel) = channel.guild() {
          if guild_channel.kind == serenity::ChannelType::Text {
            // If message channel is text channel, we can create thread there
            message.channel_id
          } else {
            // If not a text channel, then create private thread in lounge to avoid failure
            ChannelId::from(501464482996944909)
          }
        } else {
          // If we couldn't convert to GuildChannel, then just default to lounge
          ChannelId::from(501464482996944909)
        }
      }
      Err(_e) => {
        // Default to lounge if channel retrieval request failed
        ChannelId::from(501464482996944909)
      }
    };

    let mut notification_thread = thread_channel
      .create_thread(
        ctx,
        CreateThread::new("Private Notification: Message Deleted".to_string()),
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
          .embed(dm_embed.clone())
          .allowed_mentions(CreateAllowedMentions::new().users([message.author.id])),
      )
      .await?;
  }

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
  #[description = "Date format (Defaults to YYYY-MM-DD)"] date_format: Option<DateFormat>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_nick_or_name = user
    .nick_in(&ctx, guild_id)
    .await
    .unwrap_or_else(|| user.global_name.as_ref().unwrap_or(&user.name).clone());

  let privacy = ctx.channel_id() != CHANNELS.logs;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  let erases = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user.id).await?;
  let erases: Vec<PageRowRef> = erases.iter().map(|erase| erase as _).collect();
  drop(transaction);
  let pagination = Pagination::new(
    format!("Erases for {user_nick_or_name}"),
    erases,
    ENTRIES_PER_PAGE,
  )
  .await?;

  if pagination.get_page(current_page).is_none() {
    current_page = pagination.get_last_page_number();
  }

  let first_page = match date_format {
    Some(DateFormat::Dmy) => pagination.create_page_embed(current_page, PageType::Alternate),
    _ => pagination.create_page_embed(current_page, PageType::Standard),
  };

  ctx
    .send({
      let mut f = CreateReply::default();
      if pagination.get_page_count() > 1 {
        f = f.components(vec![CreateActionRow::Buttons(vec![
          CreateButton::new(&prev_button_id).label("Previous"),
          CreateButton::new(&next_button_id).label("Next"),
        ])]);
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
    if let Some(DateFormat::Dmy) = date_format {
      press
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .embed(pagination.create_page_embed(current_page, PageType::Alternate)),
          ),
        )
        .await?;
    } else {
      press
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .embed(pagination.create_page_embed(current_page, PageType::Standard)),
          ),
        )
        .await?;
    }
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
  #[description = "The reason for the erasure"] reason: Option<String>,
  #[description = "Choose a predefined default reason"] default_reason: Option<DefaultReasons>,
  #[description = "The date of the erasure (YYYY-MM-DD)"]
  #[rename = "date"]
  erase_date: chrono::NaiveDate,
  #[description = "The time of the erasure (HH:MM)"]
  #[rename = "time"]
  erase_time: Option<chrono::NaiveTime>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let erase_time = erase_time.unwrap_or(
    chrono::NaiveTime::from_hms_opt(0, 0, 0)
      .with_context(|| "Failed to assign hardcoded 00:00:00 NaiveTime to erase_time")?,
  );

  let datetime = chrono::NaiveDateTime::new(erase_date, erase_time).and_utc();

  let reason = match reason {
    Some(custom_reason) => custom_reason,
    None => {
      if let Some(default_reason) = default_reason {
        default_reason.response()
      } else {
        "No reason provided.".to_string()
      }
    }
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  DatabaseHandler::add_erase(
    &mut transaction,
    &guild_id,
    &user.id,
    &message_link,
    Some(&reason),
    datetime,
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Erase data has been added.", EMOJI.mmcheck)),
    true,
  )
  .await?;

  Ok(())
}
