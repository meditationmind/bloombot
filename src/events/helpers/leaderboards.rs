use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use log::{error, info};
use poise::serenity_prelude::{GuildId, Http, UserId};
use sqlx::{Connection, PgConnection};

use crate::commands::helpers::time::Timeframe;
use crate::data::stats::LeaderboardUser;
use crate::database::DatabaseHandler;

/// Refreshes materialized views used to query stats for generating [`stats::leaderboard`][stats] charts.
///
/// [stats]: crate::commands::stats::stats
async fn refresh(db: &DatabaseHandler) -> Result<()> {
  let mut transaction = db.start_transaction().await?;
  DatabaseHandler::refresh_leaderboard(&mut transaction, &Timeframe::Daily).await?;

  let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
    transaction
  } else {
    info!(target: "bloombot::database","Connection closed. Reconnecting.");
    db.start_transaction().await?
  };

  DatabaseHandler::refresh_leaderboard(&mut transaction, &Timeframe::Weekly).await?;

  let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
    transaction
  } else {
    info!(target: "bloombot::database","Connection closed. Reconnecting.");
    db.start_transaction().await?
  };

  DatabaseHandler::refresh_leaderboard(&mut transaction, &Timeframe::Monthly).await?;

  let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
    transaction
  } else {
    info!(target: "bloombot::database","Connection closed. Reconnecting.");
    db.start_transaction().await?
  };

  DatabaseHandler::refresh_leaderboard(&mut transaction, &Timeframe::Yearly).await?;
  transaction.commit().await?;

  Ok(())
}

/// Processes [`LeaderboardUser`] data to prepare it for use in generating [`stats::leaderboard`][stats] charts.
///
/// [stats]: crate::commands::stats::stats
pub async fn process_stats(
  ctx: &Http,
  guild_id: &GuildId,
  stats: &Vec<LeaderboardUser>,
) -> Result<Option<Vec<Vec<String>>>> {
  let mut leaderboard_data: Vec<Vec<String>> = vec![vec![
    "Name".to_string(),
    "Minutes".to_string(),
    "Sessions".to_string(),
    "Streak".to_string(),
  ]];

  let mut rank = 1;
  for record in stats {
    if let Some(user_id) = &record.name {
      let user_nick_or_name = if record.anonymous_tracking.unwrap_or(false) {
        "Anonymous".to_string()
      } else {
        let user = UserId::new(user_id.parse::<u64>()?).to_user(&ctx).await?;
        let name = user
          .nick_in(&ctx, guild_id)
          .await
          .unwrap_or_else(|| user.global_name.unwrap_or(user.name));
        name
          .chars()
          .filter(|c| {
            c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace()
          })
          .collect()
      };
      leaderboard_data.push(vec![
        format!("{}. {}", rank, user_nick_or_name),
        record.minutes.unwrap_or(0).to_string(),
        record.sessions.unwrap_or(0).to_string(),
        if record.streaks_active.unwrap_or(true) && !record.streaks_private.unwrap_or(false) {
          record.streak.unwrap_or(0).to_string()
        } else {
          "N/A".to_string()
        },
      ]);
      rank += 1;
    }
  }

  if leaderboard_data.len() == 1 {
    return Ok(None);
  }

  Ok(Some(leaderboard_data))
}

/// Helps maintain up-to-date [`stats::leaderboard`][stats] charts by calling [`refresh`]
/// to refresh materialized views.
///
/// Logging includes notification upon initiation, and upon completion with time elapsed.
/// The source argument can be used to customize the target in the logs. For default behavior,
/// use the [`module_path!`] macro.
///
/// [stats]: crate::commands::stats::stats
pub async fn update(source: &str, task_conn: Arc<DatabaseHandler>) {
  info!(target: source, "Leaderboard: Refreshing views");
  let refresh_start = Instant::now();
  if let Err(err) = refresh(&task_conn).await {
    error!(target: source, "Leaderboard: Error refreshing views: {err:?}");
  }
  info!(
    target: source,
    "Leaderboard: Refresh completed in {:#?}",
    refresh_start.elapsed()
  );
}
