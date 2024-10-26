use crate::database::DatabaseHandler;
use crate::events::helpers::starboard;
use anyhow::Result;
use poise::serenity_prelude::{Context, Reaction};

pub async fn reaction_remove(
  ctx: &Context,
  database: &DatabaseHandler,
  remove_reaction: &Reaction,
) -> Result<()> {
  starboard::remove_star(ctx, database, remove_reaction).await?;

  Ok(())
}
