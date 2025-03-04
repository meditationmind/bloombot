#![allow(clippy::unused_async)]

use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{Colour, User, builder::*};
use poise::{ChoiceParameter, CreateReply};
use tracing::info;

use crate::Context;
use crate::charts::{Chart, LeaderboardOptions, StatsOptions};
use crate::commands::helpers::time::Timeframe;
use crate::config::{BloomBotEmbed, EMOJI, ROLES};
use crate::data::tracking_profile::{Privacy, Status, privacy};
use crate::database::DatabaseHandler;
use crate::events::leaderboards::{self, LEADERBOARDS};

#[allow(clippy::module_name_repetitions)]
#[derive(ChoiceParameter)]
pub enum StatsType {
  #[name = "minutes"]
  MeditationMinutes,
  #[name = "count"]
  MeditationCount,
}

#[derive(ChoiceParameter)]
pub enum ChartStyle {
  #[name = "bar chart"]
  Bar,
  #[name = "area chart"]
  Area,
  #[name = "bar chart (combined data)"]
  BarCombined,
}

#[derive(ChoiceParameter)]
pub enum SortBy {
  #[name = "minutes"]
  Minutes,
  #[name = "sessions"]
  Sessions,
  #[name = "streak"]
  Streak,
}

#[derive(ChoiceParameter)]
pub enum LeaderboardType {
  #[name = "Top 5"]
  Top5,
  #[name = "Top 10"]
  Top10,
}

#[derive(ChoiceParameter)]
pub enum Theme {
  #[name = "light mode"]
  LightMode,
  #[name = "dark mode"]
  DarkMode,
}

