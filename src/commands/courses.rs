use crate::commands::helpers::courses;
use crate::commands::helpers::database::{self, MessageType};
use crate::commands::helpers::pagination::{PageRowRef, PageType, Paginator};
use crate::config::{EMOJI, ENTRIES_PER_PAGE};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude as serenity;
use poise::CreateReply;

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
  //hide_in_help,
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
  #[description = "The role participants of the course are assumed to have"]
  participant_role: serenity::Role,
  #[description = "Role to be given to graduates"] graduate_role: serenity::Role,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  if DatabaseHandler::course_exists(&mut transaction, &guild_id, course_name.as_str()).await? {
    ctx
      .say(format!("{} Course already exists.", EMOJI.mminfo))
      .await?;
    return Ok(());
  }

  // Verify that the roles are in the guild
  if !participant_role.guild_id.eq(&guild_id) {
    ctx
      .say(format!(
        "{} The participant role must be in the same guild as the command.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }
  if !graduate_role.guild_id.eq(&guild_id) {
    ctx
      .say(format!(
        "{} The graduate role must be in the same guild as the command.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }

  // Verify that the roles are not managed by an integration
  if participant_role.managed {
    ctx
      .say(format!(
        "{} The participant role must not be a bot role.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }
  if graduate_role.managed {
    ctx
      .say(format!(
        "{} The graduate role must not be a bot role.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }

  // Verify that the roles are not privileged
  if participant_role.permissions.administrator() {
    ctx
      .say(format!(
        "{} The participant role must not be an administrator role.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }
  if graduate_role.permissions.administrator() {
    ctx
      .say(format!(
        "{} The graduate role must not be an administrator role.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }

  if participant_role == graduate_role {
    ctx
      .say(format!(
        "{} The participant role and the graduate role must not be the same.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }

  DatabaseHandler::add_course(
    &mut transaction,
    &guild_id,
    course_name.as_str(),
    &participant_role,
    &graduate_role,
  )
  .await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Course has been added.", EMOJI.mmcheck)),
    true,
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
  #[description = "Update the role that participants of the course are assumed to have"]
  participant_role: Option<serenity::Role>,
  #[description = "Update the role that graduates of the course are given"] graduate_role: Option<
    serenity::Role,
  >,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  if participant_role.is_none() && graduate_role.is_none() {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} No changes were provided.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let course =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?;

  // Verify that the course exists
  if course.is_none() {
    courses::course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  }

  let course = course.with_context(|| "Failed to assign CourseData to course")?;

  let participant_role = match participant_role {
    Some(participant_role) => {
      if !participant_role.guild_id.eq(&guild_id) {
        ctx
          .say(format!(
            "{} The participant role must be in the same guild as the command.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      if participant_role.managed {
        ctx
          .say(format!(
            "{} The participant role must not be a bot role.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      if participant_role.permissions.administrator() {
        ctx
          .say(format!(
            "{} The participant role must not be an administrator role.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      participant_role.id.to_string()
    }
    None => course.participant_role.to_string(),
  };

  let graduate_role = match graduate_role {
    Some(graduate_role) => {
      if !graduate_role.guild_id.eq(&guild_id) {
        ctx
          .say(format!(
            "{} The graduate role must be in the same guild as the command.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      if graduate_role.managed {
        ctx
          .say(format!(
            "{} The graduate role must not be a bot role.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      if graduate_role.permissions.administrator() {
        ctx
          .say(format!(
            "{} The graduate role must not be an administrator role.",
            EMOJI.mminfo
          ))
          .await?;
        return Ok(());
      }
      graduate_role.id.to_string()
    }
    None => course.graduate_role.to_string(),
  };

  // Verify that the roles are not the same
  if participant_role == graduate_role {
    ctx
      .say(format!(
        "{} The participant role and the graduate role must not be the same.",
        EMOJI.mminfo
      ))
      .await?;
    return Ok(());
  }

  DatabaseHandler::update_course(
    &mut transaction,
    course_name.as_str(),
    participant_role,
    graduate_role,
  )
  .await?;

  database::commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(format!("{} Course roles have been updated.", EMOJI.mmcheck)),
    true,
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
  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let courses = DatabaseHandler::get_all_courses(&mut transaction, &guild_id).await?;
  let courses: Vec<PageRowRef> = courses.iter().map(|course| course as _).collect();

  drop(transaction);

  Paginator::new("Courses", &courses, ENTRIES_PER_PAGE.default)
    .paginate(ctx, page, PageType::Standard, true)
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

  let data = ctx.data();

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
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
    true,
  )
  .await?;

  Ok(())
}
