#![allow(clippy::unused_async)]

use crate::config::{BloomBotEmbed, ROLES};
use crate::database::Timeframe;
use crate::database::{DatabaseHandler, TrackingProfile};
use crate::Context;
use crate::{charts, config};
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::ChoiceParameter;

#[derive(poise::ChoiceParameter)]
pub enum StatsType {
  #[name = "Minutes"]
  MeditationMinutes,
  #[name = "Count"]
  MeditationCount,
}

#[derive(poise::ChoiceParameter)]
pub enum Privacy {
  #[name = "Private"]
  Private,
  #[name = "Public"]
  Public,
}

#[derive(poise::ChoiceParameter)]
pub enum Theme {
  #[name = "Light Mode"]
  LightMode,
  #[name = "Dark Mode"]
  DarkMode,
}

/// Show stats for a user or the server
///
/// Shows stats for yourself, a specified user, or the whole server.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  subcommands("user", "server"),
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
pub async fn user(
  ctx: Context<'_>,
  #[description = "The user to get the stats of (Defaults to you)"] user: Option<serenity::User>,
  #[description = "The type of stats to get (Defaults to minutes)"]
  #[rename = "type"]
  stats_type: Option<StatsType>,
  #[description = "The timeframe to get the stats for (Defaults to daily)"] timeframe: Option<
    Timeframe,
  >,
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
  let user_nick_or_name = user.nick_in(&ctx, guild_id).await.unwrap_or_else(|| {
    if let Some(global_name) = &user.global_name {
      global_name.clone()
    } else {
      user.name.clone()
    }
  });

  let tracking_profile =
    match DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user.id).await? {
      Some(tracking_profile) => tracking_profile,
      None => TrackingProfile {
        ..Default::default()
      },
    };

  let privacy = match privacy {
    Some(privacy) => match privacy {
      Privacy::Private => true,
      Privacy::Public => false,
    },
    None => tracking_profile.stats_private,
  };

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
      Some(color) => (color.r(), color.g(), color.b(), 1.0),
      None => (253, 172, 46, 1.0),
    }
  } else {
    (253, 172, 46, 1.0)
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

  let chart_stats =
    DatabaseHandler::get_user_chart_stats(&mut transaction, &guild_id, &user.id, &timeframe)
      .await?;
  let chart_drawer = charts::ChartDrawer::new()?;
  let chart = chart_drawer
    .draw(&chart_stats, &timeframe, &stats_type, bar_color, light_mode)
    .await?;
  let file_path = chart.get_file_path();

  embed = embed.image(chart.get_attachment_url());

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
      stats.streak
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

  Ok(())
}

/// Show stats for the server
///
/// Shows stats for the whole server.
///
/// Defaults to daily minutes. Optionally specify the type (minutes or session count) and/or timeframe (daily, weekly, monthly, or yearly).
#[poise::command(slash_command)]
pub async fn server(
  ctx: Context<'_>,
  #[description = "The type of stats to get (Defaults to minutes)"] stats_type: Option<StatsType>,
  #[description = "The timeframe to get the stats for (Defaults to daily)"] timeframe: Option<
    Timeframe,
  >,
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

  let bar_color = (253, 172, 46, 1.0);
  let light_mode = match theme {
    Some(theme) => match theme {
      Theme::LightMode => true,
      Theme::DarkMode => false,
    },
    None => false,
  };

  let chart_stats =
    DatabaseHandler::get_guild_chart_stats(&mut transaction, &guild_id, &timeframe).await?;
  let chart_drawer = charts::ChartDrawer::new()?;
  let chart = chart_drawer
    .draw(&chart_stats, &timeframe, &stats_type, bar_color, light_mode)
    .await?;
  let file_path = chart.get_file_path();

  embed = embed.image(chart.get_attachment_url());

  ctx
    .send({
      let mut f =
        poise::CreateReply::default().attachment(CreateAttachment::path(&file_path).await?);
      f.embeds = vec![embed.clone()];

      f
    })
    .await?;

  Ok(())
}
