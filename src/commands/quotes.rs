use anyhow::{Context as AnyhowContext, Result};
use poise::{ApplicationContext, Context as PoiseContext, CreateReply, Modal};

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{BloomBotEmbed, EMOJI, ENTRIES_PER_PAGE};
use crate::data::quote::QuoteModal;
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};

/// Commands for managing quotes
///
/// Commands to list, add, edit, or remove quotes.
///
/// These quotes are used both for the `/quote` command and for motivational messages when a user runs `/add`.
///
/// Requires `Manage Roles` permissions.
#[poise::command(
  slash_command,
  required_permissions = "MANAGE_ROLES",
  default_member_permissions = "MANAGE_ROLES",
  category = "Moderator Commands",
  subcommands("list", "add", "edit", "remove", "search", "show"),
  subcommand_required,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn quotes(_: PoiseContext<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// Add a quote to the database
///
/// Adds a quote to the database.
#[poise::command(slash_command)]
async fn add(ctx: ApplicationContext<'_, AppData, AppError>) -> Result<()> {
  if let Some(quote_data) = QuoteModal::execute(ctx).await? {
    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;

    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    DatabaseHandler::add_quote(&mut transaction, &guild_id, quote_data).await?;

    database::commit_and_say(
      PoiseContext::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Quote has been added.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
    )
    .await?;
  }

  Ok(())
}

/// Edit an existing quote
///
/// Edits an existing quote.
#[poise::command(slash_command)]
async fn edit(
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "ID of the quote to edit"]
  #[rename = "id"]
  quote_id: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(existing_quote) =
    DatabaseHandler::get_quote(&mut transaction, &guild_id, quote_id.as_str()).await?
  {
    let defaults = QuoteModal::from(existing_quote);

    if let Some(quote_data) = QuoteModal::execute_with_defaults(ctx, defaults).await? {
      let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

      DatabaseHandler::update_quote(&mut transaction, quote_data.into_quote(quote_id)).await?;

      database::commit_and_say(
        PoiseContext::Application(ctx),
        transaction,
        MessageType::TextOnly(format!("{} Quote has been edited.", EMOJI.mmcheck)),
        Visibility::Ephemeral,
      )
      .await?;
    }
  } else {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Invalid quote ID.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
  }

  Ok(())
}

/// Remove a quote from the database
///
/// Removes a quote from the database.
#[poise::command(slash_command)]
async fn remove(
  ctx: Context<'_>,
  #[description = "The quote ID to remove"]
  #[rename = "id"]
  quote_id: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if DatabaseHandler::quote_exists(&mut transaction, &guild_id, quote_id.as_str()).await? {
    DatabaseHandler::remove_quote(&mut transaction, &guild_id, quote_id.as_str()).await?;

    database::commit_and_say(
      ctx,
      transaction,
      MessageType::TextOnly(format!("{} Quote has been removed.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
    )
    .await?;
  } else {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Quote does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
  }

  Ok(())
}

/// List all quotes in the database
///
/// Lists all quotes in the database.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let quotes = DatabaseHandler::get_all_quotes(&mut transaction, &guild_id).await?;
  let quotes: Vec<PageRowRef> = quotes.iter().map(|quote| quote as PageRowRef).collect();

  drop(transaction);

  Paginator::new("Quotes", &quotes, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Search the quote database
///
/// Searches the quote database using one or more keywords in search engine format. Valid search operators include quotation marks (""), OR, and minus (-).
///
/// Example: "coming back" pema or chodron -thubten
#[poise::command(slash_command)]
async fn search(
  ctx: Context<'_>,
  #[description = "One or more keywords in search engine format"] keyword: String,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let quotes = DatabaseHandler::search_quotes(&mut transaction, &guild_id, &keyword).await?;

  if quotes.is_empty() {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} No quotes match your search query.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let quotes: Vec<PageRowRef> = quotes.iter().map(|quote| quote as PageRowRef).collect();

  drop(transaction);

  Paginator::new("Quotes", &quotes, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Show a quote
///
/// Shows a specific quote using the quote ID.
#[poise::command(slash_command)]
async fn show(
  ctx: Context<'_>,
  #[description = "ID of the quote to show"]
  #[rename = "id"]
  quote_id: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  match DatabaseHandler::get_quote(&mut transaction, &guild_id, quote_id.as_str()).await? {
    None => {
      ctx
        .send(
          CreateReply::default()
            .content(format!("{} Invalid quote ID.", EMOJI.mminfo))
            .ephemeral(true),
        )
        .await?;
    }
    Some(quote) => {
      let embed = BloomBotEmbed::new().description(format!(
        "{}\n\n\\― {}",
        quote.quote,
        quote.author.unwrap_or("Anonymous".to_string())
      ));

      ctx.send(CreateReply::default().embed(embed)).await?;
    }
  }

  Ok(())
}
