use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::Message;
use poise::Context as PoiseContext;
use poise::{ApplicationContext, CreateReply, Modal};

use crate::commands::helpers::common::{self, Visibility};
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{EMOJI, ENTRIES_PER_PAGE};
use crate::data::bookmark::Bookmark;
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};

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
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "Message to bookmark"] message: Message,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let supporter = common::is_supporter(PoiseContext::Application(ctx)).await?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let bookmark_count =
    DatabaseHandler::get_bookmark_count(&mut transaction, &guild_id, &user_id).await?;

  if !supporter && bookmark_count > 19 {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} Sorry, you've reached the bookmark limit. Please remove one and try again.\n-# Subscription-based supporters can add unlimited bookmarks. [Learn more.](<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>)",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  if let Some(bookmark) = AddBookmarkModal::execute(ctx).await? {
    let new_bookmark = Bookmark::new(guild_id, user_id, message.link(), bookmark.description);

    DatabaseHandler::add_bookmark(&mut transaction, &new_bookmark).await?;

    database::commit_and_say(
      PoiseContext::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Bookmark has been added.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
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
pub async fn bookmark(_: PoiseContext<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// List your bookmarks
///
/// View a list of your bookmarks.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let bookmarks = DatabaseHandler::get_bookmarks(&mut transaction, &guild_id, &user_id).await?;
  let bookmarks: Vec<PageRowRef> = bookmarks
    .iter()
    .map(|bookmark| bookmark as PageRowRef)
    .collect();

  drop(transaction);

  Paginator::new("Your Bookmarks", &bookmarks, ENTRIES_PER_PAGE.bookmarks)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Add a message to your bookmarks
///
/// Adds a message to your bookmarks, with an optional short description.
#[poise::command(slash_command)]
async fn add(
  ctx: Context<'_>,
  #[description = "Message to bookmark (message link)"] message: Message,
  #[max_length = 100]
  #[description = "Include a short description (optional)"]
  description: Option<String>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let supporter = common::is_supporter(ctx).await?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let bookmark_count =
    DatabaseHandler::get_bookmark_count(&mut transaction, &guild_id, &user_id).await?;

  if !supporter && bookmark_count > 19 {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} Sorry, you've reached the bookmark limit. Please remove one and try again.\n-# Subscription-based supporters can add unlimited bookmarks. [Learn more.](<https://discord.com/channels/244917432383176705/1030424719138246667/1031137243345211413>)",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let new_bookmark = Bookmark::new(guild_id, user_id, message.link(), description);

  DatabaseHandler::add_bookmark(&mut transaction, &new_bookmark).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Bookmark has been added.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Remove a bookmark
///
/// Removes one of your bookmarks.
#[poise::command(slash_command)]
async fn remove(
  ctx: Context<'_>,
  #[description = "The ID of the bookmark to remove"] id: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let bookmark_id = id.to_ascii_uppercase();

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let result =
    DatabaseHandler::remove_bookmark(&mut transaction, &guild_id, bookmark_id.as_str()).await?;
  if result > 0 {
    database::commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(format!("{} Bookmark has been removed.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
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
async fn search(
  ctx: Context<'_>,
  #[description = "One or more keywords in search engine format"] keyword: String,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let user_id = ctx.author().id;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

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
  Paginator::new("Bookmark Search Results", &bookmarks, bookmarks_per_page)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}
