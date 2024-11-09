use anyhow::Result;
use poise::serenity_prelude::GuildId;
use poise::CreateReply;
use sqlx::{Postgres, Transaction};

use crate::config::EMOJI;
use crate::database::DatabaseHandler;
use crate::Context;

/// Checks the database for courses with names that meet a similarity threshold of 0.8
/// (high similarity) and returns either the course with the highest similarity or `None`.
pub async fn course_not_found(
  ctx: Context<'_>,
  transaction: &mut Transaction<'_, Postgres>,
  guild_id: GuildId,
  course_name: String,
) -> Result<()> {
  let Some(possible_course) =
    DatabaseHandler::get_possible_course(transaction, &guild_id, course_name.as_str(), 0.8).await?
  else {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} Course does not exist.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "{} Course does not exist. Did you mean `{}`?",
          EMOJI.mminfo, possible_course.name
        ))
        .ephemeral(true),
    )
    .await?;

  Ok(())
}
