use std::sync::Arc;

use anyhow::{Context as AnyhowContext, Result, anyhow};
use pgvector::Vector;
use poise::serenity_prelude::GuildId;
use poise::{ApplicationContext, Context as PoiseContext, CreateReply, Modal};
use sqlx::{Postgres, Transaction};
use tracing::info;

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::terms;
use crate::config::EMOJI;
use crate::data::term::{Term, TermModal};
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};

/// Commands for managing glossary entries
///
/// Commands to add, remove, or edit glossary entries.
///
/// Requires `Manage Roles` permissions.
#[poise::command(
  slash_command,
  required_permissions = "MANAGE_ROLES",
  default_member_permissions = "MANAGE_ROLES",
  category = "Moderator Commands",
  subcommands("add", "remove", "edit", "update_embeddings"),
  subcommand_required,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn terms(_: PoiseContext<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// Add a new term to the glossary
///
/// Adds a new term to the glossary.
#[poise::command(slash_command)]
async fn add(
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to add"]
  #[rename = "term"]
  term_name: String,
) -> Result<()> {
  if let Some(term_data) = TermModal::execute(ctx).await? {
    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;

    let vector = Vector::from(
      ctx
        .data()
        .embeddings
        .create_embedding(
          format!("{term_name} {}", term_data.meaning).as_str(),
          ctx.author().id,
        )
        .await?,
    );

    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    let term = Term::from_modal(guild_id, term_name, term_data, Some(vector));

    if let Err(e) = DatabaseHandler::add_term(&mut transaction, &term).await {
      let msg = format!("{} Failed to add term. Please try again.", EMOJI.mmx);
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Err(anyhow!("Failed to add term: {e}"));
    }

    // Update the term name list used for autocomplete.
    if let Ok(term_list) = DatabaseHandler::get_term_list(&mut transaction, &guild_id).await {
      let term_names = Arc::clone(&ctx.data().term_names);
      tokio::spawn(terms::update_names(term_list, term_names));
    }

    database::commit_and_say(
      PoiseContext::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Term has been added.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
    )
    .await?;
  }

  Ok(())
}

/// Update an existing term in the glossary
///
/// Updates an existing term in the glossary.
#[poise::command(slash_command)]
async fn edit(
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to edit"]
  #[rename = "term"]
  term_name: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let Some(existing_term) =
    DatabaseHandler::get_term(&mut transaction, &guild_id, term_name.as_str()).await?
  else {
    term_not_found(
      PoiseContext::Application(ctx),
      &mut transaction,
      guild_id,
      term_name,
    )
    .await?;
    return Ok(());
  };

  let existing_meaning = existing_term.meaning.clone();
  let defaults = TermModal::from(existing_term);

  if let Some(term_data) = TermModal::execute_with_defaults(ctx, defaults).await? {
    let vector = if term_data.meaning == existing_meaning {
      None
    } else {
      Some(Vector::from(
        ctx
          .data()
          .embeddings
          .create_embedding(
            format!("{} {}", term_name, term_data.meaning).as_str(),
            ctx.author().id,
          )
          .await?,
      ))
    };

    let term = Term::from_modal(guild_id, term_name, term_data, vector);

    if let Err(e) = DatabaseHandler::update_term(&mut transaction, &term).await {
      let msg = format!("{} Failed to edit term. Please try again.", EMOJI.mmx);
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      return Err(anyhow!("Failed to edit term: {e}"));
    }

    database::commit_and_say(
      PoiseContext::Application(ctx),
      transaction,
      MessageType::TextOnly(format!("{} Term has been edited.", EMOJI.mmcheck)),
      Visibility::Ephemeral,
    )
    .await?;
  }

  Ok(())
}

/// Remove a term from the glossary
///
/// Removes a term from the glossary.
#[poise::command(slash_command)]
async fn remove(
  ctx: Context<'_>,
  #[description = "The term to remove"]
  #[rename = "term"]
  term_name: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::term_exists(&mut transaction, &guild_id, term_name.as_str()).await? {
    let msg = format!("{} Term does not exist.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  if let Err(e) =
    DatabaseHandler::remove_term(&mut transaction, &guild_id, term_name.as_str()).await
  {
    let msg = format!("{} Failed to remove term. Please try again.", EMOJI.mmx);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Err(anyhow!("Failed to remove term: {e}"));
  }

  // Update the term name list used for autocomplete.
  if let Ok(term_list) = DatabaseHandler::get_term_list(&mut transaction, &guild_id).await {
    let term_names = Arc::clone(&ctx.data().term_names);
    tokio::spawn(terms::update_names(term_list, term_names));
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Term has been removed.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Update all embeddings
///
/// Updates embeddings for all terms.
#[poise::command(slash_command)]
async fn update_embeddings(ctx: Context<'_>) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let terms = DatabaseHandler::get_term_list(&mut transaction, &guild_id).await?;

  for term in terms {
    let Some(existing_term) =
      DatabaseHandler::get_term_meaning(&mut transaction, &guild_id, term.name.as_str()).await?
    else {
      info!("Unable to retrieve term: {}", term.name);
      continue;
    };

    let vector = Some(Vector::from(
      ctx
        .data()
        .embeddings
        .create_embedding(
          format!("{} {}", term.name, existing_term.meaning).as_str(),
          ctx.author().id,
        )
        .await?,
    ));

    DatabaseHandler::update_term_embedding(
      &mut transaction,
      &guild_id,
      term.name.as_str(),
      vector.as_ref(),
    )
    .await?;
  }

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!(
      "{} Term embeddings have been updated.",
      EMOJI.mmcheck
    )),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Checks the database for terms with names that meet a similarity threshold of 0.8
/// (high similarity). If found, suggests the matching term(s) in order of similarity.
/// Otherwise, informs the user that the term does not exist.
async fn term_not_found(
  ctx: Context<'_>,
  transaction: &mut Transaction<'_, Postgres>,
  guild_id: GuildId,
  term_name: String,
) -> Result<()> {
  let possible_terms =
    DatabaseHandler::get_possible_terms(transaction, &guild_id, term_name.as_str(), 0.8).await?;

  if possible_terms.len() > 1 {
    let msg = format!(
      "{} Term does not exist. Did you mean one of these?\n{}",
      EMOJI.mminfo,
      possible_terms
        .iter()
        .map(|term| format!("`{}`", term.name))
        .collect::<Vec<String>>()
        .join("\n")
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
  } else if let Some(possible_term) = possible_terms.first() {
    let msg = format!(
      "{} Term does not exist. Did you mean `{}`?",
      EMOJI.mminfo, possible_term.name
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
  } else {
    let msg = format!("{} Term does not exist.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
  }

  Ok(())
}
