use crate::Context;
use anyhow::Result;

/// Check Bloom's uptime
///
/// Check Bloom's uptime.
///
/// See how long Bloom has been running.
#[poise::command(slash_command, category = "Utilities")]
pub async fn uptime(ctx: Context<'_>) -> Result<()> {
  let uptime = ctx.data().bloom_start_time.elapsed();

  let div_mod = |a, b| (a / b, a % b);

  let seconds = uptime.as_secs();
  let (minutes, seconds) = div_mod(seconds, 60);
  let (hours, minutes) = div_mod(minutes, 60);
  let (days, hours) = div_mod(hours, 24);

  ctx
    .say(format!("My current uptime is {days}d {hours}h {minutes}m {seconds}s."))
    .await?;

  Ok(())
}
