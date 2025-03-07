use anyhow::{Context as AnyhowContext, Result};

use crate::Context;
use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::ENTRIES_PER_PAGE;
use crate::database::DatabaseHandler;

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
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let entries =
    DatabaseHandler::get_user_meditation_entries(&mut transaction, &guild_id, &ctx.author().id)
      .await?;
  let entries: Vec<PageRowRef> = entries.iter().map(|entry| entry as PageRowRef).collect();

  drop(transaction);

  Paginator::new("Meditation Entries", &entries, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}
