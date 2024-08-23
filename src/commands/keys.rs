use crate::commands::{commit_and_say, MessageType};
use crate::config::ENTRIES_PER_PAGE;
use crate::database::DatabaseHandler;
use crate::pagination::{PageRowRef, Pagination};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*, Mentionable};
use poise::CreateReply;

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
  //hide_in_help,
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
pub async fn list_keys(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  let keys = DatabaseHandler::get_all_steam_keys(&mut transaction, &guild_id).await?;
  let keys: Vec<PageRowRef> = keys.iter().map(|key| key as PageRowRef).collect();
  drop(transaction);
  let pagination = Pagination::new("Playne Keys", keys, ENTRIES_PER_PAGE).await?;

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
        ])]);
      }
      f.embeds = vec![first_page];
      f.ephemeral(true)
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

/// Add a Playne key to the database
///
/// Adds a Playne key to the database.
#[poise::command(slash_command, rename = "add")]
pub async fn add_key(
  ctx: Context<'_>,
  #[description = "The Playne key to add"] key: String,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  if DatabaseHandler::steam_key_exists(&mut transaction, &guild_id, key.as_str()).await? {
    ctx
      .send(
        CreateReply::default()
          .content(":x: Key already exists.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::add_steam_key(&mut transaction, &guild_id, key.as_str()).await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Key has been added.".to_string()),
    true,
  )
  .await?;

  Ok(())
}

/// Remove a Playne key from the database
///
/// Removes a Playne key from the database.
#[poise::command(slash_command, rename = "remove")]
pub async fn remove_key(
  ctx: Context<'_>,
  #[description = "The Playne key to remove"] key: String,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::steam_key_exists(&mut transaction, &guild_id, key.as_str()).await? {
    ctx
      .send(
        CreateReply::default()
          .content(":x: Key does not exist.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_steam_key(&mut transaction, &guild_id, key.as_str()).await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Key has been removed.".to_string()),
    true,
  )
  .await?;

  Ok(())
}

/// Retrieve a Playne key
///
/// Selects an unused Playne key from the database, returning it and marking it as used.
#[poise::command(slash_command, rename = "use")]
pub async fn use_key(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  if DatabaseHandler::unused_key_exists(&mut transaction, &guild_id).await? {
    let key = DatabaseHandler::get_key_and_mark_used(&mut transaction, &guild_id)
      .await?
      .with_context(|| "Failed to retrieve key despite unused_key_exists returning true")?;

    ctx
      .send(
        CreateReply::default()
          .content(format!(
            ":white_check_mark: Key retrieved and marked used: `{key}`"
          ))
          .ephemeral(true),
      )
      .await?;
  } else {
    ctx
      .send(
        CreateReply::default()
          .content(":x: No unused keys found.")
          .ephemeral(true),
      )
      .await?;
  };

  Ok(())
}

/// Commands for managing Playne key recipients
///
/// Commands to list or manage entries in the Playne key recipients database.
#[poise::command(slash_command, subcommands("list_recipients", "update_recipient"))]
#[allow(clippy::unused_async)]
pub async fn recipients(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// List all Playne key recipients in the database
///
/// Lists all Playne key recipients in the database.
#[poise::command(slash_command, rename = "list")]
pub async fn list_recipients(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  let recipients = DatabaseHandler::get_steamkey_recipients(&mut transaction, &guild_id).await?;
  let recipients: Vec<PageRowRef> = recipients
    .iter()
    .map(|recipient| recipient as PageRowRef)
    .collect();
  drop(transaction);
  let pagination = Pagination::new("Playne Key Recipients", recipients, ENTRIES_PER_PAGE).await?;

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
        ])]);
      }
      f.embeds = vec![first_page];
      f.ephemeral(true)
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

/// Update the Playne key recipient database
///
/// Updates the Playne key recipient database.
///
/// If data is provided for a recipient not in the database, a new entry will be created. If data is provided for an existing recipient, the recipient's data will be updated. Specifying zero total keys for an existing recipient will remove that recipient from the database.
#[poise::command(slash_command, rename = "update")]
pub async fn update_recipient(
  ctx: Context<'_>,
  #[description = "Playne key recipient"] recipient: serenity::User,
  #[description = "Received key as challenge prize"] challenge_prize: Option<bool>,
  #[description = "Received key as donator perk"] donator_perk: Option<bool>,
  #[description = "Total number of Playne keys received"]
  #[min = 0]
  total_keys: Option<i16>,
) -> Result<()> {
  if challenge_prize.is_none() && donator_perk.is_none() && total_keys.is_none() {
    ctx
      .send(
        CreateReply::default()
          .content(":x: No input provided. Update aborted.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let steamkey_recipient =
    DatabaseHandler::get_steamkey_recipient(&mut transaction, &guild_id, &recipient.id).await?;

  if steamkey_recipient.is_none() {
    if let Some(total_keys) = total_keys {
      DatabaseHandler::add_steamkey_recipient(
        &mut transaction,
        &guild_id,
        &recipient.id,
        challenge_prize,
        donator_perk,
        total_keys,
      )
      .await?;

      commit_and_say(
        ctx,
        transaction,
        MessageType::TextOnly(
          ":white_check_mark: Recipient has been added to the database.".to_string(),
        ),
        true,
      )
      .await?;
      return Ok(());
    }

    ctx
      .send(CreateReply::default().content(":x: No existing record for recipient. Please specify a number of keys to create a new record.").ephemeral(true))
      .await?;
    DatabaseHandler::rollback_transaction(transaction).await?;
    return Ok(());
  }

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
            return Ok(());
          }
          Err(e) => {
            DatabaseHandler::rollback_transaction(transaction).await?;
            return Err(anyhow::anyhow!(
              "Failed to tell user that {} ({}) was removed from the recipient database: {}",
              recipient.name,
              recipient.id,
              e,
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
    return Ok(());
  }

  let steamkey_recipient = steamkey_recipient
    .with_context(|| "Failed to assign SteamKeyRecipientData to steamkey_recipient")?;
  let challenge_prize = match challenge_prize {
    Some(_) => challenge_prize,
    None => steamkey_recipient.challenge_prize,
  };
  let donator_perk = match donator_perk {
    Some(_) => donator_perk,
    None => steamkey_recipient.donator_perk,
  };
  let total_keys = match total_keys {
    Some(total_keys) => total_keys,
    None => steamkey_recipient.total_keys,
  };

  DatabaseHandler::update_steamkey_recipient(
    &mut transaction,
    &guild_id,
    &recipient.id,
    challenge_prize,
    donator_perk,
    total_keys,
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Recipient has been updated.".to_string()),
    true,
  )
  .await?;

  Ok(())
}
