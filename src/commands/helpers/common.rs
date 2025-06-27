use anyhow::Result;
use poise::CreateReply;
use poise::serenity_prelude::{CreateAllowedMentions, GuildId, Http};
use tracing::debug;

use crate::Context;
use crate::config::{EMOJI, ROLES, Role};
use crate::data::tracking_profile::Privacy;

pub enum Visibility {
  Public,
  Ephemeral,
}

impl From<Privacy> for Visibility {
  /// Converts [`Privacy`] into [`Visibility`], with [`Privacy::Private`]
  /// implying [`Visibility::Ephemeral`].
  fn from(privacy: Privacy) -> Self {
    match privacy {
      Privacy::Private => Self::Ephemeral,
      Privacy::Public => Self::Public,
    }
  }
}

/// Takes [`Context`] as an argument and attempts to retrieve the author of the invoking
/// interaction as a [`serenity::Member`] via [`author_member()`][am]. If successful, checks
/// the member's roles and returns `true` if they include a valid supporter role, as defined
/// in [`ROLES`]. Returns `false` if retrieval is unsuccessful or no valid roles are found.
///
/// Valid roles include:
/// - [`ROLES.patreon`][roles]
/// - [`ROLES.kofi`][roles]
/// - [`ROLES.staff`][roles]
///
/// [am]: poise::structs::Context::author_member()
/// [roles]: crate::config::ROLES
pub async fn is_supporter(ctx: Context<'_>) -> bool {
  ctx.author_member().await.is_some_and(|member| {
    member.roles.contains(&ROLES.patreon.into())
      || member.roles.contains(&ROLES.kofi.into())
      || member.roles.contains(&ROLES.staff.into())
  })
}

/// Attempts to retrieve the author of the invoking interaction as a [`serenity::Member`]
/// via [`author_member()`][am]. If successful, checks the member's roles and returns `true`
/// if they include the specified [`Role`] from [`ROLES`]. Returns `false` if retrieval is
/// unsuccessful or member does not have the role.
///
/// [am]: poise::structs::Context::author_member()
pub async fn has_role(ctx: Context<'_>, role: Role) -> bool {
  ctx
    .author_member()
    .await
    .is_some_and(|member| member.roles.contains(&role.into()))
}

/// For use in command checks. Calls [`has_role`] to check if the invoking user has the
/// specified [`Role`] from [`ROLES`]. If [`has_role`] returns `false`, notifies user that
/// they do not have the required role and returns `false` to abort the command.
pub async fn role_check(ctx: Context<'_>, role: Role) -> Result<bool> {
  let has_role = has_role(ctx, role).await;
  if !has_role {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} This command requires the {role} role.",
            EMOJI.mminfo
          ))
          .allowed_mentions(CreateAllowedMentions::new().empty_roles())
          .ephemeral(true),
      )
      .await?;
  }
  Ok(has_role)
}

pub async fn print_command(
  http: impl AsRef<Http>,
  guild_id: GuildId,
  command_name: &str,
) -> String {
  let mut result = format!("`/{command_name}`");
  if let Ok(commands) = guild_id.get_commands(http).await {
    let parent = command_name
      .split_whitespace()
      .next()
      .unwrap_or(command_name);
    for command in commands {
      if command.name == parent {
        result = format!("</{}:{}>", command_name, command.id.get());
        break;
      }
    }
  }
  debug!("{result:?}");
  result
}
