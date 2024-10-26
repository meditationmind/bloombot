use anyhow::Result;
use log::error;
use poise::{
  serenity_prelude::{
    self as serenity, ChannelId, CreateAllowedMentions, CreateMessage, Mentionable,
  },
  CreateReply,
};

use crate::{
  config::{StreakRoles, TimeSumRoles, CHANNELS, EMOJI},
  database::DatabaseHandler,
  Context,
};

pub async fn get_guild_hours(
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: &serenity::GuildId,
) -> Result<Option<i64>> {
  let guild_count = DatabaseHandler::get_guild_meditation_count(transaction, guild_id).await?;
  if guild_count % 10 == 0 {
    let guild_sum = DatabaseHandler::get_guild_meditation_sum(transaction, guild_id).await?;
    Ok(Some(guild_sum / 60))
  } else {
    Ok(None)
  }
}

pub async fn post_guild_hours(ctx: &Context<'_>, guild_hours: &Option<i64>) -> Result<()> {
  if let Some(guild_hours) = guild_hours {
    if ctx.channel_id() == CHANNELS.tracking {
      ctx.say(format!("Awesome sauce! This server has collectively generated {guild_hours} hours of realmbreaking meditation!")).await?;
    } else {
      ChannelId::new(CHANNELS.tracking).send_message(&ctx,CreateMessage::new().content(format!("Awesome sauce! This server has collectively generated {guild_hours} hours of realmbreaking meditation!")).allowed_mentions(CreateAllowedMentions::new()),).await?;
    }
  }
  Ok(())
}

pub async fn show_add_with_quote(
  ctx: &Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: &serenity::GuildId,
  user_id: &serenity::UserId,
  minutes: &i32,
  user_sum: &i64,
  privacy: bool,
) -> Result<String> {
  let random_quote = DatabaseHandler::get_random_quote(transaction, guild_id).await?;

  if let Some(random_quote) = random_quote {
    let quote = random_quote
      .quote
      .chars()
      .filter(|c| !matches!(c, '*'))
      .map(|c| {
        if c.is_ascii_punctuation() {
          if matches!(c, '_' | '~') {
            c.to_string()
          } else {
            format!("\\{c}")
          }
        } else {
          c.to_string()
        }
      })
      .collect::<String>();

    if privacy {
      Ok(format!(
        "Someone just added **{minutes} minutes** to their meditation time! :tada:\n*{quote}*"
      ))
    } else if ctx.command().name == "add" {
      Ok(format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:\n*{quote}*"))
    } else {
      Ok(format!("<@{user_id}> added **{minutes} minutes** to their meditation time! Their total meditation time is now {user_sum} minutes :tada:\n*{quote}*"))
    }
  } else if privacy {
    Ok(format!(
      "Someone just added **{minutes} minutes** to their meditation time! :tada:"
    ))
  } else if ctx.command().name == "add" {
    Ok(format!("Added **{minutes} minutes** to your meditation time! Your total meditation time is now {user_sum} minutes :tada:"))
  } else {
    Ok(format!("<@{user_id}> added **{minutes} minutes** to their meditation time! Their total meditation time is now {user_sum} minutes :tada:"))
  }
}

pub async fn update_time_roles(
  ctx: &Context<'_>,
  member: &serenity::Member,
  sum: i64,
  privacy: bool,
) -> Result<()> {
  let current_time_roles = TimeSumRoles::get_users_current_roles(&member.roles);
  let updated_time_role = TimeSumRoles::from_sum(sum);

  if let Some(updated_time_role) = updated_time_role {
    if !current_time_roles.contains(&updated_time_role.to_role_id()) {
      for role in current_time_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");
            ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_time_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");
          ctx.send(CreateReply::default()
            .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true)).await?;

          return Ok(());
        }
      }

      if ctx.command().name == "add" {
        ctx.send(CreateReply::default()
        .content(format!(":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the <@&{}> role!", member.mention(), updated_time_role.to_role_id()))
        .allowed_mentions(serenity::CreateAllowedMentions::new())
        .ephemeral(privacy)).await?;
      } else {
        let congrats = if ctx.guild_id().is_none() && privacy {
          format!(
          ":tada: Congrats {}, your hard work is paying off! Your total meditation minutes have given you the @{} role!",
          member.mention(),
          updated_time_role.to_role_icon()
          )
        } else {
          format!(
          ":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the <@&{}> role!",
          member.mention(),
          updated_time_role.to_role_id()
          )
        };

        if privacy {
          ctx
            .send(
              CreateReply::default()
                .content(congrats)
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(privacy),
            )
            .await?;
        } else {
          ChannelId::new(CHANNELS.tracking)
            .send_message(
              &ctx,
              CreateMessage::new()
                .content(congrats)
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
            )
            .await?;
        }
      }
    }
  }

  Ok(())
}

pub async fn update_streak_roles(
  ctx: &Context<'_>,
  member: &serenity::Member,
  streak: i32,
  privacy: bool,
) -> Result<()> {
  let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);
  #[allow(clippy::cast_sign_loss)]
  let updated_streak_role = StreakRoles::from_streak(streak as u64);

  if let Some(updated_streak_role) = updated_streak_role {
    if !current_streak_roles.contains(&updated_streak_role.to_role_id()) {
      for role in current_streak_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");

            ctx.send(CreateReply::default()
                .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_streak_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");

          ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

          return Ok(());
        }
      }

      if ctx.command().name == "add" {
        ctx.send(CreateReply::default()
          .content(format!(":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the <@&{}> role!", member.mention(), streak, updated_streak_role.to_role_id()))
          .allowed_mentions(serenity::CreateAllowedMentions::new())
          .ephemeral(privacy)).await?;
      } else {
        let congrats = if ctx.guild_id().is_none() && privacy {
          format!(
          ":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the @{} role!",
          member.mention(),
          streak,
          updated_streak_role.to_role_icon()
        )
        } else {
          format!(
          ":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the <@&{}> role!",
          member.mention(),
          streak,
          updated_streak_role.to_role_id()
        )
        };

        if privacy {
          ctx
            .send(
              CreateReply::default()
                .content(congrats)
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(privacy),
            )
            .await?;
        } else {
          ChannelId::new(CHANNELS.tracking)
            .send_message(
              &ctx,
              CreateMessage::new()
                .content(congrats)
                .allowed_mentions(serenity::CreateAllowedMentions::new()),
            )
            .await?;
        }
      }
    }
  }

  Ok(())
}
