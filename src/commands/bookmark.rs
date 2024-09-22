use crate::commands::{commit_and_say, MessageType};
use crate::config::{EMOJI, ROLES};
use crate::database::DatabaseHandler;
use crate::pagination::{PageRowRef, PageType, Pagination};
use crate::{Context, Data as AppData, Error as AppError};
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*, RoleId};
use poise::{CreateReply, Modal};

#[derive(Debug, Modal)]
#[name = "Add to Bookmarks"]
struct AddBookmarkModal {
  #[name = "Description"]
  #[placeholder = "Include a short description (optional)"]
  #[max_length = 100]
  description: Option<String>,
}

/// Add a message to your bookmarks
///
/// Adds a message to your bookmarks.
///
/// To use, right-click the message that you want to bookmark, then go to "Apps" > "Add to Bookmarks".
#[poise::command(
  ephemeral,
  context_menu_command = "Add to Bookmarks",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn add_bookmark(
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "Message to bookmark"] message: serenity::Message,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let supporter = {
    if let Some(member) = ctx.author_member().await {
      member.roles.contains(&RoleId::from(ROLES.patreon))
        || member.roles.contains(&RoleId::from(ROLES.kofi))
        || member.roles.contains(&RoleId::from(ROLES.staff))
    } else {
      false
    }
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let bookmark_count =
    DatabaseHandler::get_bookmark_count(&mut transaction, &guild_id, &user_id).await?;

  if !supporter && bookmark_count > 19 {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Sorry, you've reached the bookmark limit. Please remove one and try again.\n-# Subscription-based supporters can add unlimited bookmarks. [Learn more.](<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>)", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let bookmark_data = AddBookmarkModal::execute(ctx).await?;

  if let Some(bookmark) = bookmark_data {
    let message_link = message.link();
    let description = bookmark.description;

    DatabaseHandler::add_bookmark(
      &mut transaction,
      &guild_id,
      &user_id,
      message_link.as_str(),
      description.as_deref(),
    )
    .await?;

    commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Bookmark has been added.", EMOJI.mmcheck)),
      true,
    )
    .await?;
  }

  Ok(())
}

/// Manage your bookmarks
///
/// View your bookmarks or remove a bookmark from your list.
#[poise::command(
  slash_command,
  category = "Informational",
  subcommands("list", "add", "remove", "search"),
  subcommand_required,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn bookmark(_: poise::Context<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// List your bookmarks
///
/// View a list of your bookmarks.
#[poise::command(slash_command)]
pub async fn list(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  let bookmarks = DatabaseHandler::get_bookmarks(&mut transaction, &guild_id, &user_id).await?;
  let bookmarks: Vec<PageRowRef> = bookmarks
    .iter()
    .map(|bookmark| bookmark as PageRowRef)
    .collect();
  drop(transaction);

  let bookmarks_per_page = 5;
  let pagination = Pagination::new("Your Bookmarks", bookmarks, bookmarks_per_page).await?;

  if pagination.get_page(current_page).is_none() {
    current_page = pagination.get_last_page_number();
  }

  let first_page = pagination.create_page_embed(current_page, PageType::Standard);

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
          CreateInteractionResponseMessage::new()
            .embed(pagination.create_page_embed(current_page, PageType::Standard)),
        ),
      )
      .await?;
  }

  Ok(())
}

/// Add a message to your bookmarks
///
/// Adds a message to your bookmarks, with an optional short description.
#[poise::command(slash_command)]
pub async fn add(
  ctx: Context<'_>,
  #[description = "Message to bookmark (message link)"] message: serenity::Message,
  #[max_length = 100]
  #[description = "Include a short description (optional)"]
  description: Option<String>,
) -> Result<()> {
  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let supporter = {
    if let Some(member) = ctx.author_member().await {
      member.roles.contains(&RoleId::from(ROLES.patreon))
        || member.roles.contains(&RoleId::from(ROLES.kofi))
        || member.roles.contains(&RoleId::from(ROLES.staff))
    } else {
      false
    }
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let bookmark_count =
    DatabaseHandler::get_bookmark_count(&mut transaction, &guild_id, &user_id).await?;

  if !supporter && bookmark_count > 19 {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Sorry, you've reached the bookmark limit. Please remove one and try again.\n-# Subscription-based supporters can add unlimited bookmarks. [Learn more.](<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>)", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::add_bookmark(
    &mut transaction,
    &guild_id,
    &user_id,
    message.link().as_str(),
    description.as_deref(),
  )
  .await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Bookmark has been added.", EMOJI.mmcheck)),
    true,
  )
  .await?;

  Ok(())
}

/// Remove a bookmark
///
/// Removes one of your bookmarks.
#[poise::command(slash_command)]
pub async fn remove(
  ctx: Context<'_>,
  #[description = "The ID of the bookmark to remove"] id: String,
) -> Result<()> {
  let data = ctx.data();
  let bookmark_id = id.to_ascii_uppercase().clone();

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let result = DatabaseHandler::remove_bookmark(&mut transaction, bookmark_id.as_str()).await?;
  if result > 0 {
    commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(format!("{} Bookmark has been removed.", EMOJI.mmcheck)),
      true,
    )
    .await?;
  } else {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} Bookmark not found. Please verify the ID and try again.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
  }

  Ok(())
}

/// Search your bookmarks
///
/// Searches your bookmark descriptions using one or more keywords in search engine format. Valid search operators include quotation marks (""), OR, and minus (-).
///
/// Example: "guided meditation" breath or mantra -walking
#[poise::command(slash_command)]
pub async fn search(
  ctx: Context<'_>,
  #[description = "One or more keywords in search engine format"] keyword: String,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  // Define some unique identifiers for the navigation buttons
  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  let bookmarks =
    DatabaseHandler::search_bookmarks(&mut transaction, &guild_id, &user_id, &keyword).await?;

  if bookmarks.is_empty() {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} No bookmarks match your search query.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let bookmarks: Vec<PageRowRef> = bookmarks
    .iter()
    .map(|bookmark| bookmark as PageRowRef)
    .collect();
  drop(transaction);

  let bookmarks_per_page = 5;
  let pagination =
    Pagination::new("Bookmark Search Results", bookmarks, bookmarks_per_page).await?;

  if pagination.get_page(current_page).is_none() {
    current_page = pagination.get_last_page_number();
  }

  let first_page = pagination.create_page_embed(current_page, PageType::Standard);

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
          CreateInteractionResponseMessage::new()
            .embed(pagination.create_page_embed(current_page, PageType::Standard)),
        ),
      )
      .await?;
  }

  Ok(())
}
