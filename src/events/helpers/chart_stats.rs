use std::{sync::Arc, time::Duration};

use crate::commands::helpers::time::Timeframe;
use crate::database::DatabaseHandler;
use anyhow::Result;
use log::{error, info};
use tokio::time::sleep;

async fn refresh(db: &DatabaseHandler) -> Result<()> {
  let mut transaction = db.start_transaction().await?;
  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Weekly).await?;
  tokio::time::sleep(std::time::Duration::from_secs(60 * 2)).await;
  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Monthly).await?;
  tokio::time::sleep(std::time::Duration::from_secs(60 * 2)).await;
  DatabaseHandler::refresh_chart_stats(&mut transaction, &Timeframe::Yearly).await?;
  transaction.commit().await?;

  Ok(())
}

pub async fn update(source: &str, task_conn: Arc<DatabaseHandler>) {
  let mut interval = tokio::time::interval(Duration::from_secs(60 * 60 * 12));
  let wait = {
    let now = chrono::Utc::now();
    let midnight = now
      .date_naive()
      .and_hms_opt(0, 0, 0)
      .unwrap_or_else(|| now.naive_utc())
      .and_utc()
      + chrono::Duration::hours(24);
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
      (chrono::Utc::now() + chrono::Duration::seconds(wait)).format("%H:%M %P")
    );
  }

  sleep(Duration::from_secs(wait.unsigned_abs())).await;

  loop {
    interval.tick().await;

    info!(target: source, "Chart stats: Refreshing views");
    let refresh_start = std::time::Instant::now();
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
