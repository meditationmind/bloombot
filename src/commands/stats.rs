#![allow(clippy::unused_async)]

use crate::commands::helpers::time::Timeframe;
use crate::commands::helpers::tracking::{privacy, Privacy};
use crate::config::{BloomBotEmbed, EMOJI, ROLES};
use crate::data::tracking_profile::TrackingProfile;
use crate::database::DatabaseHandler;
use crate::events::leaderboards::{self, LEADERBOARDS};
use crate::Context;
use crate::{charts, config};
use anyhow::{Context as AnyhowContext, Result};
use log::info;
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::ChoiceParameter;

#[allow(clippy::module_name_repetitions)]
#[derive(poise::ChoiceParameter)]
pub enum StatsType {
  #[name = "minutes"]
  MeditationMinutes,
  #[name = "count"]
  MeditationCount,
}

#[derive(poise::ChoiceParameter)]
pub enum ChartStyle {
  #[name = "bar chart"]
  Bar,
  #[name = "area chart"]
  Area,
  #[name = "bar chart (combined data)"]
  BarCombined,
}

#[derive(poise::ChoiceParameter)]
pub enum SortBy {
  #[name = "minutes"]
  Minutes,
  #[name = "sessions"]
  Sessions,
  #[name = "streak"]
  Streak,
}

#[derive(poise::ChoiceParameter)]
pub enum LeaderboardType {
  #[name = "Top 5"]
  Top5,
  #[name = "Top 10"]
  Top10,
}

