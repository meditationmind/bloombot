use std::sync::Arc;

use crate::database::DatabaseHandler;
use crate::events::helpers::{chart_stats, leaderboards};
use anyhow::Result;
use poise::serenity_prelude::{Context, GuildId};

pub async fn guild_create(
  ctx: &Context,
  database: &Arc<DatabaseHandler>,
  guild_id: &GuildId,
) -> Result<()> {
  tokio::spawn(leaderboards::update(
    "bloombot",
    ctx.http.clone(),
    database.clone(),
    *guild_id,
  ));

  tokio::spawn(chart_stats::update("bloombot", database.clone()));
  Ok(())
}
