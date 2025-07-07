use std::fmt::Write;
use std::time::{Duration, Instant};

use anyhow::{Context as AnyhowContext, Result};
use pgvector::Vector;
use poise::CreateReply;
use poise::serenity_prelude::ChannelId;
use poise::serenity_prelude::{ComponentInteractionCollector, CreateActionRow, CreateButton};
use poise::serenity_prelude::{CreateEmbed, CreateEmbedFooter, CreateInteractionResponse};
use poise::serenity_prelude::{CreateInteractionResponseMessage, CreateMessage, EditMessage};

use crate::Context;
use crate::commands::helpers::{common, terms};
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ENTRIES_PER_PAGE};
use crate::data::term::Term;
use crate::database::DatabaseHandler;

/// Glossary commands
///
/// Commands for interacting with the glossary.
///
/// Get `info` on a glossary entry, see a `list` of entries, `search` for a relevant entry, or `suggest` a term for addition.
#[poise::command(
  slash_command,
  category = "Informational",
  subcommands("list", "info", "search", "suggest"),
  subcommand_required,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn glossary(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// See a list of all glossary entries
///
/// Shows a list of all glossary entries.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let term_names = DatabaseHandler::get_term_list(&mut transaction, &guild_id).await?;
  let term_count = term_names.len();
  let mut sorted_terms = Vec::<(String, String)>::with_capacity(term_count);

  for term in term_names {
    let first_char = term.name.chars().next().unwrap_or_default().to_string();
    let mut full_term = term.name;
    if let Some(aliases) = term.aliases {
      if !aliases.is_empty() {
        full_term.push_str(" (");
        let alias_count = aliases.len();
        for (i, alias) in aliases.iter().enumerate() {
          full_term.push_str(alias);
          if i < (alias_count - 1) {
            full_term.push_str(", ");
          }
        }
        full_term.push(')');
      }
    }
    sorted_terms.push((first_char, full_term));
  }

  let terms_per_page = ENTRIES_PER_PAGE.glossary;
  let mut pages: Vec<Vec<(String, String)>> = vec![];
  while !sorted_terms.is_empty() {
    let mut page = vec![];
    for _i in 1..=terms_per_page {
      if sorted_terms.is_empty() {
        break;
      }
      if let Some(term) = sorted_terms.pop() {
        page.push(term);
      }
    }
    pages.push(page);
  }

  let mut letter: &str;
  let mut page_text: String;
  let mut all_pages = vec![];
  let mut total_pages = 0;

  let glossary_info = common::print_command(&ctx.data().commands, "glossary info");

  for page in pages {
    letter = &page[0].0;
    page_text = format!(
      "-# Terms in parentheses are aliases for the preceding term. Use {glossary_info} with any term or alias to read the full entry.\n\n-# {letter}\n"
    );
    for entry in &page {
      if entry.0 == letter {
        page_text.push_str(format!("- {}\n", entry.1).as_str());
      } else {
        page_text.push_str(format!("-# {}\n- {}\n", entry.0, entry.1).as_str());
        letter = &entry.0;
      }
    }
    page_text.push_str("** **\n\n");
    all_pages.push(page_text);
    total_pages += 1;
  }

  let ctx_id = ctx.id();
  let prev_button_id = format!("{ctx_id}prev");
  let next_button_id = format!("{ctx_id}next");

  let mut current_page = page.unwrap_or(0).saturating_sub(1);

  // Send the embed with the first page as content.
  let reply = {
    let footer = CreateEmbedFooter::new(format!(
      "Page {} of {total_pages}・Terms {}-{}・Total Terms: {term_count}",
      current_page + 1,
      current_page * terms_per_page + 1,
      if (term_count / ((current_page + 1) * terms_per_page)) > 0 {
        (current_page + 1) * terms_per_page
      } else {
        term_count
      },
    ));
    let embed = BloomBotEmbed::new()
      .title("List of Glossary Terms")
      .description(&all_pages[current_page])
      .footer(footer);
    let components = CreateActionRow::Buttons(vec![
      CreateButton::new(&prev_button_id).label("Previous"),
      CreateButton::new(&next_button_id).label("Next"),
    ]);
    CreateReply::default()
      .embed(embed)
      .components(vec![components])
  };
  let list_msg = ctx.send(reply).await?;

  // Loop through incoming interactions with the navigation buttons.
  while let Some(press) = ComponentInteractionCollector::new(ctx)
    .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
    // Timeout when no navigation button has been pressed for 10 minutes.
    .timeout(Duration::from_secs(60 * 10))
    .await
  {
    // Depending on which button was pressed, go to next or previous page.
    if press.data.custom_id == next_button_id {
      current_page += 1;
      if current_page >= all_pages.len() {
        current_page = 0;
      }
    } else if press.data.custom_id == prev_button_id {
      current_page = current_page.checked_sub(1).unwrap_or(all_pages.len() - 1);
    } else {
      // This is an unrelated button interaction.
      continue;
    }

    // Update the message with the new page contents.
    let footer = CreateEmbedFooter::new(format!(
      "Page {} of {total_pages}・Terms {}-{}・Total Terms: {term_count}",
      current_page + 1,
      current_page * terms_per_page + 1,
      if (term_count / ((current_page + 1) * terms_per_page)) > 0 {
        (current_page + 1) * terms_per_page
      } else {
        term_count
      },
    ));
    let msg = CreateInteractionResponseMessage::new().embed(
      BloomBotEmbed::new()
        .title("List of Glossary Terms")
        .description(&all_pages[current_page])
        .footer(footer),
    );
    press
      .create_response(
        ctx.serenity_context(),
        CreateInteractionResponse::UpdateMessage(msg),
      )
      .await?;
  }

  // Remove buttons after collector times out.
  list_msg
    .into_message()
    .await?
    .edit(ctx, EditMessage::new().components(vec![]))
    .await?;

  Ok(())
}

