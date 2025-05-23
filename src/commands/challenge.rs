#![allow(clippy::cast_precision_loss)]

use anyhow::{Context as AnyhowContext, Result};
use chrono::{Datelike, Timelike, Utc};
use poise::serenity_prelude::{RoleId, builder::*};
use poise::{ChoiceParameter, CreateReply};

use crate::Context;
use crate::commands::helpers::time::ChallengeTimeframe;
use crate::config::{BloomBotEmbed, EMOJI, ROLES};
use crate::data::stats::User;
use crate::data::tracking_profile::{Privacy, Status};
use crate::database::DatabaseHandler;

#[derive(ChoiceParameter)]
enum ChallengeChoices {
  #[name = "Monthly Challenge"]
  Monthly,
  #[name = "365-Day Challenge"]
  YearRound,
}

struct ProcessedStats {
  days: i64,
  total_h: String,
  total_m: String,
  total_s: String,
  avg_h: String,
  avg_m: String,
  avg_s: String,
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

  let challenge = challenge.unwrap_or(ChallengeChoices::Monthly);

  match challenge {
    ChallengeChoices::Monthly => {
      if ctx
        .author()
        .has_role(ctx, guild_id, ROLES.meditation_challenger)
        .await?
      {
        let msg = "You've already joined the monthly challenge. Awesome!";
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      }

      if member
        .add_role(ctx, ROLES.meditation_challenger)
        .await
        .is_err()
      {
        let msg = format!(
          "{} An error occurred while updating your roles. Please try again, or contact server staff for assistance.",
          EMOJI.mminfo
        );
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      }

      let msg = format!(
        "Challenge accepted! You're awesome, {member}! Now commit to practicing consistently throughout the month of {} and `/add` your times in this channel. You can use <#534702592245235733> and <#465656096929873942> for extra accountability. Let's do this!",
        Utc::now().format("%B")
      );
      ctx.say(msg).await?;
    }
    ChallengeChoices::YearRound => {
      if ctx
        .author()
        .has_role(ctx, guild_id, ROLES.meditation_challenger_365)
        .await?
      {
        let msg = "You've already joined the 365-day challenge. Awesome!";
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      }

      if member
        .add_role(ctx, ROLES.meditation_challenger_365)
        .await
        .is_err()
      {
        let msg = format!(
          "{} An error occurred while updating your roles. Please try again, or contact server staff for assistance.",
          EMOJI.mminfo
        );
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      }

      let msg = format!(
        "Awesome, {member}! You have successfully joined the 365-day challenge {}",
        EMOJI.pepeglow
      );
      ctx.say(msg).await?;
    }
  }

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

  let challenge = challenge.unwrap_or(ChallengeChoices::Monthly);

  match challenge {
    ChallengeChoices::Monthly => {
      if ctx
        .author()
        .has_role(ctx, guild_id, ROLES.meditation_challenger)
        .await?
      {
        if member
          .remove_role(ctx, ROLES.meditation_challenger)
          .await
          .is_err()
        {
          let msg = format!(
            "{} An error occurred while updating your roles. Please try again, or contact server staff for assistance.",
            EMOJI.mminfo
          );
          ctx
            .send(CreateReply::default().content(msg).ephemeral(true))
            .await?;
          return Ok(());
        }

        let msg = format!("You have successfully opted out of the monthly challenge, {member}.");
        ctx.say(msg).await?;

        return Ok(());
      }

      let msg = "You're not currently participating in the monthly challenge. If you want to join, use `/challenge join`.";
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
    }
    ChallengeChoices::YearRound => {
      if ctx
        .author()
        .has_role(ctx, guild_id, ROLES.meditation_challenger_365)
        .await?
      {
        if member
          .remove_role(ctx, ROLES.meditation_challenger_365)
          .await
          .is_err()
        {
          let msg = format!(
            "{} An error occurred while updating your roles. Please try again, or contact server staff for assistance.",
            EMOJI.mminfo
          );
          ctx
            .send(CreateReply::default().content(msg).ephemeral(true))
            .await?;
          return Ok(());
        }

        let msg = format!("You have successfully opted out of the 365-day challenge, {member}.");
        ctx.say(msg).await?;

        return Ok(());
      }

      let msg = "You're not currently participating in the 365-day challenge. If you want to join, use `/challenge join`.";
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
    }
  }

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
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  let member = ctx
    .author_member()
    .await
    .with_context(|| "Failed to retrieve author member from context, cache, or HTTP")?;

  let timeframe = challenge.unwrap_or(ChallengeTimeframe::Monthly);
  let role = match timeframe {
    ChallengeTimeframe::Monthly => &RoleId::from(ROLES.meditation_challenger),
    ChallengeTimeframe::YearRound => &RoleId::from(ROLES.meditation_challenger_365),
  };

