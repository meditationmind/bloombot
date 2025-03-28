use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use log::{error, info};
use poise::serenity_prelude::{GuildId, Http, UserId};
use sqlx::{Connection, PgConnection};
use tokio::time;

use crate::charts::{Chart, LeaderboardOptions};
use crate::commands::helpers::time::Timeframe;
use crate::commands::stats::{LeaderboardType, SortBy, Theme};
use crate::data::stats::LeaderboardUser;
use crate::database::DatabaseHandler;

#[allow(dead_code)]
pub struct Leaderboards<'a> {
  pub day_min_top5_dark: &'a str,
  pub day_min_top5_light: &'a str,
  pub day_min_top10_dark: &'a str,
  pub day_min_top10_light: &'a str,
  pub day_ses_top5_dark: &'a str,
  pub day_ses_top5_light: &'a str,
  pub day_ses_top10_dark: &'a str,
  pub day_ses_top10_light: &'a str,
  pub day_str_top5_dark: &'a str,
  pub day_str_top5_light: &'a str,
  pub day_str_top10_dark: &'a str,
  pub day_str_top10_light: &'a str,
  pub week_min_top5_dark: &'a str,
  pub week_min_top5_light: &'a str,
  pub week_min_top10_dark: &'a str,
  pub week_min_top10_light: &'a str,
  pub week_ses_top5_dark: &'a str,
  pub week_ses_top5_light: &'a str,
  pub week_ses_top10_dark: &'a str,
  pub week_ses_top10_light: &'a str,
  pub week_str_top5_dark: &'a str,
  pub week_str_top5_light: &'a str,
  pub week_str_top10_dark: &'a str,
  pub week_str_top10_light: &'a str,
  pub month_min_top5_dark: &'a str,
  pub month_min_top5_light: &'a str,
  pub month_min_top10_dark: &'a str,
  pub month_min_top10_light: &'a str,
  pub month_ses_top5_dark: &'a str,
  pub month_ses_top5_light: &'a str,
  pub month_ses_top10_dark: &'a str,
  pub month_ses_top10_light: &'a str,
  pub month_str_top5_dark: &'a str,
  pub month_str_top5_light: &'a str,
  pub month_str_top10_dark: &'a str,
  pub month_str_top10_light: &'a str,
  pub year_min_top5_dark: &'a str,
  pub year_min_top5_light: &'a str,
  pub year_min_top10_dark: &'a str,
  pub year_min_top10_light: &'a str,
  pub year_ses_top5_dark: &'a str,
  pub year_ses_top5_light: &'a str,
  pub year_ses_top10_dark: &'a str,
  pub year_ses_top10_light: &'a str,
  pub year_str_top5_dark: &'a str,
  pub year_str_top5_light: &'a str,
  pub year_str_top10_dark: &'a str,
  pub year_str_top10_light: &'a str,
}

