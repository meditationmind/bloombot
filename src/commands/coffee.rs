use std::sync::Arc;

use anyhow::Result;
use rand::Rng;

use crate::{config::EMOJI, Context};

const BABYCOMEBACK: u64 = 762_671_692_430_180_363;
const TEACUPS: u64 = 1_085_468_454_578_044_979;

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

  let (one, two) = match ctx.author().id.get() {
    BABYCOMEBACK => ("☕", &*EMOJI.derpman.to_string()),
    TEACUPS => ("🍵", "⚰️"),
    _ => ("☕", "⚰️"),
  };

  if rng.gen() {
    ctx.say(one).await?;
  } else {
    ctx.say(two).await?;
  }

  Ok(())
}
