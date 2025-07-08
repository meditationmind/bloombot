use anyhow::{Context as AnyhowContext, Result, anyhow};
use poise::CreateReply;
use poise::serenity_prelude::{ChannelId, CreateMessage};

use crate::Context;
use crate::commands::helpers::courses;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI};
use crate::database::DatabaseHandler;

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
async fn join(
  ctx: Context<'_>,
  #[description = "Course you wish to join"]
  #[rename = "course"]
  course_name: Option<String>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  // Default to Mindfulness Course since it's the only course currently
  let course_name = course_name.unwrap_or("Mindfulness Course".to_owned());

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let Some(course) =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?
  else {
    courses::course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  };

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

  if let Err(e) = member.add_role(ctx, course.participant_role).await {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} Failed to add the course role. Please try again or contact staff for assistance.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Err(anyhow!("Failed to add course role: {e}"));
  }

  // Add course-specific embeds when more courses are added
  let embed = if course_name == "Mindfulness Course" {
    BloomBotEmbed::new()
      .title("Thank you for joining the Mindfulness Course!")
      .description(
        "You have two options for accessing the course materials.\
        \n\n\
        - If you prefer the Discord environment, you can access the full course directly from \
        the Meditation Mind server: <#1257709248847155240>\n- If you prefer an online course \
        platform, you may enroll and begin your journey on Thinkific: [Getting Started with \
        Mindfulness](<https://meditation-mind.thinkific.com/courses/mindfulness>)\
        \n\n\
        If you decide you would like to opt out of the course-specific channels at a later time, \
        just use the `/course leave` command.\n\nWe hope you find the course helpful!",
      )
      .image("https://meditationmind.org/wp-content/uploads/2022/01/Meditation_CHallenge_kopie.png")
  } else {
    BloomBotEmbed::new().description("How did you get here?")
  };

  ctx
    .send(CreateReply::default().embed(embed).ephemeral(true))
    .await?;

  // Log enrollment in staff channel
  let log_embed = BloomBotEmbed::new()
    .title("New Course Enrollment")
    .description(format!(
      "<@{}> has opted into the course: **{course_name}**",
      ctx.author().id
    ));

  let log_channel = ChannelId::from(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}

/// Leave a course
///
/// Leave a Meditation Mind course.
#[poise::command(slash_command)]
async fn leave(
  ctx: Context<'_>,
  #[description = "Course you wish to leave"]
  #[rename = "course"]
  course_name: Option<String>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  // Default to Mindfulness Course since it's the only course currently
  let course_name = course_name.unwrap_or("Mindfulness Course".to_owned());

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;
  let Some(course) =
    DatabaseHandler::get_course(&mut transaction, &guild_id, course_name.as_str()).await?
  else {
    courses::course_not_found(ctx, &mut transaction, guild_id, course_name).await?;
    return Ok(());
  };

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

  if let Err(e) = member.remove_role(ctx, course.participant_role).await {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} Failed to remove the course role. Please try again or contact staff for assistance.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
    return Err(anyhow!("Failed to remove course role: {e}"));
  }

  // Adjust when new courses are added
  ctx
    .send(
      CreateReply::default()
        .content(format!(
          "You have successfully opted out of the **{course_name}** course-specific channels.\
          \n\n\
          If you also enrolled on Thinkific and would like to be unenrolled there, please submit \
          a ticket (`/new`) or email us at `info@meditationmind.org` and let us know \
          your Thinkific username or the email address you used to sign up.\
          \n\n\
          We wish you all the best on your journey!"
        ))
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

  let log_channel = ChannelId::from(CHANNELS.logs);

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}
