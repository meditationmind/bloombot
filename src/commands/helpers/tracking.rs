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

#[derive(poise::ChoiceParameter)]
pub enum Privacy {
  #[name = "private"]
  Private,
  #[name = "public"]
  Public,
}

/// Takes the [`Privacy`][priv] choice parameter as an argument and returns `true` if the user
/// chose [`Privacy::Private`] or `false` if they chose [`Privacy::Public`].
///
/// [`Option<Privacy>`][priv] can also be passed as an argument, along with a default value,
/// specified via a second argument. If [`Option<Privacy>`][priv] is `Some`, the same matching
/// as above is applied. If `None`, the default value is returned. In most cases, the default
/// value should be taken from the user's [`TrackingProfile`][tp].
///
/// # Examples
///
/// ```rust
/// let privacy = Privacy::Private;
/// assert!(privacy!(private));
///
/// let privacy = None;
/// let profile = TrackingProfile::default().streaks_private(true);
/// assert!(privacy!(privacy, profile.streaks_private));
///
/// let privacy = Privacy::Public;
/// let profile = TrackingProfile::default().streaks_private(true);
/// assert!(!(privacy!(privacy, profile.streaks_private)));
/// ```
///
/// [priv]: self::Privacy
/// [tp]: crate::data::tracking_profile::TrackingProfile
macro_rules! privacy {
  ($privacy:expr, $default:expr) => {
    !(!(match $privacy {
      Some(privacy) => match privacy {
        Privacy::Private => true,
        Privacy::Public => false,
      },
      None => $default,
    }))
  };
  ($privacy:expr) => {
    match $privacy {
      Privacy::Private => true,
      Privacy::Public => false,
    }
  };
}
pub(crate) use privacy;

/// Queries the database for the total count of guild sessions and divides by 10. If there is no
/// remainder, the function queries the database for the guild total of minutes meditated, divides
/// this number by 60 to convert to hours, and returns this number. If the total count divided by
/// 10 produces a remainder, the function returns `None`. This works as a trigger to announce the
/// total minutes every 10th session added.
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

/// Announces the guild total of minutes meditated in the [`CHANNELS.tracking`][tracking] channel, using
/// the value returned by [`get_guild_hours`] as a trigger to announce every 10th session added.
///
/// [tracking]: crate::config::CHANNELS
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

/// Takes a `&str` and strips all asterisks (`*`), then escapes all other ASCII punctuation,
/// except for underscores (`_`) and tildes (`~`). For Discord markdown, this prevents italics
/// (or cancellation thereof) and all other markdown except for underline and strikethrough.
/// This is the desired behavior for quotes, which permit normal markdown when displayed using
/// [`quote`][quote], but are fully italicized when presented with an [`add`][add] or
/// [`import`][import] notification.
///
/// [add]: crate::commands::add::add()
/// [quote]: crate::commands::quote::quote()
/// [import]: crate::commands::import::import()
pub fn minimize_markdown(text: &str) -> String {
  text
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
    .collect::<String>()
}

/// Displays confirmation of time added via [`add`][add] or [`import`][import] and attempts to
/// include a random quote from the database. If a quote could not be fetched, the notification
/// is posted with the quote omitted.
///
/// When called from [`add`][add], the notification is formatted for use as a reply to the slash
/// command. When called from elsewhere ([`import`][import]), the notification is formatted for
/// independent posting, directly to a channel, e.g., [`CHANNELS.tracking`][tracking].
/// When `privacy` is set to `true`, notifications are anonymized.
///
/// [add]: crate::commands::add::add()
/// [import]: crate::commands::import::import()
/// [tracking]: crate::config::CHANNELS
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
    let quote = minimize_markdown(&random_quote.quote);

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

/// Gets a user's [`TimeSumRoles`] and checks to see whether a new role should be added.
/// If so, all previous [`TimeSumRoles`] are first removed, and if this fails, the user is
/// notified and the operation is aborted. Since the new role has not been added, the
/// removal will be attempted again on next add.
///
/// Once previous roles are removed, the new role is added and notification is sent as a
/// reply to the slash command ([`add`][add]), or in the case of [`import`][import], directly
/// to the [`CHANNELS.tracking`][tracking] channel or the originating DM. Notifications
/// honor privacy settings using ephemeral messages, based on the `privacy` argument.
///
/// [add]: crate::commands::add::add()
/// [import]: crate::commands::import::import()
/// [tracking]: crate::config::CHANNELS
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

/// Gets a user's [`StreakRoles`] and checks to see whether a new role should be added.
/// If so, all previous [`StreakRoles`] are first removed, and if this fails, the user is
/// notified and the operation is aborted. Since the new role has not been added, the
/// removal will be attempted again on next add.
///
/// Once previous roles are removed, the new role is added and notification is sent as a
/// reply to the slash command ([`add`][add]), or in the case of [`import`][import], directly
/// to the [`CHANNELS.tracking`][tracking] channel or the originating DM. Notifications
/// honor privacy settings using ephemeral messages, based on the `privacy` argument.
///
/// [add]: crate::commands::add::add()
/// [import]: crate::commands::import::import()
/// [tracking]: crate::config::CHANNELS
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

#[cfg(test)]
mod tests {
  use crate::data::tracking_profile::TrackingProfile;

  use super::*;

  #[test]
  fn test_privacy_macro() {
    let profile = TrackingProfile::default().streaks_private(true);
    assert!(privacy!(Privacy::Private));
    assert!(!(privacy!(Some(Privacy::Public), profile.streaks_private)));
    assert!(privacy!(None, profile.streaks_private));
  }

  #[test]
  fn test_minimize_markdown() {
    assert_eq!(
      minimize_markdown("A quote with *italics markdown* inside."),
      "A quote with italics markdown inside\\."
    );
    assert_eq!(
      minimize_markdown("A quote with __underline markdown__ inside."),
      "A quote with __underline markdown__ inside\\."
    );
    assert_eq!(
      minimize_markdown("A quote with ~~strikethrough markdown~~ inside."),
      "A quote with ~~strikethrough markdown~~ inside\\."
    );
    assert_eq!(
      minimize_markdown("A quote with a hyphen (-) and an em dash (—) inside."),
      "A quote with a hyphen \\(\\-\\) and an em dash \\(—\\) inside\\."
    );
    assert_eq!(
      minimize_markdown("A quote with single quotes ('') and double quotes (\"\") inside."),
      "A quote with single quotes \\(\\'\\'\\) and double quotes \\(\\\"\\\"\\) inside\\."
    );
  }
}