/// Show stats for a user or the server
///
/// Shows stats for yourself, a specified user, or the whole server.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  subcommands("user", "server", "leaderboard"),
  subcommand_required,
  guild_only
)]
pub async fn stats(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Show stats for a user
///
/// Shows stats for yourself or a specified user.
///
/// Defaults to daily minutes for yourself. Optionally specify the user, type (minutes or session count), and/or timeframe (daily, weekly, monthly, or yearly).
#[poise::command(slash_command)]
async fn user(
  ctx: Context<'_>,
  #[description = "User to get stats for (Defaults to you)"] user: Option<User>,
  #[description = "Type of stats to get (Defaults to minutes)"]
  #[rename = "type"]
  stats_type: Option<StatsType>,
  #[description = "Timeframe to get stats for (Defaults to daily)"] timeframe: Option<Timeframe>,
  #[description = "Style of chart (Defaults to bar chart)"] style: Option<ChartStyle>,
  #[description = "Visibility of the response (Defaults to public)"] privacy: Option<Privacy>,
  #[description = "Toggle between light/dark mode (Defaults to dark mode)"] theme: Option<Theme>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let user = user.as_ref().unwrap_or_else(|| ctx.author());
  let user_nick_or_name = user.nick_in(&ctx, guild_id).await.unwrap_or_else(|| {
    user
      .global_name
      .as_deref()
      .unwrap_or(user.name.as_str())
      .to_string()
  });

  let tracking_profile =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user.id)
      .await?
      .unwrap_or_default();

  let privacy = privacy!(privacy, tracking_profile.stats.privacy);

  if privacy {
    ctx.defer_ephemeral().await?;
  } else {
    ctx.defer().await?;
  }

  if ctx.author().id != user.id
    && tracking_profile.stats.privacy == Privacy::Private
    && !ctx.author().has_role(&ctx, guild_id, ROLES.staff).await?
  {
    let msg = format!("Sorry, {user_nick_or_name}'s stats are set to private.");
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let offset = tracking_profile.utc_offset;
  let theme = theme.unwrap_or(Theme::DarkMode);
  let chart_style = style.unwrap_or(ChartStyle::Bar);
  let stats_type = stats_type.unwrap_or(StatsType::MeditationMinutes);
  let timeframe = timeframe.unwrap_or(Timeframe::Daily);
  let timeframe_header = match timeframe {
    Timeframe::Yearly => "Years",
    Timeframe::Monthly => "Months",
    Timeframe::Weekly => "Weeks",
    Timeframe::Daily => "Days",
  };

  // Role-based bar color for donators; default otherwise.
  let donator = user.has_role(&ctx, guild_id, ROLES.patreon).await?
    || user.has_role(&ctx, guild_id, ROLES.kofi).await?;
  let bar_color = if !donator {
    StatsOptions::default().bar_color
  } else if donator && user.id == ctx.author().id {
    if let Some(member) = ctx.author_member().await {
      let color = member
        .colour(ctx)
        .unwrap_or(Colour::from(StatsOptions::default().rgb()));
      (color.r(), color.g(), color.b(), 255)
    } else {
      StatsOptions::default().bar_color
    }
  } else {
    let color = guild_id
      .member(&ctx, user.id)
      .await?
      .colour(ctx)
      .unwrap_or(Colour::from(StatsOptions::default().rgb()));
    (color.r(), color.g(), color.b(), 255)
  };

  let stats = DatabaseHandler::get_user_stats(&mut transaction, &guild_id, &user.id).await?;

  let chart_stats = DatabaseHandler::get_user_chart_stats(
    &mut transaction,
    &guild_id,
    &user.id,
    &timeframe,
    offset,
  )
  .await?;

  let total_minutes = stats.sessions.sum.unwrap_or(0);
  let total_count = stats.sessions.count.unwrap_or(0);
  let timeframe_sum = chart_stats
    .iter()
    .fold(0, |total, session| total + session.sum.unwrap_or(0));
  let timeframe_count = chart_stats
    .iter()
    .fold(0, |total, session| total + session.count.unwrap_or(0));

  let mut embed = BloomBotEmbed::new()
    .title(format!("Stats for {user_nick_or_name}"))
    .author(CreateEmbedAuthor::new(format!("{user_nick_or_name}'s Stats")).icon_url(user.face()));

  match stats_type {
    StatsType::MeditationMinutes => {
      embed = embed
        .field(
          "All-Time Meditation Minutes",
          format!("```{total_minutes}```"),
          true,
        )
        .field(
          format!("Minutes The Past 12 {timeframe_header}"),
          format!("```{timeframe_sum}```"),
          true,
        );
    }
    StatsType::MeditationCount => {
      embed = embed
        .field(
          "All-Time Session Count",
          format!("```{total_count}```"),
          true,
        )
        .field(
          format!("Sessions The Past 12 {timeframe_header}"),
          format!("```{timeframe_count}```"),
          true,
        );
    }
  }

  let (average, label) = match stats_type {
    StatsType::MeditationMinutes => (timeframe_sum / 12, "minutes"),
    StatsType::MeditationCount => (timeframe_count / 12, "sessions"),
  };

  // Hide streak in footer if streaks disabled.
  if tracking_profile.streak.status == Status::Enabled
    // Hide streak in footer if streak set to private, unless own stats in ephemeral.
    && (tracking_profile.streak.privacy == Privacy::Public || (ctx.author().id == user.id && privacy))
  {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Avg. {} {}: {}・Current streak: {}",
      timeframe.name().to_lowercase(),
      label,
      average,
      stats.streak.current
    )));
  } else {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Average {} {}: {}",
      timeframe.name().to_lowercase(),
      label,
      average
    )));
  }

  let options = StatsOptions::new(timeframe, offset, stats_type, chart_style, bar_color, theme);
  let chart = Chart::new().await?;
  let chart = chart.stats(&chart_stats, &options).await?;

  embed = embed.image(chart.url());
  let attachment = CreateAttachment::path(chart.path()).await?;

  ctx
    .send(CreateReply::default().embed(embed).attachment(attachment))
    .await?;

  chart.remove().await?;

  Ok(())
}

