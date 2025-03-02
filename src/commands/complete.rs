use anyhow::Result;
use poise::serenity_prelude::{ChannelId, CreateMessage};

use crate::Context;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::database::DatabaseHandler;

/// Indicate that you have completed a course
///
/// Indicates that you have completed a course.
///
/// Marks the specified course as complete, removing the participant role and awarding the graduate role for that course.
#[poise::command(
  slash_command,
  category = "Secret",
  rename = "coursecomplete",
  hide_in_help,
  dm_only
)]
pub async fn complete(
  ctx: Context<'_>,
  #[description = "The course you have completed"] course_name: String,
) -> Result<()> {
  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  let Some(course) =
    DatabaseHandler::get_course_in_dm(&mut transaction, course_name.as_str()).await?
  else {
    let msg = format!(
      "{} Course not found. Please contact server staff for assistance.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  };

  let guild_id = course.guild_id;
  let participant_role = course.participant_role;
  let graduate_role = course.graduate_role;

  if guild_id.to_guild_cached(&ctx).is_none() {
    let msg = format!(
      "{} Can't retrieve server information. Please contact server staff for assistance.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  }

  let Ok(member) = guild_id.member(ctx, ctx.author().id).await else {
    let msg = format!(
      "{} You don't appear to be a member of the server. If I'm mistaken, please contact server staff for assistance.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  };

  if !member
    .user
    .has_role(ctx, guild_id, participant_role)
    .await?
  {
    let msg = format!(
      "{} You are not enrolled in the course: **{course_name}**.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  }

  if member.user.has_role(ctx, guild_id, graduate_role).await? {
    let msg = format!(
      "{} You have already claimed the graduate role for course: **{course_name}**.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  }

  if member.add_role(ctx, graduate_role).await.is_err() {
    let msg = format!(
      "{} An error occurred while attempting to add the graduate role. Please contact server staff for assistance.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
    return Ok(());
  };

  if let Ok(()) = member.remove_role(ctx, participant_role).await {
    let msg = format!(":tada: Congrats! You are now a graduate of the course: **{course_name}**!");
    ctx.say(msg).await?;
  } else {
    let msg = format!(
      ":tada: Congrats! You are now a graduate of the course: **{course_name}**!\n\n{} An error occurred while attempting to remove the participant role. Please contact server staff to have it manually removed. Your graduate role is unaffected.",
      EMOJI.mminfo
    );
    ctx.say(msg).await?;
  };

  // Log completion in staff logs.
  let log_embed = BloomBotEmbed::new()
    .title("New Course Graduate")
    .description(format!("**User**: {member}\n**Course**: {course_name}"));

  let log_channel = ChannelId::new(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}