/// See information about a glossary entry
///
/// Shows information about a glossary entry.
#[poise::command(slash_command)]
async fn info(
  ctx: Context<'_>,
  #[description = "The term to show information about"]
  #[autocomplete = "terms::autocomplete"]
  term: String,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if let Some(term) = DatabaseHandler::get_term(&mut transaction, &guild_id, term.as_str()).await? {
    ctx
      .send(CreateReply::default().embed(term_embed(&term)))
      .await?;
    return Ok(());
  }

  let possible_terms =
    DatabaseHandler::get_possible_terms(&mut transaction, &guild_id, term.as_str(), 0.7).await?;

  let embed = if possible_terms.is_empty() {
    BloomBotEmbed::new()
      .title("Term not found")
      .description(format!("The term `{term}` was not found in the glossary."))
  } else if possible_terms.len() == 1 {
    let possible_term = possible_terms
      .first()
      .with_context(|| "Failed to retrieve first element of possible_terms")?;
    possible_term_embed(&term, possible_term)
  } else {
    let suggestions = format!(
      "{}\n*Try using {} to take advantage of a more powerful search.*",
      possible_terms
        .iter()
        .take(5)
        .fold(String::new(), |mut field, term| {
          let _ = writeln!(field, "`{}`", term.name);
          field
        }),
      common::print_command(&ctx.data().commands, "glossary search")
    );
    BloomBotEmbed::new()
      .title("Term not found")
      .description(format!("The term `{term}` was not found in the glossary."))
      .field("Did you mean one of these?", suggestions, false)
  };

  ctx.send(CreateReply::default().embed(embed)).await?;

  Ok(())
}