/// Show stats for the server
///
/// Shows stats for the whole server.
///
/// Defaults to daily minutes. Optionally specify the type (minutes or session count) and/or timeframe (daily, weekly, monthly, or yearly).
#[poise::command(slash_command)]
async fn server(
  ctx: Context<'_>,
  #[description = "Type of stats to get (Defaults to minutes)"]
  #[rename = "type"]
  stats_type: Option<StatsType>,
  #[description = "Timeframe to get stats for (Defaults to daily)"] timeframe: Option<Timeframe>,
  #[description = "Style of chart (Defaults to bar chart)"] style: Option<ChartStyle>,
  #[description = "Toggle between light/dark mode (Defaults to dark mode)"] theme: Option<Theme>,
) -> Result<()> {
  ctx.defer().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let (guild_name, guild_icon) = {
    if let Some(guild) = guild_id.to_guild_cached(&ctx) {
      (guild.name.clone(), guild.icon_url().unwrap_or_default())
    } else {
      (
        "This Server".to_string(),
        "https://cdn.discordapp.com/embed/avatars/3.png".to_string(),
      )
    }
  };

  let chart_style = style.unwrap_or(ChartStyle::Bar);
  let stats_type = stats_type.unwrap_or(StatsType::MeditationMinutes);
  let timeframe = timeframe.unwrap_or(Timeframe::Daily);

  let timeframe_header = match timeframe {
    Timeframe::Yearly => "Years",
    Timeframe::Monthly => "Months",
    Timeframe::Weekly => "Weeks",
    Timeframe::Daily => "Days",
  };

  let bar_color = StatsOptions::default().bar_color;
  let theme = theme.unwrap_or(Theme::DarkMode);

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let stats = DatabaseHandler::get_guild_stats(&mut transaction, &guild_id).await?;
  let chart_stats =
    DatabaseHandler::get_guild_chart_stats(&mut transaction, &guild_id, &timeframe).await?;

  let total_minutes = stats.sum.unwrap_or(0);
  let total_count = stats.count.unwrap_or(0);
  let timeframe_sum = chart_stats
    .iter()
    .fold(0, |total, session| total + session.sum.unwrap_or(0));
  let timeframe_count = chart_stats
    .iter()
    .fold(0, |total, session| total + session.count.unwrap_or(0));

  let mut embed = BloomBotEmbed::new()
    .title(format!("Stats for {guild_name}"))
    .author(CreateEmbedAuthor::new(format!("{guild_name}'s Stats")).icon_url(guild_icon));

  match stats_type {
    StatsType::MeditationMinutes => {
      embed = embed
        .field(
          "All-Time Meditation Minutes",
          format!("```{total_minutes}```"),
          true,
        )
        .field(
          format!("Minutes The Past 12 {timeframe_header}"),
          format!("```{timeframe_sum}```"),
          true,
        );
    }
    StatsType::MeditationCount => {
      embed = embed
        .field(
          "All-Time Session Count",
          format!("```{total_count}```"),
          true,
        )
        .field(
          format!("Sessions The Past 12 {timeframe_header}"),
          format!("```{timeframe_count}```"),
          true,
        );
    }
  }

  let options = StatsOptions::new(timeframe, 0, stats_type, chart_style, bar_color, theme);
  let chart = Chart::new().await?;
  let chart = chart.stats(&chart_stats, &options).await?;

  embed = embed.image(chart.url());
  let attachment = CreateAttachment::path(chart.path()).await?;

  ctx
    .send(CreateReply::default().embed(embed).attachment(attachment))
    .await?;

  chart.remove().await?;

  Ok(())
}

