#![allow(clippy::cast_precision_loss)]

use crate::commands::helpers::time::ChallengeTimeframe;
use crate::config::{BloomBotEmbed, EMOJI, ROLES};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use chrono::{Datelike, Timelike, Utc};
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::CreateReply;

#[derive(poise::ChoiceParameter)]
enum ChallengeChoices {
  #[name = "Monthly Challenge"]
  Monthly,
  #[name = "365-Day Challenge"]
  YearRound,
}

/// Participate in a meditation challenge
///
/// Join or leave the monthly or 365-day meditation challenge, or check your challenge stats.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  subcommands("join", "leave", "stats"),
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn challenge(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Join a meditation challenge
///
/// Join the monthly or 365-day meditation challenge.
#[poise::command(slash_command)]
async fn join(
  ctx: Context<'_>,
  #[description = "Challenge you wish to join (Defaults to monthly)"] challenge: Option<
    ChallengeChoices,
  >,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let member = guild_id.member(ctx, ctx.author().id).await?;

  if let Some(challenge) = challenge {
    match challenge {
      ChallengeChoices::Monthly => {
        if ctx
          .author()
          .has_role(ctx, guild_id, ROLES.meditation_challenger)
          .await?
        {
          ctx
            .send(
              CreateReply::default()
                .content("You've already joined the monthly challenge. Awesome!")
                .ephemeral(true),
            )
            .await?;

          return Ok(());
        }

        member.add_role(ctx, ROLES.meditation_challenger).await?;

        ctx.say(format!(
    "Challenge accepted! You're awesome, <@{}>! Now commit to practicing consistently throughout the month of {} and `/add` your times in this channel. You can use <#534702592245235733> and <#465656096929873942> for extra accountability. Let's do this!",
    member.user.id,
    chrono::Utc::now().format("%B"),
    )).await?;

        return Ok(());
      }
      ChallengeChoices::YearRound => {
        if ctx
          .author()
          .has_role(ctx, guild_id, ROLES.meditation_challenger_365)
          .await?
        {
          ctx
            .send(
              CreateReply::default()
                .content("You've already joined the 365-day challenge. Awesome!")
                .ephemeral(true),
            )
            .await?;

          return Ok(());
        }

        member
          .add_role(ctx, ROLES.meditation_challenger_365)
          .await?;

        ctx
          .say(format!(
            "Awesome, <@{}>! You have successfully joined the 365-day challenge {}",
            member.user.id, EMOJI.pepeglow,
          ))
          .await?;

        return Ok(());
      }
    }
  }

  // Defaults to monthly
  if ctx
    .author()
    .has_role(ctx, guild_id, ROLES.meditation_challenger)
    .await?
  {
    ctx
      .send(
        CreateReply::default()
          .content("You've already joined the monthly challenge. Awesome!")
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  member.add_role(ctx, ROLES.meditation_challenger).await?;

  ctx.say(format!(
    "Challenge accepted! You're awesome, <@{}>! Now commit to practicing consistently throughout the month of {} and `/add` your times in this channel. You can use <#534702592245235733> and <#465656096929873942> for extra accountability. Let's do this!",
    member.user.id,
    chrono::Utc::now().format("%B"),
    )).await?;

  Ok(())
}

/// Leave a meditation challenge
///
/// Leave the monthly or 365-day meditation challenge.
#[poise::command(slash_command)]
async fn leave(
  ctx: Context<'_>,
  #[description = "Challenge you wish to leave (Defaults to monthly)"] challenge: Option<
    ChallengeChoices,
  >,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let member = guild_id.member(ctx, ctx.author().id).await?;

  if let Some(challenge) = challenge {
    match challenge {
      ChallengeChoices::Monthly => {
        if ctx
          .author()
          .has_role(ctx, guild_id, ROLES.meditation_challenger)
          .await?
        {
          member.remove_role(ctx, ROLES.meditation_challenger).await?;

          ctx
            .say(format!(
              "You have successfully opted out of the monthly challenge, <@{}>.",
              member.user.id,
            ))
            .await?;

          return Ok(());
        }

        ctx
          .send(CreateReply::default()
          .content("You're not currently participating in the monthly challenge. If you want to join, use `/challenge join`.")
          .ephemeral(true)
          )
          .await?;

        return Ok(());
      }
      ChallengeChoices::YearRound => {
        if ctx
          .author()
          .has_role(ctx, guild_id, ROLES.meditation_challenger_365)
          .await?
        {
          member
            .remove_role(ctx, ROLES.meditation_challenger_365)
            .await?;

          ctx
            .say(format!(
              "You have successfully opted out of the 365-day challenge, <@{}>.",
              member.user.id,
            ))
            .await?;

          return Ok(());
        }

        ctx
          .send(CreateReply::default()
          .content("You're not currently participating in the 365-day challenge. If you want to join, use `/challenge join`.")
          .ephemeral(true)
          )
          .await?;

        return Ok(());
      }
    }
  }

  // Defaults to monthly
  if ctx
    .author()
    .has_role(ctx, guild_id, ROLES.meditation_challenger)
    .await?
  {
    member.remove_role(ctx, ROLES.meditation_challenger).await?;

    ctx
      .say(format!(
        "You have successfully opted out of the monthly challenge, <@{}>.",
        member.user.id,
      ))
      .await?;

    return Ok(());
  }

  ctx
    .send(CreateReply::default()
    .content("You're not currently participating in the monthly challenge. If you want to join, use `/challenge join`.")
    .ephemeral(true)
    )
    .await?;

  Ok(())
}

/// View your challenge stats
///
/// View your stats for the current monthly or 365-day meditation challenge.
#[poise::command(slash_command)]
async fn stats(
  ctx: Context<'_>,
  #[description = "Challenge you wish to see stats for (Defaults to monthly)"] challenge: Option<
    ChallengeTimeframe,
  >,
) -> Result<()> {
  let data = ctx.data();
  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let member = guild_id.member(ctx, ctx.author().id).await?;
  let timeframe = challenge.unwrap_or(ChallengeTimeframe::Monthly);

  if timeframe == ChallengeTimeframe::YearRound {
    if member
      .roles
      .contains(&serenity::RoleId::from(ROLES.meditation_challenger_365))
    {
      let member_nick_or_name = match &member.nick {
        Some(nick) => nick.clone(),
        None => member
          .user
          .global_name
          .as_ref()
          .unwrap_or(&member.user.name)
          .clone(),
      };

      let tracking_profile =
        DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &member.user.id)
          .await?
          .unwrap_or_default();

      if tracking_profile.stats_private {
        ctx.defer_ephemeral().await?;
      } else {
        ctx.defer().await?;
      }

      let stats = DatabaseHandler::get_challenge_stats(
        &mut transaction,
        &guild_id,
        &member.user.id,
        &timeframe,
      )
      .await?;

      let days = {
        let end_time = Utc::now();
        let start_time = end_time
          .with_month(1)
          .unwrap_or_default()
          .with_day(1)
          .unwrap_or_default()
          .with_hour(0)
          .unwrap_or_default()
          .with_minute(0)
          .unwrap_or_default();
        let days = (end_time - start_time).num_days();
        if days == 0 {
          1
        } else {
          days
        }
      };

      let total_time = stats.timeframe_stats.sum.unwrap_or(0) as f64;
      let total_hrs = (total_time.trunc() / 60.0).trunc();
      let total_min = (total_time.trunc() / 60.0).fract() * 60.0;
      let total_sec = total_time.fract() * 60.0;

      let total_h = if total_hrs > 0.0 {
        format!("{total_hrs:.0}h ")
      } else {
        String::new()
      };
      let total_m = if total_min > 0.0 {
        format!("{total_min:.0}m ")
      } else {
        String::new()
      };
      let total_s = if total_sec > 0.0 {
        format!("{total_sec:.0}s")
      } else {
        String::new()
      };

      let avg_time = stats.timeframe_stats.sum.unwrap_or(0) as f64 / days as f64;
      let avg_hrs = (avg_time.trunc() / 60.0).trunc();
      let avg_min = (avg_time.trunc() / 60.0).fract() * 60.0;
      let avg_sec = avg_time.fract() * 60.0;

      let avg_h = if avg_hrs > 0.0 {
        format!("{avg_hrs:.0}h ")
      } else {
        String::new()
      };
      let avg_m = if avg_min > 0.0 {
        format!("{avg_min:.0}m ")
      } else {
        String::new()
      };
      let avg_s = if avg_sec > 0.0 {
        format!("{avg_sec:.0}s")
      } else {
        String::new()
      };

      let mut embed = BloomBotEmbed::new();
      embed = embed
        .title("365-Day Meditation Challenge Stats")
        .author(CreateEmbedAuthor::new(member_nick_or_name).icon_url(member.user.face()))
        .field(
          "Time",
          format!("```yml\nChallenge Total: {total_h}{total_m}{total_s}\nAverage Per Day: {avg_h}{avg_m}{avg_s}```"),
          false,
        )
        .field(
          "Sessions",
          format!(
            "```yml\nChallenge Total: {}\nAverage Per Day: {:.2}```",
            stats.timeframe_stats.count.unwrap_or(0),
            ((stats.timeframe_stats.count.unwrap_or(0) as f64 / days as f64) * 100.0).round() / 100.0
          ),
          false,
        );

      // Hide streaks if streaks disabled
      if tracking_profile.streaks_active
        // Hide streaks if streak set to private, unless own stats in ephemeral
        && (!tracking_profile.streaks_private || tracking_profile.stats_private)
      {
        embed = embed.field(
          "Streaks",
          format!(
            "```yml\nCurrent Streak: {}\nLongest Streak: {}```",
            stats.streak.current, stats.streak.longest
          ),
          false,
        );
      }

      embed = embed.footer(CreateEmbedFooter::new(format!(
        "Stats for 365-Day Challenge ({})",
        Utc::now().format("%Y")
      )));

      ctx.send(CreateReply::default().embed(embed)).await?;

      return Ok(());
    }
    ctx
          .send(CreateReply::default()
          .content("You're not currently participating in the 365-day challenge. If you want to join, use `/challenge join`.")
          .ephemeral(true)
          )
          .await?;

    return Ok(());
  }

  // Defaults to monthly
  if member
    .roles
    .contains(&serenity::RoleId::from(ROLES.meditation_challenger))
  {
    let member_nick_or_name = match &member.nick {
      Some(nick) => nick.clone(),
      None => member
        .user
        .global_name
        .as_ref()
        .unwrap_or(&member.user.name)
        .clone(),
    };

    let tracking_profile =
      DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &member.user.id)
        .await?
        .unwrap_or_default();

    if tracking_profile.stats_private {
      ctx.defer_ephemeral().await?;
    } else {
      ctx.defer().await?;
    }

    let stats = DatabaseHandler::get_challenge_stats(
      &mut transaction,
      &guild_id,
      &member.user.id,
      &timeframe,
    )
    .await?;

    let days = {
      let end_time = Utc::now();
      let start_time = end_time
        .with_day(1)
        .unwrap_or_default()
        .with_hour(0)
        .unwrap_or_default()
        .with_minute(0)
        .unwrap_or_default();
      let days = (end_time - start_time).num_days();
      if days == 0 {
        1
      } else {
        days
      }
    };

    let total_time = stats.timeframe_stats.sum.unwrap_or(0) as f64;
    let total_hrs = (total_time.trunc() / 60.0).trunc();
    let total_min = (total_time.trunc() / 60.0).fract() * 60.0;
    let total_sec = total_time.fract() * 60.0;

    let total_h = if total_hrs > 0.0 {
      format!("{total_hrs:.0}h ")
    } else {
      String::new()
    };
    let total_m = if total_min > 0.0 {
      format!("{total_min:.0}m ")
    } else {
      String::new()
    };
    let total_s = if total_sec > 0.0 {
      format!("{total_sec:.0}s")
    } else {
      String::new()
    };

    let avg_time = stats.timeframe_stats.sum.unwrap_or(0) as f64 / days as f64;
    let avg_hrs = (avg_time.trunc() / 60.0).trunc();
    let avg_min = (avg_time.trunc() / 60.0).fract() * 60.0;
    let avg_sec = avg_time.fract() * 60.0;

    let avg_h = if avg_hrs > 0.0 {
      format!("{avg_hrs:.0}h ")
    } else {
      String::new()
    };
    let avg_m = if avg_min > 0.0 {
      format!("{avg_min:.0}m ")
    } else {
      String::new()
    };
    let avg_s = if avg_sec > 0.0 {
      format!("{avg_sec:.0}s")
    } else {
      String::new()
    };

    let mut embed = BloomBotEmbed::new();
    embed = embed
      .title("Monthly Meditation Challenge Stats")
      .author(CreateEmbedAuthor::new(member_nick_or_name).icon_url(member.user.face()))
      .field(
        "Time",
        format!("```yml\nChallenge Total: {total_h}{total_m}{total_s}\nAverage Per Day: {avg_h}{avg_m}{avg_s}```"),
        false,
      )
      .field(
        "Sessions",
        format!(
          "```yml\nChallenge Total: {}\nAverage Per Day: {:.2}```",
          stats.timeframe_stats.count.unwrap_or(0),
          ((stats.timeframe_stats.count.unwrap_or(0) as f64 / days as f64) * 100.0).round() / 100.0
        ),
        false,
      );

    // Hide streaks if streaks disabled
    if tracking_profile.streaks_active
      // Hide streaks if streak set to private, unless own stats in ephemeral
      && (!tracking_profile.streaks_private || tracking_profile.stats_private)
    {
      embed = embed.field(
        "Streaks",
        format!(
          "```yml\nCurrent Streak: {}\nLongest Streak: {}```",
          stats.streak.current, stats.streak.longest
        ),
        false,
      );
    }

    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Stats for {} Monthly Challenge",
      Utc::now().format("%B %Y")
    )));

    ctx.send(CreateReply::default().embed(embed)).await?;

    return Ok(());
  }

  ctx
    .send(CreateReply::default()
    .content("You're not currently participating in the monthly challenge. If you want to join, use `/challenge join`.")
    .ephemeral(true)
    )
    .await?;

  Ok(())
}