#[derive(poise::ChoiceParameter)]
enum Theme {
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
  #[description = "The user to get the stats of (Defaults to you)"] user: Option<serenity::User>,
  #[description = "The type of stats to get (Defaults to minutes)"]
  #[rename = "type"]
  stats_type: Option<StatsType>,
  #[description = "The timeframe to get the stats for (Defaults to daily)"] timeframe: Option<
    Timeframe,
  >,
  #[description = "The style of chart (Defaults to bar chart)"] style: Option<ChartStyle>,
  #[description = "Set visibility of response (Defaults to public)"] privacy: Option<Privacy>,
  #[description = "Toggle between light mode and dark mode (Defaults to dark mode)"] theme: Option<
    Theme,
  >,
) -> Result<()> {
  let data = ctx.data();
  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let user = user.unwrap_or_else(|| ctx.author().clone());
  let user_nick_or_name = user
    .nick_in(&ctx, guild_id)
    .await
    .unwrap_or_else(|| user.global_name.as_ref().unwrap_or(&user.name).clone());

  let tracking_profile =
    match DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user.id).await? {
      Some(tracking_profile) => tracking_profile,
      None => TrackingProfile {
        ..Default::default()
      },
    };

  let privacy = privacy!(privacy, tracking_profile.stats_private);

  if privacy {
    ctx.defer_ephemeral().await?;
  } else {
    ctx.defer().await?;
  }

  if ctx.author().id != user.id
    && tracking_profile.stats_private
    && !ctx.author().has_role(&ctx, guild_id, ROLES.staff).await?
  {
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!(
            "Sorry, {user_nick_or_name}'s stats are set to private."
          ))
          .ephemeral(true)
          .allowed_mentions(serenity::CreateAllowedMentions::new()),
      )
      .await?;

    return Ok(());
  }

  let chart_style = style.unwrap_or(ChartStyle::Bar);
  let stats_type = stats_type.unwrap_or(StatsType::MeditationMinutes);
  let timeframe = timeframe.unwrap_or(Timeframe::Daily);

  let timeframe_header = match timeframe {
    Timeframe::Yearly => "Years",
    Timeframe::Monthly => "Months",
    Timeframe::Weekly => "Weeks",
    Timeframe::Daily => "Days",
  };

  let stats =
    DatabaseHandler::get_user_stats(&mut transaction, &guild_id, &user.id, &timeframe).await?;

  let mut embed = BloomBotEmbed::new();
  embed = embed
    .title(format!("Stats for {user_nick_or_name}"))
    .author(CreateEmbedAuthor::new(format!("{user_nick_or_name}'s Stats")).icon_url(user.face()));

  match stats_type {
    StatsType::MeditationMinutes => {
      embed = embed
        .field(
          "All-Time Meditation Minutes",
          format!("```{}```", stats.all_minutes),
          true,
        )
        .field(
          format!("Minutes The Past 12 {timeframe_header}"),
          format!("```{}```", stats.timeframe_stats.sum.unwrap_or(0)),
          true,
        );
    }
    StatsType::MeditationCount => {
      embed = embed
        .field(
          "All-Time Session Count",
          format!("```{}```", stats.all_count),
          true,
        )
        .field(
          format!("Sessions The Past 12 {timeframe_header}"),
          format!("```{}```", stats.timeframe_stats.count.unwrap_or(0)),
          true,
        );
    }
  }

  // Role-based bar color for donators; default otherwise
  let bar_color = if user.has_role(&ctx, guild_id, config::ROLES.patreon).await?
    || user.has_role(&ctx, guild_id, config::ROLES.kofi).await?
  {
    match guild_id.member(&ctx, user.id).await?.colour(ctx) {
      Some(color) => (color.r(), color.g(), color.b(), 255),
      None => (253, 172, 46, 255),
    }
  } else {
    (253, 172, 46, 255)
  };

  // Role-based bar color for all users
  //let bar_color = match guild_id.member(&ctx, user.id).await?.colour(&ctx) {
  //  Some(color) => (color.r(), color.g(), color.b(), 1.0),
  //  None => (253, 172, 46, 1.0)
  //};

  let light_mode = match theme {
    Some(theme) => match theme {
      Theme::LightMode => true,
      Theme::DarkMode => false,
    },
    None => false,
  };

  let chart_stats = DatabaseHandler::get_user_chart_stats(
    &mut transaction,
    &guild_id,
    &user.id,
    &timeframe,
    tracking_profile.utc_offset,
  )
  .await?;

  let chart = charts::Chart::new()
    .await?
    .stats(
      &chart_stats,
      &timeframe,
      tracking_profile.utc_offset,
      &stats_type,
      &chart_style,
      bar_color,
      light_mode,
    )
    .await?;

  let file_path = chart.path();

  embed = embed.image(chart.url());

  let average = match stats_type {
    StatsType::MeditationMinutes => stats.timeframe_stats.sum.unwrap_or(0) / 12,
    StatsType::MeditationCount => stats.timeframe_stats.count.unwrap_or(0) / 12,
  };

  let stats_type_label = match stats_type {
    StatsType::MeditationMinutes => "minutes",
    StatsType::MeditationCount => "sessions",
  };

  // Hide streak in footer if streaks disabled
  if tracking_profile.streaks_active
    // Hide streak in footer if streak set to private, unless own stats in ephemeral
    && (!tracking_profile.streaks_private || (ctx.author().id == user.id && privacy))
  {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Avg. {} {}: {}・Current streak: {}",
      timeframe.name().to_lowercase(),
      stats_type_label,
      average,
      stats.streak.current
    )));
  } else {
    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Average {} {}: {}",
      timeframe.name().to_lowercase(),
      stats_type_label,
      average
    )));
  }

  ctx
    .send({
      let mut f =
        poise::CreateReply::default().attachment(CreateAttachment::path(&file_path).await?);
      f.embeds = vec![embed.clone()];

      f
    })
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
  #[description = "The type of stats to get (Defaults to minutes)"]
  #[rename = "type"]
  stats_type: Option<StatsType>,
  #[description = "The timeframe to get the stats for (Defaults to daily)"] timeframe: Option<
    Timeframe,
  >,
  #[description = "The style of chart (Defaults to bar chart)"] style: Option<ChartStyle>,
  #[description = "Toggle between light mode and dark mode (Defaults to dark mode)"] theme: Option<
    Theme,
  >,
) -> Result<()> {
  ctx.defer().await?;

  let data = ctx.data();

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

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let stats = DatabaseHandler::get_guild_stats(&mut transaction, &guild_id, &timeframe).await?;

  let mut embed = BloomBotEmbed::new();
  embed = embed
    .title(format!("Stats for {guild_name}"))
    .author(CreateEmbedAuthor::new(format!("{guild_name}'s Stats")).icon_url(guild_icon));

  match stats_type {
    StatsType::MeditationMinutes => {
      embed = embed
        .field(
          "All-Time Meditation Minutes",
          format!("```{}```", stats.all_minutes),
          true,
        )
        .field(
          format!("Minutes The Past 12 {timeframe_header}"),
          format!("```{}```", stats.timeframe_stats.sum.unwrap_or(0)),
          true,
        );
    }
    StatsType::MeditationCount => {
      embed = embed
        .field(
          "All-Time Session Count",
          format!("```{}```", stats.all_count),
          true,
        )
        .field(
          format!("Sessions The Past 12 {timeframe_header}"),
          format!("```{}```", stats.timeframe_stats.count.unwrap_or(0)),
          true,
        );
    }
  }

  let bar_color = (253, 172, 46, 255);
  let light_mode = match theme {
    Some(theme) => match theme {
      Theme::LightMode => true,
      Theme::DarkMode => false,
    },
    None => false,
  };

  let chart_stats =
    DatabaseHandler::get_guild_chart_stats(&mut transaction, &guild_id, &timeframe).await?;

  let chart = charts::Chart::new()
    .await?
    .stats(
      &chart_stats,
      &timeframe,
      0,
      &stats_type,
      &chart_style,
      bar_color,
      light_mode,
    )
    .await?;

  let file_path = chart.path();

  embed = embed.image(chart.url());

  ctx
    .send({
      let mut f =
        poise::CreateReply::default().attachment(CreateAttachment::path(&file_path).await?);
      f.embeds = vec![embed.clone()];

      f
    })
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

  let light_mode = match theme {
    Some(theme) => match theme {
      Theme::LightMode => true,
      Theme::DarkMode => false,
    },
    None => false,
  };

  if !light_mode {
    let chart = match timeframe {
      Timeframe::Yearly => match sort_by {
        SortBy::Minutes => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.year_min_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.year_min_top10_dark).await?,
        },
        SortBy::Sessions => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.year_ses_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.year_ses_top10_dark).await?,
        },
        SortBy::Streak => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.year_str_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.year_str_top10_dark).await?,
        },
      },
      Timeframe::Monthly => match sort_by {
        SortBy::Minutes => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.month_min_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.month_min_top10_dark).await?,
        },
        SortBy::Sessions => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.month_ses_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.month_ses_top10_dark).await?,
        },
        SortBy::Streak => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.month_str_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.month_str_top10_dark).await?,
        },
      },
      Timeframe::Weekly => match sort_by {
        SortBy::Minutes => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.week_min_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.week_min_top10_dark).await?,
        },
        SortBy::Sessions => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.week_ses_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.week_ses_top10_dark).await?,
        },
        SortBy::Streak => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.week_str_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.week_str_top10_dark).await?,
        },
      },
      Timeframe::Daily => match sort_by {
        SortBy::Minutes => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.day_min_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.day_min_top10_dark).await?,
        },
        SortBy::Sessions => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.day_ses_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.day_ses_top10_dark).await?,
        },
        SortBy::Streak => match leaderboard_type {
          LeaderboardType::Top5 => charts::Chart::open(LEADERBOARDS.day_str_top5_dark).await?,
          LeaderboardType::Top10 => charts::Chart::open(LEADERBOARDS.day_str_top10_dark).await?,
        },
      },
    };

    let file_path = chart.path();

    let embed = BloomBotEmbed::new().image(chart.url());

    if let Err(err) = ctx
      .send(
        poise::CreateReply::default()
          .embed(embed)
          .ephemeral(false)
          .attachment(CreateAttachment::path(&file_path).await?),
      )
      .await
    {
      info!("Failed to send pre-generated leaderboard file: {:?}", err);
      ctx
        .send(
          poise::CreateReply::default()
            .content(format!(
              "{} Sorry, no leaderboard data available.",
              EMOJI.mminfo
            ))
            .ephemeral(true)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
        )
        .await?;
    }
    return Ok(());
  }

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let data = ctx.data();
  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let stats = DatabaseHandler::get_leaderboard_stats(
    &mut transaction,
    &guild_id,
    &timeframe,
    &sort_by,
    &leaderboard_type,
  )
  .await?;

  let leaderboard_data = leaderboards::process_stats(ctx.http(), &guild_id, &stats).await?;

  if let Some(leaderboard_data) = leaderboard_data {
    let chart = charts::Chart::new()
      .await?
      .leaderboard(
        leaderboard_data,
        &timeframe,
        &sort_by,
        &leaderboard_type,
        light_mode,
      )
      .await?;

    let file_path = chart.path();

    let embed = BloomBotEmbed::new().image(chart.url());

    ctx
      .send(
        poise::CreateReply::default()
          .embed(embed)
          .ephemeral(false)
          .attachment(CreateAttachment::path(&file_path).await?),
      )
      .await?;

    chart.remove().await?;

    return Ok(());
  }

  ctx
    .send(
      poise::CreateReply::default()
        .content(format!(
          "{} Sorry, no leaderboard data available.",
          EMOJI.mminfo
        ))
        .ephemeral(true)
        .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

  Ok(())
}
