use std::sync::Arc;

use anyhow::Result;

use crate::database::DatabaseHandler;
use crate::events::helpers::chart_stats;

pub async fn guild_create(database: &Arc<DatabaseHandler>) -> Result<()> {
  tokio::spawn(chart_stats::update("bloombot", database.clone()));
  Ok(())
}
