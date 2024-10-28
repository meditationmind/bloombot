use anyhow::Result;
use poise::serenity_prelude::RoleId;

use crate::{config::ROLES, Context};

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
pub async fn is_supporter(ctx: Context<'_>) -> Result<bool> {
  let supporter = if let Some(member) = ctx.author_member().await {
    member.roles.contains(&RoleId::from(ROLES.patreon))
      || member.roles.contains(&RoleId::from(ROLES.kofi))
      || member.roles.contains(&RoleId::from(ROLES.staff))
  } else {
    false
  };
  Ok(supporter)
}
