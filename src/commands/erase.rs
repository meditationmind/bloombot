use std::time::Duration;

use anyhow::{anyhow, Context as AnyhowContext, Result};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use poise::serenity_prelude::{builder::*, ChannelId, ChannelType, ComponentInteractionCollector};
use poise::serenity_prelude::{ComponentInteractionDataKind, CreateQuickModal, InputTextStyle};
use poise::serenity_prelude::{Message, User};
use poise::{ApplicationContext, ChoiceParameter, Context as PoiseContext, CreateReply};
use sqlx::{Postgres, Transaction};

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ENTRIES_PER_PAGE};
use crate::data::erase::Erase;
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};

#[derive(ChoiceParameter)]
enum DateFormat {
  #[name = "YYYY-MM-DD (ISO 8601)"]
  Ymd,
  #[name = "DD Month YYYY"]
  Dmy,
}

#[derive(ChoiceParameter)]
enum DefaultReasons {
  #[name = "Rule 1: Be kind"]
  Rule1BeKind,
  #[name = "Rule 2: Be respectful"]
  Rule2BeRespectful,
  #[name = "Rule 3: All-ages appropriate"]
  Rule3AllAges,
  #[name = "Rule 4: Respect boundaries"]
  Rule4RespectBoundaries,
  #[name = "Rule 8: Self-promo"]
  Rule8SelfPromo,
  #[name = "Rule 9: Respect IP rights"]
  Rule9IPRights,
  #[name = "Rule 10: Drug chat"]
  Rule10Drugs,
  #[name = "Rule 10: Politics"]
  Rule10Politics,
  #[name = "Rule 10: Disinformation"]
  Rule10Disinformation,
  #[name = "Mental health guidelines"]
  MentalHealth,
  #[name = "Non-discussion channel"]
  NonDiscussionChannel,
  #[name = "Unwholesome meme"]
  UnwholesomeMeme,
  #[name = "Excessive venting"]
  ExcessiveVenting,
  #[name = "None"]
  None,
}

