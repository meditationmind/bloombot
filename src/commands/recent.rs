use crate::config::ENTRIES_PER_PAGE;
use crate::database::DatabaseHandler;
use crate::pagination::{PageRowRef, PageType, Pagination};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::CreateReply;

/// See your recent meditation entries
///
/// Displays a list of your recent meditation entries.
///
/// Use this command to retrieve the ID used to remove an entry.
#[poise::command(slash_command, category = "Meditation Tracking", guild_only)]
pub async fn recent(
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

  let entries =
    DatabaseHandler::get_user_meditation_entries(&mut transaction, &guild_id, &ctx.author().id)
      .await?;
  drop(transaction);
  let entries: Vec<PageRowRef> = entries.iter().map(|entry| entry as _).collect();
  let pagination = Pagination::new("Meditation Entries", entries, ENTRIES_PER_PAGE).await?;

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
          CreateInteractionResponseMessage::new().embed(pagination.create_page_embed(current_page, PageType::Standard)),
        ),
      )
      .await?;
  }

  Ok(())
}
