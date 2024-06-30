use crate::config::ROLES;
use crate::Context;
use anyhow::Result;
use chrono;
use poise::CreateReply;

#[derive(poise::ChoiceParameter)]
pub enum ChallengeChoices {
  #[name = "Monthly Challenge"]
  Monthly,
  #[name = "365-Day Challenge"]
  YearRound,
}

/// Join or leave a meditation challenge
///
/// Join or leave the monthly or 365-day meditation challenge.
#[poise::command(
  slash_command,
  category = "Meditation Tracking",
  subcommands("join", "leave"),
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
pub async fn join(
  ctx: Context<'_>,
  #[description = "Challenge you wish to join (Defaults to monthly)"] challenge: Option<
    ChallengeChoices,
  >,
) -> Result<()> {
  let guild_id = ctx.guild_id().unwrap();
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

        ctx.say(format!(
    "Awesome, <@{}>! You have successfully joined the 365-day challenge <:pepeglow:1174181400249901076>",
    member.user.id,
    )).await?;

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
pub async fn leave(
  ctx: Context<'_>,
  #[description = "Challenge you wish to leave (Defaults to monthly)"] challenge: Option<
    ChallengeChoices,
  >,
) -> Result<()> {
  let guild_id = ctx.guild_id().unwrap();
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