  if !member.roles.contains(role) {
    let msg = format!(
      "{} You're not currently participating in the {}. If you want to join, use </challenge join:1187466829547978904>.",
      EMOJI.mminfo,
      timeframe.name().to_ascii_lowercase()
    );
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let member_nick_or_name = member.nick.as_deref().unwrap_or_else(|| {
    member
      .user
      .global_name
      .as_ref()
      .unwrap_or(&member.user.name)
  });

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let tracking_profile =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &member.user.id)
      .await?
      .unwrap_or_default();

  if tracking_profile.stats.privacy == Privacy::Private {
    ctx.defer_ephemeral().await?;
  } else {
    ctx.defer().await?;
  }

  let stats =
    DatabaseHandler::get_challenge_stats(&mut transaction, &guild_id, &member.user.id, &timeframe)
      .await?;
  let s = process_stats(&stats, &timeframe)?;

  let title = match timeframe {
    ChallengeTimeframe::Monthly => "Monthly Meditation Challenge Stats",
    ChallengeTimeframe::YearRound => "365-Day Meditation Challenge Stats",
  };

  let footer = match timeframe {
    ChallengeTimeframe::Monthly => CreateEmbedFooter::new(format!(
      "Stats for {} Monthly Challenge",
      Utc::now().format("%B %Y")
    )),
    ChallengeTimeframe::YearRound => CreateEmbedFooter::new(format!(
      "Stats for 365-Day Challenge ({})",
      Utc::now().format("%Y")
    )),
  };

  let count = stats.sessions.count.unwrap_or(0);
  let mut embed = BloomBotEmbed::new()
    .title(title)
    .author(CreateEmbedAuthor::new(member_nick_or_name).icon_url(member.user.face()))
    .field(
      "Time",
      format!(
        "```yml\nChallenge Total: {}{}{}\nAverage Per Day: {}{}{}```",
        s.total_h, s.total_m, s.total_s, s.avg_h, s.avg_m, s.avg_s
      ),
      false,
    )
    .field(
      "Sessions",
      format!(
        "```yml\nChallenge Total: {count}\nAverage Per Day: {:.2}```",
        ((count as f64 / s.days as f64) * 100.0).round() / 100.0
      ),
      false,
    )
    .footer(footer);

  // Hide streaks if streaks disabled.
  if tracking_profile.streak.status == Status::Enabled
      // Hide streaks if streak set to private, unless own stats in ephemeral.
      && (tracking_profile.streak.privacy == Privacy::Public || tracking_profile.stats.privacy == Privacy::Private)
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

  ctx.send(CreateReply::default().embed(embed)).await?;

  Ok(())
}

fn process_stats(stats: &User, timeframe: &ChallengeTimeframe) -> Result<ProcessedStats> {
  let days = {
    let end_time = Utc::now();
    let start_time = match timeframe {
      ChallengeTimeframe::Monthly => end_time
        .with_day(1)
        .with_context(|| "Failed to set day to 1")?
        .with_hour(0)
        .with_context(|| "Failed to set hour to 0")?
        .with_minute(0)
        .with_context(|| "Failed to set minute to 0")?,
      ChallengeTimeframe::YearRound => end_time
        .with_month(1)
        .with_context(|| "Failed to set month to 1")?
        .with_day(1)
        .with_context(|| "Failed to set day to 1")?
        .with_hour(0)
        .with_context(|| "Failed to set hour to 0")?
        .with_minute(0)
        .with_context(|| "Failed to set minute to 0")?,
    };
    let days = (end_time - start_time).num_days();
    if days == 0 { 1 } else { days }
  };

  let total_time = stats.sessions.sum.unwrap_or(0) as f64;
  let total_hrs = (total_time.trunc() / 60.0).trunc();
  let total_min = (total_time.trunc() / 60.0).fract() * 60.0;
  let total_sec = total_time.fract() * 60.0;

  let total_h = if total_hrs > 0.0 {
    format!("{total_hrs:.0}h ")
  } else {
    String::new()
  };
  let total_s = if total_sec > 0.0 {
    format!("{total_sec:.0}s")
  } else {
    String::new()
  };
  let total_m = if (total_min > 0.0) || (total_h.is_empty() && total_s.is_empty()) {
    format!("{total_min:.0}m ")
  } else {
    String::new()
  };

  let avg_time = total_time / days as f64;
  let avg_hrs = (avg_time.trunc() / 60.0).trunc();
  let avg_min = (avg_time.trunc() / 60.0).fract() * 60.0;
  let avg_sec = avg_time.fract() * 60.0;

  let avg_h = if avg_hrs > 0.0 {
    format!("{avg_hrs:.0}h ")
  } else {
    String::new()
  };
  let avg_s = if avg_sec > 0.0 {
    format!("{avg_sec:.0}s")
  } else {
    String::new()
  };
  let avg_m = if (avg_min > 0.0) || (avg_h.is_empty() && avg_s.is_empty()) {
    format!("{avg_min:.0}m ")
  } else {
    String::new()
  };

  Ok(ProcessedStats {
    days,
    total_h,
    total_m,
    total_s,
    avg_h,
    avg_m,
    avg_s,
  })
}