/// Search glossary entries using keywords or phrases
///
/// Searches glossary entries using keywords or phrases, leveraging AI to find the closest matches.
#[poise::command(slash_command)]
async fn search(
  ctx: Context<'_>,
  #[description = "The term to search for"] search: String,
) -> Result<()> {
  ctx.defer().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let start_time = Instant::now();
  let data = ctx.data();
  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let vector = Vector::from(
    data
      .embeddings
      .create_embedding(&search, ctx.author().id)
      .await?,
  );
  let possible_terms =
    DatabaseHandler::search_terms_by_vector(&mut transaction, &guild_id, &vector, 3).await?;
  let search_time = start_time.elapsed();

  let mut terms_returned = 0;
  let mut embed = BloomBotEmbed::new().title(format!("Search results for `{search}`"));

  if possible_terms.is_empty() {
    embed =
      embed.description("No terms were found. Try browsing the glossary with `/glossary list`.");
  } else {
    for (index, possible_term) in possible_terms.iter().enumerate() {
      let similarity_threshold = 0.3;
      if possible_term.distance_score.unwrap_or(1.0) > similarity_threshold {
        continue;
      }
      let relevance_description = possible_term.distance_score.map_or("Unknown", |score| {
        // Adjust score for cosine similarity.
        let similarity_score = (1.0 - score) * 100.0;
        match similarity_score.round() {
          100.0..=f64::MAX => "Exact match",
          90.0..=99.0 => "High",
          80.0..=89.0 => "Medium",
          70.0..=79.0 => "Low",
          _ => "Unknown",
        }
      });

      // Maximum length is 979: 1024 (embed field max) - 45 (relevance message).
      let meaning = if possible_term.meaning.len() > 979 {
        &format!(
          "{}...",
          // Truncate to 976: 979 (maximum length) - 3 (ellipsis).
          possible_term.meaning.chars().take(976).collect::<String>()
        )
      } else {
        &possible_term.meaning
      };

      embed = embed.field(
        format!("Term {}: `{}`", index + 1, &possible_term.term_name),
        format!("{meaning}\n```Estimated relevance: {relevance_description}```\n** **"),
        false,
      );

      terms_returned += 1;
    }
  }

  embed = embed.footer(CreateEmbedFooter::new(format!(
    "Search took {}ms",
    search_time.as_millis()
  )));

  if terms_returned == 0 {
    embed =
      embed.description("No terms were found. Try browsing the glossary with `/glossary list`.");
  }

  ctx.send(CreateReply::default().embed(embed)).await?;

  Ok(())
}

/// Suggest a term for the glossary
///
/// Suggest a term for addition to the glossary.
#[poise::command(slash_command)]
async fn suggest(
  ctx: Context<'_>,
  #[description = "Term you wish to suggest"] suggestion: String,
) -> Result<()> {
  let log_embed = BloomBotEmbed::new()
    .title("Term Suggestion")
    .description(format!("**Suggestion**: {suggestion}"))
    .footer(
      CreateEmbedFooter::new(format!(
        "Suggested by {} ({})",
        ctx.author().name,
        ctx.author().id
      ))
      .icon_url(ctx.author().avatar_url().unwrap_or_default()),
    );

  let log_channel = ChannelId::new(CHANNELS.bloomlogs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  let msg = format!(
    "{} Your suggestion has been submitted. Thank you!",
    EMOJI.mmcheck
  );
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}

fn term_embed(term: &Term) -> CreateEmbed {
  let mut embed = BloomBotEmbed::new()
    .title(&term.name)
    .description(&term.meaning);
  if let Some(usage) = &term.usage {
    embed = embed.field("Example of Usage:", usage, false);
  }
  if let Some(links) = &term.links {
    if !links.is_empty() {
      embed = embed.field(
        "Related Resources:",
        links
          .iter()
          .enumerate()
          .fold(String::new(), |mut field, (count, link)| {
            let _ = writeln!(field, "{count}. {link}");
            field
          }),
        false,
      );
    }
  }
  if let Some(aliases) = &term.aliases {
    if !aliases.is_empty() {
      embed = embed.field(
        "Aliases:",
        {
          let alias_count = aliases.len();
          aliases
            .iter()
            .enumerate()
            .fold(String::new(), |mut field, (i, alias)| {
              let _ = write!(field, "{alias}");
              if i < (alias_count - 1) {
                let _ = write!(field, ", ");
              }
              field
            })
        },
        false,
      );
    }
  }
  if let Some(category) = &term.category {
    embed = embed.footer(CreateEmbedFooter::new(format!("Categories: {category}")));
  }
  embed
}

fn possible_term_embed(term_name: &str, possible_term: &Term) -> CreateEmbed {
  let mut embed = term_embed(possible_term);
  if let Some(category) = &possible_term.category {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Categories: {}\n\n*You searched for '{}'. The closest term available was '{}'.",
      category, term_name, possible_term.name
    )));
  } else {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "*You searched for '{}'. The closest term available was '{}'.",
      term_name, possible_term.name
    )));
  }

  embed
}
