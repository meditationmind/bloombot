use crate::Context;
use anyhow::Result;

/// Say hello to Bloom!
///
/// Say hello to Bloom.
///
/// Don't worry - Bloom is friendly :)
#[poise::command(slash_command, category = "Utilities")]
pub async fn hello(ctx: Context<'_>) -> Result<()> {
  ctx.say("Hello, friend!").await?;

  Ok(())
}
