use crate::{
  commands::helpers::common::Visibility,
  commands::helpers::database::{self, MessageType},
  config::EMOJI,
  data::term::Term,
  database::DatabaseHandler,
  Context, Data as AppData, Error as AppError,
};
use anyhow::{Context as AnyhowContext, Result};
use log::info;
use poise::serenity_prelude as serenity;
use poise::Modal;

#[derive(Debug, Modal)]
#[name = "Add/Edit Term"]
pub struct TermModal {
  #[name = "The definition of the term"]
  #[placeholder = "The first paragraph should be a concise summary (used by /whatis)"]
  #[paragraph]
  #[max_length = 1000]
  pub meaning: String,
  #[name = "An example of the term in use"]
  pub usage: Option<String>,
  #[name = "The category of the term"]
  pub category: Option<String>,
  #[name = "Links to further reading, comma separated"]
  pub links: Option<String>,
  #[name = "Term aliases, comma separated"]
  pub aliases: Option<String>,
}

impl From<Term> for TermModal {
  /// Converts a [`Term`] into a [`TermModal`]. Note that the [`GuildId`][gid]
  /// and `name` fields will be lost in the conversion. To convert back to a [`Term`],
  /// use the [`Term::from_modal()`] method with the original [`GuildId`][gid] and `name`.
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  fn from(term: Term) -> Self {
    Self {
      meaning: term.meaning,
      usage: term.usage,
      category: term.category,
      links: term.links.map(|links| links.join(", ")),
      aliases: term.aliases.map(|aliases| aliases.join(", ")),
    }
  }
}

async fn term_not_found(
  ctx: Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: serenity::GuildId,
  term_name: String,
) -> Result<()> {
  let possible_terms =
    DatabaseHandler::get_possible_terms(transaction, &guild_id, term_name.as_str(), 0.8).await?;

  if possible_terms.len() > 1 {
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!(
            "{} Term does not exist. Did you mean one of these?\n{}",
            EMOJI.mminfo,
            possible_terms
              .iter()
              .map(|term| format!("`{}`", term.name))
              .collect::<Vec<String>>()
              .join("\n")
          ))
          .ephemeral(true),
      )
      .await?;
  } else if let Some(possible_term) = possible_terms.first() {
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!(
            "{} Term does not exist. Did you mean `{}`?",
            EMOJI.mminfo, possible_term.name
          ))
          .ephemeral(true),
      )
      .await?;
  } else {
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!("{} Term does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
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
  subcommands("add", "remove", "edit", "update_embeddings"),
  subcommand_required,
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
async fn add(
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to add"]
  #[rename = "term"]
  term_name: String,
) -> Result<()> {
  let term_data = TermModal::execute(ctx).await?;

  if let Some(term_data) = term_data {
    let guild_id = ctx
      .guild_id()
      .with_context(|| "Failed to retrieve guild ID from context")?;

    let vector = pgvector::Vector::from(
      ctx
        .data()
        .embeddings
        .create_embedding(
          format!("{term_name} {}", term_data.meaning),
          ctx.author().id,
        )
        .await?,
    );

    let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

    DatabaseHandler::add_term(
      &mut transaction,
      Term::from_modal(guild_id, term_name, term_data),
      vector,
    )
    .await?;

    database::commit_and_say(
      poise::Context::Application(ctx),
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
  ctx: poise::ApplicationContext<'_, AppData, AppError>,
  #[description = "The term to edit"]
  #[rename = "term"]
  term_name: String,
) -> Result<()> {
  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  if let Some(existing_term) =
    DatabaseHandler::get_term(&mut transaction, &guild_id, term_name.as_str()).await?
  {
    let existing_meaning = existing_term.meaning.clone();
    let defaults = TermModal::from(existing_term);

    let term_data = TermModal::execute_with_defaults(ctx, defaults).await?;

    if let Some(term_data) = term_data {
      let vector = if term_data.meaning == existing_meaning {
        None
      } else {
        Some(pgvector::Vector::from(
          ctx
            .data()
            .embeddings
            .create_embedding(
              format!("{} {}", term_name, term_data.meaning),
              ctx.author().id,
            )
            .await?,
        ))
      };

      DatabaseHandler::edit_term(
        &mut transaction,
        Term::from_modal(guild_id, term_name, term_data),
        vector,
      )
      .await?;

      database::commit_and_say(
        poise::Context::Application(ctx),
        transaction,
        MessageType::TextOnly(format!("{} Term has been edited.", EMOJI.mmcheck)),
        true,
      )
      .await?;
    }
  } else {
    term_not_found(
      poise::Context::Application(ctx),
      &mut transaction,
      guild_id,
      term_name,
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
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!("{} Term does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_term(&mut transaction, term_name.as_str(), &guild_id).await?;

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
    if let Some(existing_term) =
      DatabaseHandler::get_term_meaning(&mut transaction, &guild_id, term.term_name.as_str())
        .await?
    {
      let vector = Some(pgvector::Vector::from(
        ctx
          .data()
          .embeddings
          .create_embedding(
            format!("{} {}", term.term_name, existing_term.meaning),
            ctx.author().id,
          )
          .await?,
      ));

      DatabaseHandler::edit_term_embedding(
        &mut transaction,
        &guild_id,
        term.term_name.as_str(),
        vector,
      )
      .await?;
    }
    info!("Unable to retrieve term: {}", term.term_name);
    continue;
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
