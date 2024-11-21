use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::User;

use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::database::{self, MessageType};
use crate::data::tracking_profile::Privacy;
use crate::database::DatabaseHandler;
use crate::{config::ROLES, Context};

/// See your current meditation streak
///
/// Shows your current meditation streak. Setting the visibility here will override your custom streak privacy settings.
///
/// Can also be used to check another member's streak, unless set to private.
#[poise::command(slash_command, category = "Meditation Tracking", guild_only)]
pub async fn streak(
  ctx: Context<'_>,
  #[description = "The user to check the streak of"] user: Option<User>,
  #[description = "Set visibility of response (Defaults to public)"] privacy: Option<Privacy>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;
  //let user_id = user.as_ref().map_or(ctx.author().id, |user| user.id);
  let user = user.as_ref().unwrap_or(ctx.author());

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user.id).await?;

  let tracking_profile =
    DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user.id)
      .await?
      .unwrap_or_default();

  let visibility = privacy.unwrap_or(tracking_profile.streak.privacy).into();

  if user.id == ctx.author().id {
    let message = if streak.current == streak.longest {
      format!(
        "Your current meditation streak is {} days. This is your longest streak.",
        streak.current
      )
    } else {
      format!(
        "Your current meditation streak is {} days. Your longest streak is {} days.",
        streak.current, streak.longest
      )
    };

    database::commit_and_say(ctx, transaction, MessageType::TextOnly(message), visibility).await?;

    return Ok(());
  }

  let user_nick_or_name = user
    .nick_in(&ctx, guild_id)
    .await
    .unwrap_or_else(|| user.global_name.as_ref().unwrap_or(&user.name).to_owned());

  let staff = ctx.author().has_role(&ctx, guild_id, ROLES.staff).await?;

  let (message, visibility) = match tracking_profile.streak.privacy {
    //Show for staff even when private
    Privacy::Private if staff && streak.current == streak.longest => (
      format!(
        "{user_nick_or_name}'s current **private** meditation streak is {} days. This is {user_nick_or_name}'s longest streak.",
        streak.current
      ),
      Visibility::Ephemeral,
    ),
    Privacy::Private if staff => (
      format!(
        "{user_nick_or_name}'s current **private** meditation streak is {} days. {user_nick_or_name}'s longest streak is {} days.",
        streak.current, streak.longest
      ),
      Visibility::Ephemeral,
    ),
    Privacy::Private => (
      format!("Sorry, {user_nick_or_name}'s meditation streak is set to private."),
      Visibility::Ephemeral,
    ),
    Privacy::Public if streak.current == streak.longest => (
      format!(
        "{user_nick_or_name}'s current meditation streak is {} days. This is {user_nick_or_name}'s longest streak.",
        streak.current
      ),
      visibility,
    ),
    Privacy::Public => (
      format!(
        "{user_nick_or_name}'s current meditation streak is {} days. {user_nick_or_name}'s longest streak is {} days.",
        streak.current, streak.longest
      ),
      visibility,
    ),
  };

  database::commit_and_say(ctx, transaction, MessageType::TextOnly(message), visibility).await?;

  Ok(())
}