/// Filename consists of prefix `leaderboard_` followed by:
/// - Timeframe as (`d`)aily, (`w`)eekly, (`m`)onthly, or (`y`)early
/// - Sorting stat as (`min`)utes, (`ses`)sions, or (`str`)eak
/// - Type as top (`5`) or top (`10`)
/// - Theme as (`d`)ark mode or (`l`)ight mode
pub const LEADERBOARDS: Leaderboards = Leaderboards {
  day_min_top5_dark: "leaderboard_dmin5d.webp",
  day_min_top5_light: "leaderboard_dmin5l.webp",
  day_min_top10_dark: "leaderboard_dmin10d.webp",
  day_min_top10_light: "leaderboard_dmin10l.webp",
  day_ses_top5_dark: "leaderboard_dses5d.webp",
  day_ses_top5_light: "leaderboard_dses5l.webp",
  day_ses_top10_dark: "leaderboard_dses10d.webp",
  day_ses_top10_light: "leaderboard_dses10l.webp",
  day_str_top5_dark: "leaderboard_dstr5d.webp",
  day_str_top5_light: "leaderboard_dstr5l.webp",
  day_str_top10_dark: "leaderboard_dstr10d.webp",
  day_str_top10_light: "leaderboard_dstr10l.webp",
  week_min_top5_dark: "leaderboard_wmin5d.webp",
  week_min_top5_light: "leaderboard_wmin5l.webp",
  week_min_top10_dark: "leaderboard_wmin10d.webp",
  week_min_top10_light: "leaderboard_wmin10l.webp",
  week_ses_top5_dark: "leaderboard_wses5d.webp",
  week_ses_top5_light: "leaderboard_wses10d.webp",
  week_ses_top10_dark: "leaderboard_wses5l.webp",
  week_ses_top10_light: "leaderboard_wses10l.webp",
  week_str_top5_dark: "leaderboard_wstr5d.webp",
  week_str_top5_light: "leaderboard_wstr5l.webp",
  week_str_top10_dark: "leaderboard_wstr10d.webp",
  week_str_top10_light: "leaderboard_wstr10l.webp",
  month_min_top5_dark: "leaderboard_mmin5d.webp",
  month_min_top5_light: "leaderboard_mmin5l.webp",
  month_min_top10_dark: "leaderboard_mmin10d.webp",
  month_min_top10_light: "leaderboard_mmin10l.webp",
  month_ses_top5_dark: "leaderboard_mses5d.webp",
  month_ses_top5_light: "leaderboard_mses5l.webp",
  month_ses_top10_dark: "leaderboard_mses10d.webp",
  month_ses_top10_light: "leaderboard_mses10l.webp",
  month_str_top5_dark: "leaderboard_mstr5d.webp",
  month_str_top5_light: "leaderboard_mstr5l.webp",
  month_str_top10_dark: "leaderboard_mstr10d.webp",
  month_str_top10_light: "leaderboard_mstr10l.webp",
  year_min_top5_dark: "leaderboard_ymin5d.webp",
  year_min_top5_light: "leaderboard_ymin5l.webp",
  year_min_top10_dark: "leaderboard_ymin10d.webp",
  year_min_top10_light: "leaderboard_ymin10l.webp",
  year_ses_top5_dark: "leaderboard_yses5d.webp",
  year_ses_top5_light: "leaderboard_yses5l.webp",
  year_ses_top10_dark: "leaderboard_yses10d.webp",
  year_ses_top10_light: "leaderboard_yses10l.webp",
  year_str_top5_dark: "leaderboard_ystr5d.webp",
  year_str_top5_light: "leaderboard_ystr5l.webp",
  year_str_top10_dark: "leaderboard_ystr10d.webp",
  year_str_top10_light: "leaderboard_ystr10l.webp",
};

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

