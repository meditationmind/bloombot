use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::CreateReply;
use poise::serenity_prelude::{ButtonStyle, ChannelId, builder::*};
use poise::serenity_prelude::{ComponentInteractionCollector, Mentionable, User};

use crate::Context;
use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ENTRIES_PER_PAGE};
use crate::data::steam_key::{Recipient, SteamKey};
use crate::database::DatabaseHandler;

/// Commands for managing Playne keys
///
/// Commands to list, add, remove, or use Playne keys.
///
/// Requires `Administrator` permissions.
#[poise::command(
  slash_command,
  required_permissions = "ADMINISTRATOR",
  default_member_permissions = "ADMINISTRATOR",
  category = "Admin Commands",
  subcommands("list_keys", "add_key", "remove_key", "use_key", "recipients"),
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn keys(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// List all Playne keys in the database
///
/// Lists all Playne keys in the database.
#[poise::command(slash_command, rename = "list")]
async fn list_keys(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let keys = DatabaseHandler::get_all_steam_keys(&mut transaction, &guild_id).await?;
  let keys: Vec<PageRowRef> = keys.iter().map(|key| key as PageRowRef).collect();

  drop(transaction);

  Paginator::new("Playne Keys", &keys, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Add a Playne key to the database
///
/// Adds a Playne key to the database.
#[poise::command(slash_command, rename = "add")]
async fn add_key(
  ctx: Context<'_>,
  #[description = "The Playne key to add"] key: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  if DatabaseHandler::steam_key_exists(&mut transaction, &guild_id, key.as_str()).await? {
    let msg = format!("{} Key already exists.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  DatabaseHandler::add_steam_key(&mut transaction, &SteamKey::new(guild_id, key)).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Key has been added.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Remove a Playne key from the database
///
/// Removes a Playne key from the database.
#[poise::command(slash_command, rename = "remove")]
async fn remove_key(
  ctx: Context<'_>,
  #[description = "The Playne key to remove"] key: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::steam_key_exists(&mut transaction, &guild_id, key.as_str()).await? {
    let msg = format!("{} Key does not exist.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_steam_key(&mut transaction, &guild_id, key.as_str()).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Key has been removed.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Retrieve a Playne key
///
/// Selects an unused Playne key from the database, returning it and marking it as used.
#[poise::command(slash_command, rename = "use")]
async fn use_key(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if !DatabaseHandler::unused_key_exists(&mut transaction, &guild_id).await? {
    let msg = format!("{} No unused keys found.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  };

  let key = DatabaseHandler::get_key_and_mark_used(&mut transaction, &guild_id)
    .await?
    .with_context(|| "Failed to retrieve key despite unused_key_exists returning true")?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Key retrieved and marked used: `{key}`",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  let log_embed = BloomBotEmbed::new()
    .title("Playne Key Retrieved")
    .description(format!("**Key**: `{key}`"))
    .footer(
      CreateEmbedFooter::new(format!(
        "Retrieved by {} ({})",
        ctx.author().name,
        ctx.author().id
      ))
      .icon_url(ctx.author().avatar_url().unwrap_or_default()),
    );

  let log_channel = ChannelId::new(CHANNELS.bloomlogs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}

/// Commands for managing Playne key recipients
///
/// Commands to list or manage entries in the Playne key recipients database.
#[poise::command(slash_command, subcommands("list_recipients", "update_recipient"))]
#[allow(clippy::unused_async)]
async fn recipients(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// List all Playne key recipients in the database
///
/// Lists all Playne key recipients in the database.
#[poise::command(slash_command, rename = "list")]
async fn list_recipients(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let recipients = DatabaseHandler::get_steamkey_recipients(&mut transaction, &guild_id).await?;
  let recipients: Vec<PageRowRef> = recipients.iter().map(|recip| recip as PageRowRef).collect();

  drop(transaction);

  let title = "Playne Key Recipients";
  Paginator::new(title, &recipients, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Update the Playne key recipient database
///
/// Updates the Playne key recipient database.
///
/// If data is provided for a recipient not in the database, a new entry will be created. If data is provided for an existing recipient, the recipient's data will be updated. Specifying zero total keys for an existing recipient will remove that recipient from the database.
#[poise::command(slash_command, rename = "update")]
async fn update_recipient(
  ctx: Context<'_>,
  #[description = "Playne key recipient"] recipient: User,
  #[description = "Received key as challenge prize"] challenge_prize: Option<bool>,
  #[description = "Received key as donator perk"] donator_perk: Option<bool>,
  #[description = "Total number of Playne keys received"]
  #[min = 0]
  total_keys: Option<i16>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  if challenge_prize.is_none() && donator_perk.is_none() && total_keys.is_none() {
    let msg = format!("{} No input provided. Update aborted.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let Some(steamkey_recipient) =
    DatabaseHandler::get_steamkey_recipient(&mut transaction, &guild_id, &recipient.id).await?
  else {
    if let Some(total_keys) = total_keys {
      let recipient = Recipient::new(
        guild_id,
        recipient.id,
        challenge_prize,
        donator_perk,
        total_keys,
      );
      DatabaseHandler::add_steamkey_recipient(&mut transaction, &recipient).await?;

      database::commit_and_say(
        ctx,
        transaction,
        MessageType::TextOnly(format!(
          "{} Recipient has been added to the database.",
          EMOJI.mmcheck
        )),
        Visibility::Ephemeral,
      )
      .await?;
      return Ok(());
    }

    let msg = format!(
      "{} No existing record for recipient. Please specify a number of keys to create a new record.",
      EMOJI.mminfo
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    DatabaseHandler::rollback_transaction(transaction).await?;
    return Ok(());
  };

  if total_keys.is_some_and(|total| total == 0) {
    DatabaseHandler::remove_steamkey_recipient(&mut transaction, &guild_id, &recipient.id).await?;

    let ctx_id = ctx.id();
    let confirm_id = format!("{ctx_id}confirm");
    let cancel_id = format!("{ctx_id}cancel");

    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "Are you sure you want to remove {} from the recipient database?",
            recipient.mention()
          ))
          .ephemeral(true)
          .components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new(confirm_id.as_str())
              .label("Yes")
              .style(ButtonStyle::Success),
            CreateButton::new(cancel_id.as_str())
              .label("No")
              .style(ButtonStyle::Danger),
          ])]),
      )
      .await?;

    // Loop through incoming interactions with the buttons.
    while let Some(press) = ComponentInteractionCollector::new(ctx)
      // Only collect presses when button IDs start with ctx_id.
      .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
      // Timeout when no button has been pressed for one minute.
      .timeout(Duration::from_secs(60))
      .await
    {
      if press.data.custom_id != confirm_id && press.data.custom_id != cancel_id {
        // This is an unrelated button interaction.
        continue;
      }

      let confirmed = press.data.custom_id == confirm_id;

      // Update the response.
      if confirmed {
        let msg = CreateInteractionResponseMessage::new()
          .content(format!("{} Confirmed.", EMOJI.mmcheck))
          .components(Vec::new());
        if let Err(e) = press
          .create_response(ctx, CreateInteractionResponse::UpdateMessage(msg))
          .await
        {
          DatabaseHandler::rollback_transaction(transaction).await?;
          return Err(anyhow!(
            "Failed to tell user that {} ({}) was removed from the recipient database: {e}",
            recipient.name,
            recipient.id,
          ));
        }
        DatabaseHandler::commit_transaction(transaction).await?;
        return Ok(());
      }

      let msg = CreateInteractionResponseMessage::new()
        .content(format!("{} Cancelled.", EMOJI.mmx))
        .components(Vec::new());
      press
        .create_response(ctx, CreateInteractionResponse::UpdateMessage(msg))
        .await?;
    }
    // This happens when the user didn't press any button for 60 seconds.
    return Ok(());
  }

  let challenge_prize = challenge_prize.or(steamkey_recipient.challenge_prize);
  let donator_perk = donator_perk.or(steamkey_recipient.donator_perk);
  let total_keys = total_keys.unwrap_or(steamkey_recipient.total_keys);

  let recipient = Recipient::new(
    guild_id,
    recipient.id,
    challenge_prize,
    donator_perk,
    total_keys,
  );

  DatabaseHandler::update_steamkey_recipient(&mut transaction, &recipient).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Recipient has been updated.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}
