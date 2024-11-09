use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use log::{error, info};
use sqlx::{Connection, PgConnection};
use tokio::time;

use crate::commands::helpers::time::Timeframe;
use crate::database::DatabaseHandler;

/// Refreshes materialized views used to query stats for creating [`stats`][stats] charts.
/// Since this is an intensive process, the function sleeps for two minutes between refreshing
/// the materialized view for each [`Timeframe`] to keep resource usage low.
///
/// [stats]: crate::commands::stats::stats
async fn refresh(db: &DatabaseHandler) -> Result<()> {
  let mut transaction = db.start_transaction().await?;
  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Weekly).await?;
  time::sleep(Duration::from_secs(60 * 2)).await;

  let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
    transaction
  } else {
    info!(target: "bloombot::database","Connection closed. Reconnecting.");
    db.start_transaction().await?
  };

  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Monthly).await?;
  time::sleep(Duration::from_secs(60 * 2)).await;

  let mut transaction = if PgConnection::ping(&mut *transaction).await.is_ok() {
    transaction
  } else {
    info!(target: "bloombot::database","Connection closed. Reconnecting.");
    db.start_transaction().await?
  };

  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Yearly).await?;
  transaction.commit().await?;

  Ok(())
}

/// Orchestrates timing for calling [`refresh`] to refresh materialized views used for
/// charts stats. Time from call until noon or midnight, whichever is closer, is calculated
/// and a [`tokio::task`] is spawned and put to sleep for that duration, after which the
/// [`refresh`] is called and then repeated in 12-hour intervals.
///
/// Logging includes time until initial [`refresh`], as well as notification upon initiation,
/// and upon completion with time elapsed. The source argument can be used to customize the
/// target in the logs. For default behavior, use the [`module_path!`] macro.
pub async fn update(source: &str, task_conn: Arc<DatabaseHandler>) {
  let mut interval = time::interval(Duration::from_secs(60 * 60 * 12));
  let wait = {
    let now = Utc::now();
    let midnight = now
      .date_naive()
      .and_hms_opt(0, 0, 0)
      .unwrap_or_else(|| now.naive_utc())
      .and_utc()
      + ChronoDuration::hours(24);
    let noon = now
      .date_naive()
      .and_hms_opt(12, 0, 0)
      .unwrap_or_else(|| now.naive_utc())
      .and_utc();
    if noon > now {
      (noon - now).num_seconds()
    } else {
      (midnight - now).num_seconds()
    }
  };

  if wait > 0 {
    info!(
      target: source,
      "Chart stats: Next refresh in {}m ({})",
      wait / 60,
      (Utc::now() + ChronoDuration::seconds(wait)).format("%H:%M %P")
    );
  }

  time::sleep(Duration::from_secs(wait.unsigned_abs())).await;

  loop {
    interval.tick().await;

    info!(target: source, "Chart stats: Refreshing views");
    let refresh_start = Instant::now();
    if let Err(err) = refresh(&task_conn).await {
      error!(target: source, "Chart stats: Error refreshing views: {:?}", err);
    }
    info!(
      target: source,
      "Chart stats: Refresh completed in {:#?}",
      refresh_start
        .elapsed()
        .saturating_sub(Duration::from_secs(60 * 4))
    );
  }
}
