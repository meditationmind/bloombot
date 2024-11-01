use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator, Visibility};
use crate::config::{BloomBotEmbed, EMOJI, ENTRIES_PER_PAGE};
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};
use anyhow::{Context as AnyhowContext, Result};
use poise::{CreateReply, Modal};

#[derive(Debug, Modal)]
#[name = "Add a new quote"]
struct AddQuoteModal {
  #[name = "Quote text"]
  #[placeholder = "Input quote text here"]
  #[paragraph]
  #[max_length = 300]
  quote: String,
  #[name = "Author's name"]
  #[placeholder = "Defaults to \"Anonymous\""]
  author: Option<String>,
}

#[derive(Debug, Modal)]
#[name = "Edit a quote"]
struct EditQuoteModal {
  #[name = "Quote text"]
  #[paragraph]
  #[max_length = 300]
  quote: String,
  #[name = "Author's name"]
  author: Option<String>,
}

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
  //hide_in_help,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn quotes(_: poise::Context<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// Add a quote to the database
///
/// Adds a quote to the database.
#[poise::command(slash_command)]
async fn add(ctx: poise::ApplicationContext<'_, AppData, AppError>) -> Result<()> {
  use poise::Modal as _;

  let quote_data = AddQuoteModal::execute(ctx).await?;

  if let Some(quote_data) = quote_data {
    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;

    DatabaseHandler::add_quote(
      &mut transaction,
      &guild_id,
      quote_data.quote.as_str(),
      quote_data.author.as_deref(),
    )
    .await?;

    database::commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Quote has been added.", EMOJI.mmcheck)),
      true,
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
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "ID of the quote to edit"]
  #[rename = "id"]
  quote_id: String,
) -> Result<()> {
  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let existing_quote =
    DatabaseHandler::get_quote(&mut transaction, &guild_id, quote_id.as_str()).await?;

  if existing_quote.is_none() {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Invalid quote ID.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let existing_quote =
    existing_quote.with_context(|| "Failed to assign QuoteData to existing_quote")?;

  let defaults = EditQuoteModal {
    quote: existing_quote.quote,
    author: existing_quote.author,
  };

  let quote_data = EditQuoteModal::execute_with_defaults(ctx, defaults).await?;

  if let Some(quote_data) = quote_data {
    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    DatabaseHandler::edit_quote(
      &mut transaction,
      &existing_quote.id,
      quote_data.quote.as_str(),
      quote_data.author.as_deref(),
    )
    .await?;

    database::commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Quote has been edited.", EMOJI.mmcheck)),
      true,
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::quote_exists(&mut transaction, &guild_id, quote_id.as_str()).await? {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Quote does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_quote(&mut transaction, &guild_id, quote_id.as_str()).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Quote has been removed.", EMOJI.mmcheck)),
    true,
  )
  .await?;

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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

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
  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

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
      let embed = BloomBotEmbed::new()
        .description(format!(
          "{}\n\n\\― {}",
          quote.quote.as_str(),
          quote.author.unwrap_or("Anonymous".to_string())
        ))
        .clone();

      ctx.send(poise::CreateReply::default().embed(embed)).await?;
    }
  }

  Ok(())
}