impl DefaultReasons {
  fn response(&self) -> String {
    match *self {
      DefaultReasons::Rule1BeKind => {
        "Please help us cultivate a warm and welcoming atmosphere by remaining civil \
        and treating others with kindness. Disagreeing with others, challenging views, \
        questioning actions/behavior, etc. can all be done respectfully and mindfully."
          .to_string()
      }
      DefaultReasons::Rule2BeRespectful => {
        "Please help us cultivate an atmosphere of respect. This is a diverse and inclusive \
        community; all ages, genders, religions, and traditions are welcome. Questioning \
        identities or views may be done respectfully, as long as the other person exhibits a \
        willingness to engage."
          .to_string()
      }
      DefaultReasons::Rule3AllAges => {
        "This is an all-ages server. Please help us cultivate a wholesome atmosphere by being \
        mindful of excessive profanity and vulgar or violent language or content. Try to limit \
        mature themes, such as sexual or potentially triggering topics, to the \
        <#1020856801115246702> forum with the `Mature Topic` tag applied."
          .to_string()
      }
      DefaultReasons::Rule4RespectBoundaries => {
        "We love having fun! A bit of jesting or banter is fine, as long as it is good-natured \
        and consensual. If the other party ever seems uncomfortable or asks you to stop, respect \
        their wishes, even if you were engaging with good intentions."
          .to_string()
      }
      DefaultReasons::Rule8SelfPromo => {
        "Advertising, recruitment, solicitation, and self-promo require approval. This server is \
        meant to be a safe space, where community members can feel confident they're interacting \
        with fellow members who care about the community, and self-promo can violate that sense \
        of safety. While exceptions are rare, you may contact <@575252669443211264> to request \
        authorization."
          .to_string()
      }
      DefaultReasons::Rule9IPRights => {
        "Sharing content that violates intellectual property rights is prohibited by the Discord \
        Community Guidelines."
          .to_string()
      }
      DefaultReasons::Rule10Drugs => {
        "As an all-ages Discord Partner community accessible via Server Discovery, \
        discussion of drugs/controlled substances is prohibited by Discord's Discovery \
        guidelines. Please help us cultivate and maintain an environment that is appropriate \
        for all of our diverse membership. Thank you!"
          .to_string()
      }
      DefaultReasons::Rule10Politics => {
        "Due to the strong tendency for political discussions to become highly charged and \
        promote polarized views, we ask that members choose a different outlet. As long as they \
        are relevant to server themes and remain civil, discussions may include political \
        elements, but discussing politics directly is outside of the server scope."
          .to_string()
      }
      DefaultReasons::Rule10Disinformation => {
        "Sharing potentially harmful misinformation, such as conspiracy theories or fake news, \
        is prohibited by the Discord Community Guidelines. Even seemingly harmless conspiracy \
        theories serve to spread misinformation and are rarely relevant to server themes, \
        so we ask that such discussion be avoided."
          .to_string()
      }
      DefaultReasons::MentalHealth => {
        "This is not a mental health server. For everyone's safety, we ask that members avoid \
        diagnosing others and giving or requesting personalized advice regarding diagnosable \
        mental health conditions or medications. See our Mental Health Discussion Guidelines: \
        <#809814483874480138>"
          .to_string()
      }
      DefaultReasons::NonDiscussionChannel => {
        "Please note that this is a non-discussion channel. If you would like to discuss or \
        respond to a message, please create a thread, or you may DM the author if they have the \
        DM-friendly tag."
          .to_string()
      }
      DefaultReasons::UnwholesomeMeme => {
        "Please help us ensure that memes are wholesome and appropriate for all ages. While we do \
        enjoy a wide range of humor, we also recognize that much of it does not fit the intention \
        of this channel. Please employ discretion when choosing where to share. Thank you!"
          .to_string()
      }
      DefaultReasons::ExcessiveVenting => {
        "We understand that sometimes people just want to let off some steam, which is why we \
        have the `#venting` and `#venting-void` channels. We ask that venting be limited to \
        these channels. If you are looking for advice or feedback, consider posting in \
        <#1020856801115246702>. Thank you!"
          .to_string()
      }
      DefaultReasons::None => "No reason provided.".to_string(),
    }
  }
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
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "The message to delete"] message: Message,
) -> Result<()> {
  ctx.defer_ephemeral().await?;
  let ctx_id = ctx.id();

  let reply = {
    #[rustfmt::skip]
    let options = vec![
      CreateSelectMenuOption::new("Custom", "custom"),
      CreateSelectMenuOption::new(DefaultReasons::Rule1BeKind.name(), "Rule1BeKind"),
      CreateSelectMenuOption::new(DefaultReasons::Rule2BeRespectful.name(), "Rule2BeRespectful"),
      CreateSelectMenuOption::new(DefaultReasons::Rule3AllAges.name(), "Rule3AllAges"),
      CreateSelectMenuOption::new(DefaultReasons::Rule4RespectBoundaries.name(), "Rule4RespectBoundaries"),
      CreateSelectMenuOption::new(DefaultReasons::Rule8SelfPromo.name(), "Rule8SelfPromo"),
      CreateSelectMenuOption::new(DefaultReasons::Rule9IPRights.name(), "Rule9IPRights"),
      CreateSelectMenuOption::new(DefaultReasons::Rule10Drugs.name(), "Rule10Drugs"),
      CreateSelectMenuOption::new(DefaultReasons::Rule10Politics.name(), "Rule10Politics"),
      CreateSelectMenuOption::new(DefaultReasons::Rule10Disinformation.name(), "Rule10Disinformation"),
      CreateSelectMenuOption::new(DefaultReasons::MentalHealth.name(), "MentalHealth"),
      CreateSelectMenuOption::new(DefaultReasons::NonDiscussionChannel.name(), "NonDiscussionChannel"),
      CreateSelectMenuOption::new(DefaultReasons::UnwholesomeMeme.name(), "UnwholesomeMeme"),
      CreateSelectMenuOption::new(DefaultReasons::ExcessiveVenting.name(), "ExcessiveVenting"),
      CreateSelectMenuOption::new(DefaultReasons::None.name(), "None"),
      CreateSelectMenuOption::new("Cancel erase", "cancel"),
    ];
    let default_reason_dropdown = vec![CreateActionRow::SelectMenu(
      CreateSelectMenu::new(
        format!("{ctx_id}"),
        CreateSelectMenuKind::String { options },
      )
      .placeholder("Choose a reason"),
    )];

    CreateReply::default()
      .components(default_reason_dropdown)
      .ephemeral(true)
  };

  let msg = ctx.send(reply).await?;

  while let Some(mci) = ComponentInteractionCollector::new(ctx)
    .author_id(ctx.author().id)
    .channel_id(ctx.channel_id())
    .timeout(Duration::from_secs(60))
    .filter(move |mci| mci.data.custom_id == ctx_id.to_string())
    .await
  {
    let choice = match &mci.data.kind {
      ComponentInteractionDataKind::StringSelect { values } => &values[0],
      _ => return Err(anyhow!("Unexpected interaction data kind")),
    };

    if choice == "cancel" {
      mci
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .content(format!("{} Erase cancelled.", EMOJI.mminfo))
              .ephemeral(true)
              .components(Vec::new()),
          ),
        )
        .await?;

      return Ok(());
    }

    let erase_data = if choice == "custom" {
      mci
        .quick_modal(
          ctx.serenity_context,
          CreateQuickModal::new("Erase Message")
            .timeout(Duration::from_secs(600))
            .field(
              CreateInputText::new(InputTextStyle::Paragraph, "Reason", "")
                .max_length(512)
                .placeholder("The reason for deleting the message"),
            ),
        )
        .await?
    } else {
      None
    };

    // This happens when the user selects custom and then clicks cancel.
    // The modal closes, but we have no way of knowing until it times out.
    // Once it times out, we edit the ephemeral response appropriately.
    if choice == "custom" && erase_data.is_none() {
      mci
        .edit_response(
          ctx,
          EditInteractionResponse::new()
            .content(format!("{} Erase cancelled.", EMOJI.mminfo))
            .components(Vec::new()),
        )
        .await?;
      return Ok(());
    }

    let reason = if let Some(erase_data) = &erase_data {
      &erase_data.inputs[0]
    } else {
      match choice.as_str() {
        "Rule1BeKind" => &DefaultReasons::Rule1BeKind.response(),
        "Rule2BeRespectful" => &DefaultReasons::Rule2BeRespectful.response(),
        "Rule3AllAges" => &DefaultReasons::Rule3AllAges.response(),
        "Rule4RespectBoundaries" => &DefaultReasons::Rule4RespectBoundaries.response(),
        "Rule8SelfPromo" => &DefaultReasons::Rule8SelfPromo.response(),
        "Rule9IPRights" => &DefaultReasons::Rule9IPRights.response(),
        "Rule10Drugs" => &DefaultReasons::Rule10Drugs.response(),
        "Rule10Politics" => &DefaultReasons::Rule10Politics.response(),
        "Rule10Disinformation" => &DefaultReasons::Rule10Disinformation.response(),
        "MentalHealth" => &DefaultReasons::MentalHealth.response(),
        "NonDiscussionChannel" => &DefaultReasons::NonDiscussionChannel.response(),
        "UnwholesomeMeme" => &DefaultReasons::UnwholesomeMeme.response(),
        "ExcessiveVenting" => &DefaultReasons::ExcessiveVenting.response(),
        _ => &DefaultReasons::None.response(),
      }
    };

    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    let dm_embed = erase_and_log(
      PoiseContext::Application(ctx),
      &mut transaction,
      &message,
      reason,
    )
    .await?;

    DatabaseHandler::commit_transaction(transaction).await?;

    let response = CreateInteractionResponse::UpdateMessage(
      CreateInteractionResponseMessage::new()
        .content(format!(
          "{} Message deleted. User will be notified via DM or private thread.",
          EMOJI.mmcheck
        ))
        .ephemeral(true)
        .components(Vec::new()),
    );

    if let Some(qmr) = erase_data {
      qmr.interaction.create_response(ctx, response).await?;
    } else {
      mci.create_response(ctx, response).await?;
    }

    notify_user(PoiseContext::Application(ctx), &message, dm_embed).await?;
  }

  msg
    .edit(
      PoiseContext::Application(ctx),
      CreateReply::default()
        .content(format!("{} Erase cancelled.", EMOJI.mminfo))
        .components(Vec::new())
        .ephemeral(true),
    )
    .await?;

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
async fn message(
  ctx: Context<'_>,
  #[description = "The message to delete"] message: Message,
  #[max_length = 512] // Max length for audit log reason
  #[description = "The reason for deleting the message"]
  reason: Option<String>,
  #[description = "Choose a predefined default reason"] default_reason: Option<DefaultReasons>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let reason = reason.unwrap_or(default_reason.unwrap_or(DefaultReasons::None).response());

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let dm_embed = erase_and_log(ctx, &mut transaction, &message, &reason).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Message deleted. User will be notified via DM or private thread.",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  notify_user(ctx, &message, dm_embed).await?;

  Ok(())
}

