use crate::commands::course_not_found;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::CreateReply;

/// Manage your course enrollments
///
/// Join or leave a Meditation Mind course.
#[poise::command(
  slash_command,
  category = "Utilities",
  subcommands("join", "leave"),
  guild_only
)]
#[allow(clippy::unused_async)]
pub async fn course(_: Context<'_>) -> Result<()> {
  Ok(())
}

/// Join a course
///
/// Join a Meditation Mind course.
#[poise::command(slash_command)]
pub async fn join(
  ctx: Context<'_>,
  #[description = "Course you wish to join"]
  #[rename = "course"]
  course_name: Option<String>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  // Default to Mindfulness Course since it's the only course currently
  let course_name = if let Some(course_name) = course_name {
    course_name
  } else {
    "Mindfulness Course".to_owned()
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let course =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?;

  // Verify that the course exists
  if course.is_none() {
    course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  }

  let course = course.with_context(|| "Failed to assign CourseData to course")?;
  let member = ctx
    .author_member()
    .await
    .with_context(|| "Failed to retrieve Member from context, cache, or HTTP")?;

  if member.roles.contains(&course.participant_role) {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} You are already enrolled in the course: **{course_name}**.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  if member.roles.contains(&course.graduate_role) {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} You have already completed the course: **{course_name}**.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  member.add_role(ctx, course.participant_role).await?;

  // Add course-specific embeds when more courses are added
  let embed = if course_name == "Mindfulness Course" {
    BloomBotEmbed::new()
    .title("Thank you for joining the Mindfulness Course!")
    .description("You have two options for accessing the course materials.\n\n- If you prefer the Discord environment, you can access the full course directly from the Meditation Mind server: <#1257709248847155240>\n- If you prefer an online course platform, you may enroll and begin your journey on Thinkific: [Getting Started with Mindfulness](<https://meditation-mind.thinkific.com/courses/mindfulness>)\n\nIf you decide you would like to opt out of the course-specific channels at a later time, just use the `/course leave` command.\n\nWe hope you find the course helpful!")
    .image("https://meditationmind.org/wp-content/uploads/2022/01/Meditation_CHallenge_kopie.png")
  } else {
    BloomBotEmbed::new()
  };

  ctx
    .send(CreateReply {
      embeds: vec![embed],
      ephemeral: Some(true),
      ..Default::default()
    })
    .await?;

  // Log enrollment in staff channel
  let log_embed = BloomBotEmbed::new()
    .title("New Course Enrollment")
    .description(format!(
      "<@{}> has opted into the course: **{course_name}**",
      ctx.author().id
    ));

  let log_channel = serenity::ChannelId::new(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}

/// Leave a course
///
/// Leave a Meditation Mind course.
#[poise::command(slash_command)]
pub async fn leave(
  ctx: Context<'_>,
  #[description = "Course you wish to leave"]
  #[rename = "course"]
  course_name: Option<String>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let data = ctx.data();
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  // Default to Mindfulness Course since it's the only course currently
  let course_name = if let Some(course_name) = course_name {
    course_name
  } else {
    "Mindfulness Course".to_owned()
  };

  let mut transaction = data.db.start_transaction_with_retry(5).await?;
  let course =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?;

  // Verify that the course exists
  if course.is_none() {
    course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  }

  let course = course.with_context(|| "Failed to assign CourseData to course")?;
  let member = ctx
    .author_member()
    .await
    .with_context(|| "Failed to retrieve Member from context, cache, or HTTP")?;

  if !member.roles.contains(&course.participant_role) {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} You are not currently enrolled in the course: **{course_name}**.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  member.remove_role(ctx, course.participant_role).await?;

  // Adjust when new courses are added
  ctx
    .send(
      CreateReply::default()
        .content(format!("You have successfully opted out of the **{course_name}** course-specific channels.\n\nIf you also enrolled on Thinkific and would like to be unenrolled there, please send a DM to <@575252669443211264> or email us at `info@meditationmind.org` and let us know your Thinkific username or the email address you used to sign up.\n\nWe wish you all the best on your journey!"))
        .ephemeral(true),
    )
    .await?;

  // Log withdrawal in staff channel
  let log_embed = BloomBotEmbed::new()
    .title("Course Withdrawal")
    .description(format!(
      "<@{}> has opted out of the course: **{course_name}**",
      ctx.author().id
    ));

  let log_channel = serenity::ChannelId::new(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}