/// Generates [`stats::leaderboard`][stats] chart images in all available dark mode varieties
/// for quickly serving images to users. Light mode varieties are not pre-generated since very
/// few users prefer light mode. Because so many images are being generated at once, the function
/// sleeps 5 seconds between each image to keep resource usage low.
///
/// [stats]: crate::commands::stats::stats
async fn generate(http: &Http, db: &DatabaseHandler, guild_id: &GuildId) -> Result<()> {
  let mut transaction = db.start_transaction_with_retry(5).await?;

  let daily_minutes = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Daily,
    &SortBy::Minutes,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(daily_minutes) = process_stats(http, guild_id, &daily_minutes).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Minutes,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_min_top5_dark).await?;
    let _ = chart.leaderboard(daily_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Minutes,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_min_top10_dark).await?;
    let _ = chart.leaderboard(daily_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let weekly_minutes = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Weekly,
    &SortBy::Minutes,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(weekly_minutes) = process_stats(http, guild_id, &weekly_minutes).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Minutes,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_min_top5_dark).await?;
    let _ = chart.leaderboard(weekly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Minutes,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_min_top10_dark).await?;
    let _ = chart.leaderboard(weekly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let monthly_minutes = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Monthly,
    &SortBy::Minutes,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(monthly_minutes) = process_stats(http, guild_id, &monthly_minutes).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Minutes,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_min_top5_dark).await?;
    let _ = chart.leaderboard(monthly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Minutes,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_min_top10_dark).await?;
    let _ = chart.leaderboard(monthly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let yearly_minutes = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Yearly,
    &SortBy::Minutes,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(yearly_minutes) = process_stats(http, guild_id, &yearly_minutes).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Minutes,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_min_top5_dark).await?;
    let _ = chart.leaderboard(yearly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Minutes,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_min_top10_dark).await?;
    let _ = chart.leaderboard(yearly_minutes.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let daily_sessions = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Daily,
    &SortBy::Sessions,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(daily_sessions) = process_stats(http, guild_id, &daily_sessions).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Sessions,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_ses_top5_dark).await?;
    let _ = chart.leaderboard(daily_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Sessions,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_ses_top10_dark).await?;
    let _ = chart.leaderboard(daily_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let weekly_sessions = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Weekly,
    &SortBy::Sessions,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(weekly_sessions) = process_stats(http, guild_id, &weekly_sessions).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Sessions,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_ses_top5_dark).await?;
    let _ = chart.leaderboard(weekly_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Sessions,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_ses_top10_dark).await?;
    let _ = chart.leaderboard(weekly_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let monthly_sessions = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Monthly,
    &SortBy::Sessions,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(monthly_sessions) = process_stats(http, guild_id, &monthly_sessions).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Sessions,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_ses_top5_dark).await?;
    let _ = chart
      .leaderboard(monthly_sessions.clone(), &options)
      .await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Sessions,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_ses_top10_dark).await?;
    let _ = chart
      .leaderboard(monthly_sessions.clone(), &options)
      .await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let yearly_sessions = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Yearly,
    &SortBy::Sessions,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(yearly_sessions) = process_stats(http, guild_id, &yearly_sessions).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Sessions,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_ses_top5_dark).await?;
    let _ = chart.leaderboard(yearly_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Sessions,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_ses_top10_dark).await?;
    let _ = chart.leaderboard(yearly_sessions.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let daily_streaks = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Daily,
    &SortBy::Streak,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(daily_streaks) = process_stats(http, guild_id, &daily_streaks).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Streak,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_str_top5_dark).await?;
    let _ = chart.leaderboard(daily_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Daily,
      SortBy::Streak,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.day_str_top10_dark).await?;
    let _ = chart.leaderboard(daily_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let weekly_streaks = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Weekly,
    &SortBy::Streak,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(weekly_streaks) = process_stats(http, guild_id, &weekly_streaks).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Streak,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_str_top5_dark).await?;
    let _ = chart.leaderboard(weekly_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Weekly,
      SortBy::Streak,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.week_str_top10_dark).await?;
    let _ = chart.leaderboard(weekly_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let monthly_streaks = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Monthly,
    &SortBy::Streak,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(monthly_streaks) = process_stats(http, guild_id, &monthly_streaks).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Streak,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_str_top5_dark).await?;
    let _ = chart.leaderboard(monthly_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Monthly,
      SortBy::Streak,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.month_str_top10_dark).await?;
    let _ = chart.leaderboard(monthly_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;
  }

  let yearly_streaks = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    guild_id,
    &Timeframe::Yearly,
    &SortBy::Streak,
    &LeaderboardType::Top10,
  )
  .await?;

  if let Some(yearly_streaks) = process_stats(http, guild_id, &yearly_streaks).await? {
    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Streak,
      LeaderboardType::Top5,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_str_top5_dark).await?;
    let _ = chart.leaderboard(yearly_streaks.clone(), &options).await?;

    time::sleep(Duration::from_secs(5)).await;

    let options = LeaderboardOptions::new(
      Timeframe::Yearly,
      SortBy::Streak,
      LeaderboardType::Top10,
      Theme::DarkMode,
    );
    let chart = Chart::new_with_name(LEADERBOARDS.year_str_top10_dark).await?;
    let _ = chart.leaderboard(yearly_streaks.clone(), &options).await?;
  }

  Ok(())
}

/// Helps maintain up-to-date [`stats::leaderboard`][stats] charts by calling [`refresh`]
/// to refresh materialized views and [`generate`] to pre-generate images used for the charts.
/// Sleeps 10 seconds between [`refresh`] and [`generate`] to ensure that images are generated
/// using the latest stats.
///
/// Logging includes notification upon initiation, and upon completion with time elapsed
/// for each task. The source argument can be used to customize the target in the logs. For
/// default behavior, use the [`module_path!`] macro.
///
/// [stats]: crate::commands::stats::stats
pub async fn update(
  source: &str,
  task_http: Arc<Http>,
  task_conn: Arc<DatabaseHandler>,
  guild_id: GuildId,
) {
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

  time::sleep(Duration::from_secs(10)).await;

  info!(target: source, "Leaderboard: Generating images");
  let generation_start = Instant::now();
  if let Err(err) = generate(&task_http, &task_conn, &guild_id).await {
    error!(target: source, "Leaderboard: Error generating images: {err:?}");
  }
  info!(
    target: source,
    "Leaderboard: Generation completed in {:#?}",
    generation_start
      .elapsed()
      .saturating_sub(Duration::from_secs(115))
  );
}