/// List erases for a user
///
/// List erases for a specified user, with dates and links to notification messages, when available.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The user to show erase data for"] user: User,
  #[description = "The page to show"] page: Option<usize>,
  #[description = "Date format (Defaults to YYYY-MM-DD)"] date_format: Option<DateFormat>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let erases = DatabaseHandler::get_erases(&mut transaction, &guild_id, &user.id).await?;
  let erases: Vec<PageRowRef> = erases.iter().map(|erase| erase as PageRowRef).collect();

  drop(transaction);

  let title = {
    let user_nick_or_name = user
      .nick_in(&ctx, guild_id)
      .await
      .unwrap_or_else(|| user.global_name.as_ref().unwrap_or(&user.name).clone());
    format!("Erases for {user_nick_or_name}")
  };

  let page_type = match date_format {
    Some(DateFormat::Dmy) => PageType::Alternate,
    _ => PageType::Standard,
  };

  let visibility = if ctx.channel_id() == CHANNELS.logs {
    Visibility::Public
  } else {
    Visibility::Ephemeral
  };

  Paginator::new(title, &erases, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, page_type, visibility)
    .await?;

  Ok(())
}

/// Populate past erases for a user
///
/// Populate the database with past erases for a user.
#[poise::command(slash_command)]
async fn populate(
  ctx: Context<'_>,
  #[description = "The user to populate erase data for"] user: User,
  #[description = "The link for the erase notification message"] message_link: String,
  #[description = "The reason for the erasure"] reason: Option<String>,
  #[description = "Choose a predefined default reason"] default_reason: Option<DefaultReasons>,
  #[description = "The date of the erasure (YYYY-MM-DD)"]
  #[rename = "date"]
  erase_date: NaiveDate,
  #[description = "The time of the erasure (HH:MM)"]
  #[rename = "time"]
  erase_time: Option<NaiveTime>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let erase_time = erase_time.unwrap_or(
    NaiveTime::from_hms_opt(0, 0, 0)
      .with_context(|| "Failed to assign hardcoded 00:00:00 NaiveTime to erase_time")?,
  );

  let datetime = NaiveDateTime::new(erase_date, erase_time).and_utc();

  let reason = reason.unwrap_or(default_reason.unwrap_or(DefaultReasons::None).response());

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let erase = Erase::new(guild_id, user.id, message_link, reason, &datetime);

  DatabaseHandler::add_erase(&mut transaction, &erase).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Erase data has been added.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Erases a message, logs the erase in the [`CHANNELS.logs`][logs] channel, and returns
/// an embed to be used for private notification. The `transaction` needs to be committed
/// after this function is called or it will be rolled back and the erase will not be
/// added to the database.
///
/// [logs]: crate::config::CHANNELS
async fn erase_and_log(
  ctx: Context<'_>,
  transaction: &mut Transaction<'_, Postgres>,
  message: &Message,
  reason: &String,
) -> Result<CreateEmbed> {
  let channel_id = message.channel_id;
  let message_id = message.id;
  let audit_log_reason = Some(reason.as_str());

  ctx
    .http()
    .delete_message(channel_id, message_id, audit_log_reason)
    .await?;

  let occurred_at = Utc::now();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = message.author.id;

  let erase_count = DatabaseHandler::get_erases(transaction, &guild_id, &user_id)
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
    // If longer than 1018 characters (1024 max - 6 for backticks), truncate to 1015 (-3 for "...").
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
    "If you have any questions or concerns regarding this action, please contact a moderator. Replies sent to Bloom are not viewable by staff.",
  ));

  let log_channel = ChannelId::new(CHANNELS.logs);

  let log_message = log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  let erase = Erase::new(guild_id, user_id, log_message.link(), reason, &occurred_at);

  DatabaseHandler::add_erase(transaction, &erase).await?;

  Ok(dm_embed)
}

