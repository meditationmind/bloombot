use crate::commands::{commit_and_say, MessageType};
use crate::database::DatabaseHandler;
use crate::{Context, Data as AppData, Error as AppError};
use anyhow::Result;
use pgvector;
use poise::serenity_prelude as serenity;
use poise::Modal;
use std::cmp::Ordering;

#[derive(Debug, Modal)]
#[name = "Add a new term"]
struct AddTermModal {
  // #[name = "The term to add"]
  // #[placeholder = "For acronyms, use the full name here"]
  // term: String,
  #[name = "The definition of the term"]
  #[placeholder = "Include the acronym at the beginning of your definition"]
  #[paragraph]
  #[max_length = 1000]
  definition: String,
  #[name = "An example sentence showing the term in use"]
  example: Option<String>,
  #[name = "The category of the term"]
  category: Option<String>,
  #[name = "Links to further reading, comma separated"]
  links: Option<String>,
  #[name = "Term aliases, comma separated"]
  aliases: Option<String>,
}

#[derive(Debug, Modal)]
#[name = "Edit this term"]
struct UpdateTermModal {
  #[name = "The definition of the term"]
  #[paragraph]
  #[max_length = 1000]
  definition: String,
  #[name = "An example sentence showing the term in use"]
  example: Option<String>,
  #[name = "The category of the term"]
  category: Option<String>,
  #[name = "Links to further reading, comma separated"]
  links: Option<String>,
  #[name = "Term aliases, comma separated"]
  aliases: Option<String>,
}

pub async fn term_not_found(
  ctx: Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: serenity::GuildId,
  term_name: String,
) -> Result<()> {
  let possible_terms =
    DatabaseHandler::get_possible_terms(transaction, &guild_id, term_name.as_str(), 0.8).await?;

  match possible_terms.len().cmp(&1) {
    Ordering::Less => {
      ctx
        .send(
          poise::CreateReply::default()
            .content(":x: Term does not exist.")
            .ephemeral(true),
        )
        .await?;
    }
    Ordering::Equal => {
      let possible_term = possible_terms
        .first()
        .expect("first element should be Some since we matched Equal for cmp(&1)");

      ctx
        .send(
          poise::CreateReply::default()
            .content(format!(
              ":x: Term does not exist. Did you mean `{}`?",
              possible_term.name
            ))
            .ephemeral(true),
        )
        .await?;
    }
    Ordering::Greater => {
      ctx
        .send(
          poise::CreateReply::default()
            .content(format!(
              ":x: Term does not exist. Did you mean one of these?\n{}",
              possible_terms
                .iter()
                .map(|term| format!("`{}`", term.name))
                .collect::<Vec<String>>()
                .join("\n")
            ))
            .ephemeral(true),
        )
        .await?;
    }
  }

  Ok(())
}

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
  subcommands("add", "remove", "edit"),
  subcommand_required,
  //hide_in_help,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn terms(_: poise::Context<'_, AppData, AppError>) -> Result<()> {
  Ok(())
}

/// Add a new term to the glossary
///
/// Adds a new term to the glossary.
#[poise::command(slash_command)]
pub async fn add(
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to add"] term_name: String,
) -> Result<()> {
  use poise::Modal as _;

  let term_data = AddTermModal::execute(ctx).await?;

  if let Some(term_data) = term_data {
    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    let guild_id = ctx
      .guild_id()
      .expect("GuildID should be available since command is guild_only");

    let links = match term_data.links {
      Some(links) => links.split(',').map(|s| s.trim().to_string()).collect(),
      None => Vec::new(),
    };

    let aliases = match term_data.aliases {
      Some(aliases) => aliases.split(',').map(|s| s.trim().to_string()).collect(),
      None => Vec::new(),
    };

    let vector = pgvector::Vector::from(
      ctx
        .data()
        .embeddings
        .create_embedding(term_name.clone(), ctx.author().id)
        .await?,
    );

    DatabaseHandler::add_term(
      &mut transaction,
      term_name.as_str(),
      term_data.definition.as_str(),
      term_data.example.as_deref(),
      links.as_slice(),
      term_data.category.as_deref(),
      aliases.as_slice(),
      &guild_id,
      vector,
    )
    .await?;

    commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(":white_check_mark: Term has been added.".to_string()),
      true,
    )
    .await?;
  } else {
    ctx
      .send(
        poise::CreateReply::default()
          .content(":x: No data was provided.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  Ok(())
}

/// Update an existing term in the glossary
///
/// Updates an existing term in the glossary.
#[poise::command(slash_command)]
pub async fn edit(
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to edit"] term_name: String,
) -> Result<()> {
  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .expect("GuildID should be available since command is guild_only");

  let existing_term =
    DatabaseHandler::get_term(&mut transaction, &guild_id, term_name.as_str()).await?;

  if existing_term.is_none() {
    term_not_found(
      poise::Context::Application(ctx),
      &mut transaction,
      guild_id,
      term_name,
    )
    .await?;
    return Ok(());
  }

  let existing_term =
    existing_term.expect("existing_term should be Some since we already checked for is_none");
  let links = existing_term.links.map(|links| links.join(", "));
  let aliases = existing_term.aliases.map(|aliases| aliases.join(", "));

  let existing_definition = existing_term.meaning.clone();

  let defaults = UpdateTermModal {
    definition: existing_term.meaning,
    example: existing_term.usage,
    category: existing_term.category,
    links,
    aliases,
  };

  let term_data = UpdateTermModal::execute_with_defaults(ctx, defaults).await?;

  if let Some(term_data) = term_data {
    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    let links = match term_data.links {
      Some(links) => links.split(',').map(|s| s.trim().to_string()).collect(),
      None => Vec::new(),
    };

    let vector = if term_data.definition == existing_definition {
      None
    } else {
      Some(pgvector::Vector::from(
        ctx
          .data()
          .embeddings
          .create_embedding(existing_term.name, ctx.author().id)
          .await?,
      ))
    };

    let aliases = match term_data.aliases {
      Some(aliases) => aliases.split(',').map(|s| s.trim().to_string()).collect(),
      None => Vec::new(),
    };

    DatabaseHandler::edit_term(
      &mut transaction,
      &existing_term.id,
      term_data.definition.as_str(),
      term_data.example.as_deref(),
      links.as_slice(),
      term_data.category.as_deref(),
      aliases.as_slice(),
      vector,
    )
    .await?;

    commit_and_say(
      poise::Context::Application(ctx),
      transaction,
      MessageType::TextOnly(":white_check_mark: Term has been edited.".to_string()),
      true,
    )
    .await?;
  } else {
    ctx
      .send(
        poise::CreateReply::default()
          .content(":x: No data was provided.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  Ok(())
}

/// Remove a term from the glossary
///
/// Removes a term from the glossary.
#[poise::command(slash_command)]
pub async fn remove(
  ctx: Context<'_>,
  #[description = "The term to remove"] term: String,
) -> Result<()> {
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .expect("GuildID should be available since command is guild_only");

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::term_exists(&mut transaction, &guild_id, term.as_str()).await? {
    ctx
      .send(
        poise::CreateReply::default()
          .content(":x: Term does not exist.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_term(&mut transaction, term.as_str(), &guild_id).await?;

  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(":white_check_mark: Term has been removed.".to_string()),
    true,
  )
  .await?;

  Ok(())
}
