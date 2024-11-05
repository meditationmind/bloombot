use anyhow::Result;
use poise::serenity_prelude::{Context, Reaction};

use crate::database::DatabaseHandler;
use crate::events::helpers::starboard;

pub async fn reaction_remove(
  ctx: &Context,
  database: &DatabaseHandler,
  remove_reaction: &Reaction,
) -> Result<()> {
  starboard::remove_star(ctx, database, remove_reaction).await?;

  Ok(())
}