/// Show tracking leaderboard
///
/// Shows the tracking leaderboard, available in several configurations.
///
/// Defaults to monthly top 5, sorted by minutes, in dark mode. Optionally specify the timeframe (daily, weekly, monthly, or yearly), sort (minutes, sessions, or streak), and theme (light mode or dark mode).
#[poise::command(slash_command)]
async fn leaderboard(
  ctx: Context<'_>,
  #[description = "The leaderboard timeframe (Defaults to monthly)"] timeframe: Option<Timeframe>,
  #[description = "The stat to sort by (Defaults to minutes)"] sort: Option<SortBy>,
  #[description = "The leaderboard type (Defaults to Top 5)"]
  #[rename = "type"]
  leaderboard_type: Option<LeaderboardType>,
  #[description = "Toggle between light mode and dark mode (Defaults to dark mode)"] theme: Option<
    Theme,
  >,
) -> Result<()> {
  ctx.defer().await?;

  let timeframe = timeframe.unwrap_or(Timeframe::Monthly);
  let sort_by = sort.unwrap_or(SortBy::Minutes);
  let leaderboard_type = leaderboard_type.unwrap_or(LeaderboardType::Top5);
  let theme = theme.unwrap_or(Theme::DarkMode);

  if matches!(theme, Theme::DarkMode) {
    match open_leaderboard_file(&timeframe, &sort_by, &leaderboard_type).await {
      Ok(pregen_chart) => {
        let chart = pregen_chart;
        let embed = BloomBotEmbed::new().image(chart.url());
        let attachment = CreateAttachment::path(chart.path()).await?;

        if let Err(err) = ctx
          .send(
            CreateReply::default()
              .embed(embed)
              .ephemeral(false)
              .attachment(attachment),
          )
          .await
        {
          info!("Failed to send pre-generated leaderboard file: {err:?}");
          let msg = format!("{} Sorry, no leaderboard data available.", EMOJI.mminfo);
          ctx
            .send(CreateReply::default().content(msg).ephemeral(true))
            .await?;
        }
        return Ok(());
      }
      Err(e) => {
        info!("Failed to open pre-generated leaderboard file: {e:?}");
      }
    };
  }

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let stats = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    &guild_id,
    &timeframe,
    &sort_by,
    &leaderboard_type,
  )
  .await?;

  let Some(leaderboard_data) = leaderboards::process_stats(ctx.http(), &guild_id, &stats).await?
  else {
    let msg = format!("{} Sorry, no leaderboard data available.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  };

  let options = LeaderboardOptions::new(timeframe, sort_by, leaderboard_type, theme);
  let chart = Chart::new().await?;
  let chart = chart.leaderboard(leaderboard_data, &options).await?;

  let embed = BloomBotEmbed::new().image(chart.url());
  let attachment = CreateAttachment::path(chart.path()).await?;

  ctx
    .send(
      CreateReply::default()
        .embed(embed)
        .ephemeral(false)
        .attachment(attachment),
    )
    .await?;

  chart.remove().await?;

  Ok(())
}

async fn open_leaderboard_file<'a>(
  timeframe: &'a Timeframe,
  sort_by: &'a SortBy,
  leaderboard_type: &'a LeaderboardType,
) -> Result<Chart<'a>> {
  match timeframe {
    Timeframe::Yearly => match sort_by {
      SortBy::Minutes => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.year_min_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.year_min_top10_dark).await?),
      },
      SortBy::Sessions => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.year_ses_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.year_ses_top10_dark).await?),
      },
      SortBy::Streak => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.year_str_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.year_str_top10_dark).await?),
      },
    },
    Timeframe::Monthly => match sort_by {
      SortBy::Minutes => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.month_min_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.month_min_top10_dark).await?),
      },
      SortBy::Sessions => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.month_ses_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.month_ses_top10_dark).await?),
      },
      SortBy::Streak => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.month_str_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.month_str_top10_dark).await?),
      },
    },
    Timeframe::Weekly => match sort_by {
      SortBy::Minutes => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.week_min_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.week_min_top10_dark).await?),
      },
      SortBy::Sessions => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.week_ses_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.week_ses_top10_dark).await?),
      },
      SortBy::Streak => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.week_str_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.week_str_top10_dark).await?),
      },
    },
    Timeframe::Daily => match sort_by {
      SortBy::Minutes => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.day_min_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.day_min_top10_dark).await?),
      },
      SortBy::Sessions => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.day_ses_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.day_ses_top10_dark).await?),
      },
      SortBy::Streak => match leaderboard_type {
        LeaderboardType::Top5 => Ok(Chart::open(LEADERBOARDS.day_str_top5_dark).await?),
        LeaderboardType::Top10 => Ok(Chart::open(LEADERBOARDS.day_str_top10_dark).await?),
      },
    },
  }
}
