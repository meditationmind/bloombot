use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::CreateReply;
use poise::serenity_prelude::{GuildId, Role};

use crate::Context;
use crate::commands::helpers::common::Visibility;
use crate::commands::helpers::courses;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{EMOJI, ENTRIES_PER_PAGE};
use crate::data::course::Course;
use crate::database::DatabaseHandler;

/// Commands for managing courses
///
/// Commands to add, edit, list, or remove courses.
///
/// Requires `Administrator` permissions.
#[poise::command(
  slash_command,
  required_permissions = "ADMINISTRATOR",
  default_member_permissions = "ADMINISTRATOR",
  category = "Admin Commands",
  subcommands("add", "remove", "edit", "list"),
  subcommand_required,
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn courses(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Add a course and its associated graduate role to the database
///
/// Adds a course and its associated graduate role to the database.
#[poise::command(slash_command)]
async fn add(
  ctx: Context<'_>,
  #[description = "Name of the course"] course_name: String,
  #[description = "Role course participants are assumed to have"] participant_role: Role,
  #[description = "Role to be given to graduates"] graduate_role: Role,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if DatabaseHandler::course_exists(&mut transaction, &guild_id, course_name.as_str()).await? {
    ctx
      .say(format!("{} Course already exists.", EMOJI.mminfo))
      .await?;
    return Ok(());
  }

  if let Err(e) = check_eligibility(guild_id, &participant_role, &graduate_role) {
    let msg = format!("{} {e}", EMOJI.mminfo);
    ctx.say(msg).await?;
    return Ok(());
  }

  let course = Course::new(course_name, participant_role.id, graduate_role.id, guild_id);

  DatabaseHandler::add_course(&mut transaction, &course).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Course has been added.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// Update the roles for an existing course
///
/// Updates the roles for an existing course.
#[poise::command(slash_command)]
async fn edit(
  ctx: Context<'_>,
  #[description = "Name of the course"] course_name: String,
  #[description = "Role course participants are assumed to have"] participant_role: Option<Role>,
  #[description = "Role to be given to graduates"] graduate_role: Option<Role>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  if participant_role.is_none() && graduate_role.is_none() {
    let msg = format!("{} No changes were provided.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let Some(course) =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?
  else {
    courses::course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  };

  let no_changes = (participant_role.is_none()
    || participant_role
      .as_ref()
      .is_some_and(|role| role.id == course.participant_role))
    && (graduate_role.is_none()
      || graduate_role
        .as_ref()
        .is_some_and(|role| role.id == course.graduate_role));

  if no_changes {
    let msg = format!("{} No changes were provided.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let participant_role =
    participant_role.unwrap_or(guild_id.role(ctx, course.participant_role).await?);
  let graduate_role = graduate_role.unwrap_or(guild_id.role(ctx, course.graduate_role).await?);

  if let Err(e) = check_eligibility(guild_id, &participant_role, &graduate_role) {
    let msg = format!("{} {e}", EMOJI.mminfo);
    ctx.say(msg).await?;
    return Ok(());
  }

  let course = Course::new(course_name, participant_role.id, graduate_role.id, guild_id);

  DatabaseHandler::update_course(&mut transaction, &course).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Course roles have been updated.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

/// List all courses
///
/// Lists all courses in the database.
#[poise::command(slash_command)]
async fn list(
  ctx: Context<'_>,
  #[description = "The page to show"] page: Option<usize>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let courses = DatabaseHandler::get_all_courses(&mut transaction, &guild_id).await?;
  let courses: Vec<PageRowRef> = courses.iter().map(|course| course as PageRowRef).collect();

  drop(transaction);

  Paginator::new("Courses", &courses, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, Visibility::Ephemeral)
    .await?;

  Ok(())
}

/// Remove a course from the database
///
/// Removes a course from the database.
#[poise::command(slash_command)]
async fn remove(
  ctx: Context<'_>,
  #[description = "Name of the course"] course_name: String,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  if !DatabaseHandler::course_exists(&mut transaction, &guild_id, course_name.as_str()).await? {
    ctx
      .say(format!("{} Course does not exist.", EMOJI.mminfo))
      .await?;
    return Ok(());
  }

  DatabaseHandler::remove_course(&mut transaction, &guild_id, course_name.as_str()).await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Course has been removed.", EMOJI.mmcheck)),
    Visibility::Ephemeral,
  )
  .await?;

  Ok(())
}

fn check_eligibility(
  guild_id: GuildId,
  participant_role: &Role,
  graduate_role: &Role,
) -> Result<bool> {
  // Verify that the roles are in the guild.
  if participant_role.guild_id.ne(&guild_id) {
    return Err(anyhow!(
      "Participant role must belong to the same guild as the command."
    ));
  }
  if graduate_role.guild_id.ne(&guild_id) {
    return Err(anyhow!(
      "Graduate role must belong to the same guild as the command."
    ));
  }
  // Verify that the roles are not managed by an integration.
  if participant_role.managed {
    return Err(anyhow!("Participant role cannot be a bot role."));
  }
  if graduate_role.managed {
    return Err(anyhow!("Graduate role cannot be a bot role."));
  }
  // Verify that the roles are not privileged.
  if participant_role.permissions.administrator() {
    return Err(anyhow!("Participant role cannot be an administrator role."));
  }
  if graduate_role.permissions.administrator() {
    return Err(anyhow!("Graduate role cannot be an administrator role."));
  }
  // Verify that roles do not match.
  if participant_role == graduate_role {
    return Err(anyhow!(
      "Participant and graduate roles must not be the same."
    ));
  }

  Ok(true)
}