/// Notifies a user of deletion via DM, or via private thread if a DM cannot be delivered.
/// Private threads are created in the channel where the message was deleted, when possible.
/// When that fails, the [`CHANNELS.private_thread_default`][ptd] channel is used as a fallback.
///
/// [ptd]: crate::config::CHANNELS
async fn notify_user(ctx: Context<'_>, message: &Message, dm_embed: CreateEmbed) -> Result<()> {
  // First, we try to send the notification via DM.
  if message
    .author
    .direct_message(ctx, CreateMessage::new().embed(dm_embed.clone()))
    .await
    .is_err()
  {
    // If the DM can't be delivered, we create a private thread.
    let thread_channel = if message
      .channel_id
      .to_channel(&ctx)
      .await
      .is_ok_and(|channel| {
        channel
          .guild()
          .is_some_and(|channel| channel.kind == ChannelType::Text)
      }) {
      message.channel_id
    } else {
      // If the message wasn't deleted from a text channel in a server, we use a default channel
      // to create the private thread. This avoids failure when the message was originally posted
      // in a thread, forum, voice channel text chat, etc.
      ChannelId::from(CHANNELS.private_thread_default)
    };

    let mut notification_thread = thread_channel
      .create_thread(
        ctx,
        CreateThread::new("Private Notification: Message Deleted".to_string())
          .kind(ChannelType::PrivateThread),
      )
      .await?;

    // We disable inviting and lock the thread so that only moderators can add other users
    // to the thread or respond. If the user wants to respond, we want them to use ModMail.
    notification_thread
      .edit_thread(ctx, EditThread::new().invitable(false).locked(true))
      .await?;

    let mut thread_embed = dm_embed.clone();

    thread_embed = thread_embed.footer(CreateEmbedFooter::new(
      "If you have any questions or concerns regarding this action, please contact staff via ModMail.",
    ));

    let thread_initial_message = format!("Private notification for <@{}>:", message.author.id);

    notification_thread
      .send_message(
        ctx,
        CreateMessage::new()
          .content(thread_initial_message)
          .embed(thread_embed)
          .allowed_mentions(CreateAllowedMentions::new().users([message.author.id])),
      )
      .await?;
  }
  Ok(())
}
