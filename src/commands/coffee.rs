use std::sync::Arc;

use anyhow::Result;
use rand::Rng;

use crate::Context;

/// Are you feeling lucky?
///
/// Are you feeling lucky?
///
/// I will choose either ☕ or ⚰️.
#[poise::command(slash_command, category = "Utilities")]
pub async fn coffee(ctx: Context<'_>) -> Result<()> {
  let data = ctx.data();

  let rng = Arc::clone(&data.rng);
  let mut rng = rng.lock().await;

  if rng.gen() {
    ctx.say("☕").await?;
  } else {
    ctx.say("⚰️").await?;
  }

  Ok(())
}
