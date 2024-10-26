use crate::config::EMOJI;
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use poise::serenity_prelude as serenity;

pub async fn course_not_found(
  ctx: Context<'_>,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  guild_id: serenity::GuildId,
  course_name: String,
) -> Result<()> {
  let possible_course =
    DatabaseHandler::get_possible_course(transaction, &guild_id, course_name.as_str(), 0.8).await?;

  if let Some(possible_course) = possible_course {
    // Check if user is in the course
    if ctx
      .author()
      .has_role(ctx, guild_id, possible_course.participant_role)
      .await?
    {
      ctx
        .send(
          poise::CreateReply::default()
            .content(format!(
              "{} Course does not exist. Did you mean `{}`?",
              EMOJI.mminfo, possible_course.course_name
            ))
            .ephemeral(true),
        )
        .await?;
    } else {
      ctx
        .send(
          poise::CreateReply::default()
            .content(format!("{} Course does not exist.", EMOJI.mminfo))
            .ephemeral(true),
        )
        .await?;
    }
  } else {
    ctx
      .send(
        poise::CreateReply::default()
          .content(format!("{} Course does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
  }

  Ok(())
}
