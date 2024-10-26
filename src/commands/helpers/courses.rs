use crate::config::EMOJI;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

/// Checks the database for courses with names that meet a similarity threshold of 0.8
/// (high similarity) and returns either the course with the highest similarity or `None`.
///
/// If a possible course is returned, checks to see if the user is enrolled and suggests the
/// course if they are. If no course is returned or the user is not enrolled in the returned
/// course, user is informed that the specified course does not exist.
pub async fn course_not_found(
  ctx: Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: serenity::GuildId,
  course_name: String,
) -> Result<()> {
  let possible_course =
    DatabaseHandler::get_possible_course(transaction, &guild_id, course_name.as_str(), 0.8).await?;

  if let Some(possible_course) = possible_course {
    // Check if user is enrolled in possible_course
    if ctx
      .author()
      .has_role(ctx, guild_id, possible_course.participant_role)
      .await?
    {
      // Suggest possible_course if user is enrolled in the course
      ctx
        .send(
          poise::CreateReply::default()
            .content(format!(
              "{} Course does not exist. Did you mean `{}`?",
              EMOJI.mminfo, possible_course.name
            ))
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }
  }

  // If no possible_course is found or user is not enrolled in possible_course
  ctx
    .send(
      poise::CreateReply::default()
        .content(format!("{} Course does not exist.", EMOJI.mminfo))
        .ephemeral(true),
    )
    .await?;

  Ok(())
}
