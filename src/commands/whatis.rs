use std::cmp::Ordering;
use std::fmt::Write;

use anyhow::{Context as AnyhowContext, Result};
use poise::CreateReply;
use poise::serenity_prelude::CreateEmbedFooter;

use crate::Context;
use crate::commands::helpers::terms;
use crate::config::BloomBotEmbed;
use crate::data::term::Term;
use crate::database::DatabaseHandler;

/// See information about a term
///
/// Shows information about a term.
#[poise::command(slash_command, category = "Informational", guild_only)]
pub async fn whatis(
  ctx: Context<'_>,
  #[description = "The term to show information about"]
  #[autocomplete = "terms::autocomplete"]
  term: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let Some(term_info) =
    DatabaseHandler::get_term(&mut transaction, &guild_id, term.as_str()).await?
  else {
    let possible_terms =
      DatabaseHandler::get_possible_terms(&mut transaction, &guild_id, term.as_str(), 0.7).await?;
    let reply = term_not_found(&term, possible_terms.as_slice())?;
    ctx.send(reply).await?;
    return Ok(());
  };

  let embed = BloomBotEmbed::new()
    .title(term_info.name)
    .description(one_liner(term_info.meaning.as_str()));

  ctx.send(CreateReply::default().embed(embed)).await?;

  Ok(())
}

fn one_liner(term_meaning: &str) -> String {
  term_meaning
    .split_once('\n')
    .map_or(term_meaning.to_string(), |one_liner| {
      format!(
        "{}\n\n*Use </glossary info:1135659962308243479> for more information.*",
        one_liner.0
      )
    })
}

fn term_not_found(term: &str, possible_terms: &[Term]) -> Result<CreateReply> {
  match possible_terms.len().cmp(&1) {
    Ordering::Less => {
      let embed = BloomBotEmbed::new()
        .title("Term not found")
        .description(format!(
          "The term `{term}` was not found in the glossary. If you believe it should be included, use </glossary suggest:1135659962308243479> to suggest it for addition."
        ));
      Ok(CreateReply::default().embed(embed).ephemeral(true))
    }
    Ordering::Equal => {
      let possible_term = possible_terms
        .first()
        .with_context(|| "Failed to retrieve first element of possible_terms")?;
      let embed = BloomBotEmbed::new()
        .title(&possible_term.name)
        .description(one_liner(possible_term.meaning.as_str()))
        .footer(CreateEmbedFooter::new(format!(
          "*You searched for '{}'. The closest term available was '{}'.",
          term, possible_term.name,
        )));
      Ok(CreateReply::default().embed(embed))
    }
    Ordering::Greater => {
      let suggestions =
        possible_terms
          .iter()
          .take(5)
          .fold(String::new(), |mut suggestions, term| {
            let _ = writeln!(suggestions, "`{}`", term.name);
            suggestions
          });
      let embed = BloomBotEmbed::new()
        .title("Term not found")
        .description(format!("The term `{term}` was not found in the glossary."))
        .field(
          "Did you mean one of these?",
          format!(
            "{suggestions}\n*Try using </glossary search:1135659962308243479> to take advantage of a more powerful search, or use </glossary suggest:1135659962308243479> to suggest the term for addition to the glossary.*"
          ),
          false,
        );
      Ok(CreateReply::default().embed(embed).ephemeral(true))
    }
  }
}
