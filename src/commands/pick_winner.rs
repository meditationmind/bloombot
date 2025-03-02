use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result};
use chrono::Months as ChronoMonths;
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use futures::StreamExt;
use poise::serenity_prelude::{ButtonStyle, ChannelType, GuildId, builder::*};
use poise::serenity_prelude::{ChannelId, ComponentInteractionCollector, Member, RoleId};
use poise::{ChoiceParameter, CreateReply};

use crate::Context;
use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ROLES};
use crate::database::DatabaseHandler;

#[derive(Debug, Clone, Copy, ChoiceParameter)]
enum Months {
  January,
  February,
  March,
  April,
  May,
  June,
  July,
  August,
  September,
  October,
  November,
  December,
}

async fn finalize_winner(
  ctx: Context<'_>,
  guild_id: GuildId,
  winner: Member,
  minutes: i64,
  selected_date: DateTime<Utc>,
  reserved_key: String,
) -> Result<()> {
  let now = Utc::now();
  let guild_name = guild_id.name(ctx).unwrap_or("Host Server".to_owned());

  let announcement_embed = BloomBotEmbed::new()
    .title(":tada: Monthly Challenge Winner :tada:")
    .description(format!(
      "**Meditator in the Spotlight for {}**\nCongratulations to **{}** on winning our {} challenge, with a meditation time of **{}** minutes for the month!",
      selected_date.format("%B"),
      winner.user,
      selected_date.format("%B"),
      minutes
    ))
    .thumbnail(winner.user.avatar_url().unwrap_or_default())
    .footer(CreateEmbedFooter::new(format!(
      "Meditation Challenge for {} | Selected on {}",
      selected_date.format("%B %Y"),
      now.format("%B %d, %Y")
    )));

  let notification_embed = BloomBotEmbed::new()
    .title(":tada: You've won a key! :tada:")
    .thumbnail(winner.user.avatar_url().unwrap_or_default())
    .field(
      "**Congratulations on winning the giveaway!** 🥳",
      "You've won a key for [Playne: The Meditation Game](<https://store.steampowered.com/app/865540/PLAYNE__The_Meditation_Game/>) on Steam!\n\n**Would you like to redeem your key? If yes, press 'Redeem' below! Otherwise, click 'Cancel' to leave it for someone else :)**",
      false,
    )
    .footer(CreateEmbedFooter::new(format!(
      "From {guild_name} | If you need any assistance, please contact server staff."
    )));

  let announcement_channel = ChannelId::new(CHANNELS.announcement);
  let dm_channel = winner.user.create_dm_channel(ctx).await?;
  let log_channel = ChannelId::new(CHANNELS.logs);

  announcement_channel
    .send_message(ctx, CreateMessage::new().embed(announcement_embed))
    .await?;

  let ctx_id = ctx.id();
  let redeem_id = format!("{ctx_id}redeem");
  let cancel_id = format!("{ctx_id}cancel");

  let dm_test = CreateMessage::new().content("Hey, guess what...");

  let notif_msg = CreateMessage::new()
    .embed(notification_embed)
    .components(vec![CreateActionRow::Buttons(vec![
      CreateButton::new(redeem_id.as_str())
        .label("Redeem")
        .style(ButtonStyle::Success),
      CreateButton::new(cancel_id.as_str())
        .label("Cancel")
        .style(ButtonStyle::Danger),
    ])]);

  let mut notification = if (dm_channel.send_message(ctx, dm_test).await).is_ok() {
    dm_channel.send_message(ctx, notif_msg).await?
  } else {
    let thread_channel = ChannelId::from(CHANNELS.private_thread_default);
    let notification_thread = thread_channel
      .create_thread(
        ctx,
        CreateThread::new("Private Notification: You won!".to_string())
          .invitable(false)
          .kind(ChannelType::PrivateThread),
      )
      .await?;
    let thread_initial_message = format!("Private notification for <@{}>:", winner.user.id);
    notification_thread
      .send_message(
        ctx,
        notif_msg
          .content(thread_initial_message)
          .allowed_mentions(CreateAllowedMentions::new().users([winner.user.id])),
      )
      .await?
  };

  ctx
    .send(CreateReply::default().content(format!(
      "{} Notified {} and sent announcement!",
      EMOJI.mmcheck, winner.user
    )))
    .await?;

  // Loop through incoming interactions with the buttons.
  while let Some(press) = ComponentInteractionCollector::new(ctx)
    .filter(move |press| {
      press.user.id == winner.user.id && press.data.custom_id.starts_with(&ctx_id.to_string())
    })
    // Timeout when no navigation button has been pressed for 24 hours.
    .timeout(Duration::from_secs(3600 * 24))
    .await
  {
    // Depending on which button was pressed, confirm or cancel.
    if press.data.custom_id == redeem_id {
      let mut conn = ctx.data().db.get_connection_with_retry(5).await?;
      DatabaseHandler::mark_key_used(&mut conn, &reserved_key).await?;
      let hyperlink = format!(
        "[Redeem your key](https://store.steampowered.com/account/registerkey?key={reserved_key})"
      );
      DatabaseHandler::record_steamkey_receipt(&mut conn, &guild_id, &winner.user.id).await?;

      notification
        .edit(ctx, EditMessage::new().components(Vec::new()))
        .await?;

      notification
        .channel_id
        .send_message(
          ctx,
          CreateMessage::new().content(format!(
            "Awesome! Here is your key:\n```{reserved_key}```\n{hyperlink}"
          )),
        )
        .await?;

      let log_embed = BloomBotEmbed::new()
        .title("**Key Redeemed**")
        .description(format!(
          "Playne key redeemed by <@{}>. Key has been marked as used.",
          winner.user.id
        ))
        .footer(
          CreateEmbedFooter::new(format!("{} ({})", winner.user.name, winner.user.id))
            .icon_url(winner.user.avatar_url().unwrap_or_default()),
        );

      log_channel
        .send_message(ctx, CreateMessage::new().embed(log_embed))
        .await?;

      return Ok(());
    } else if press.data.custom_id == cancel_id {
      let mut conn = ctx.data().db.get_connection_with_retry(5).await?;
      DatabaseHandler::unreserve_key(&mut conn, &reserved_key).await?;

      notification
        .edit(ctx, EditMessage::new().components(Vec::new()))
        .await?;

      notification
        .channel_id
        .send_message(
          ctx,
          CreateMessage::new().content("Alright, we'll keep it for someone else. Congrats again!"),
        )
        .await?;

      let log_embed = BloomBotEmbed::new()
        .title("**Key Declined**")
        .description(format!(
          "Playne key declined by <@{}>. Key has been returned to the pool.",
          winner.user.id
        ))
        .footer(
          CreateEmbedFooter::new(format!("{} ({})", winner.user.name, winner.user.id))
            .icon_url(winner.user.avatar_url().unwrap_or_default()),
        );

      log_channel
        .send_message(ctx, CreateMessage::new().embed(log_embed))
        .await?;

      return Ok(());
    }

    // This is an unrelated button interaction.
    continue;
  }

  let timeout_embed = BloomBotEmbed::new()
    .title("**Congratulations on winning the giveaway!** 🥳")
    .description(
      "You've won a key for [Playne: The Meditation Game](<https://store.steampowered.com/app/865540/PLAYNE__The_Meditation_Game/>) on Steam!\n\n**Would you like to redeem your key? Please contact server staff and we'll get one to you!**",
    )
    .footer(CreateEmbedFooter::new(format!("From {guild_name}")));

  notification
    .edit(
      ctx,
      EditMessage::new()
        .embed(timeout_embed)
        .components(Vec::new()),
    )
    .await?;

  let log_embed = BloomBotEmbed::new()
    .title("**Key Offer Timed Out**")
    .description(format!(
      "Sent Playne key offer to <@{}>, but user did not respond within 24 hours. Key has been returned to the pool and user has been asked to contact a moderator if they wish to claim their key.",
      winner.user.id
    ))
    .footer(
      CreateEmbedFooter::new(format!("{} ({})", winner.user.name, winner.user.id))
        .icon_url(winner.user.avatar_url().unwrap_or_default()),
    );

  log_channel
    .send_message(ctx, CreateMessage::new().embed(log_embed))
    .await?;

  Ok(())
}

/// Pick a winner for the monthly challenge
///
/// Picks the winner for the monthly meditation challenge and allows them to claim an unused Playne key.
///
/// Finds a user who meets the following criteria (defaults):
/// - Has the monthly challenge participant role
/// - Has tracked at least 30 minutes during the specified month
/// - Has at least 8 sessions during the specified month
/// - Has not received a Playne key previously
/// If multiple users meet this criteria, one is chosen at random.
#[poise::command(
  slash_command,
  required_permissions = "ADMINISTRATOR",
  default_member_permissions = "ADMINISTRATOR",
  category = "Admin Commands",
  rename = "pickwinner",
  guild_only
)]
pub async fn pick_winner(
  ctx: Context<'_>,
  #[description = "Year to pick for (defaults to current year in UTC)"] year: Option<i32>,
  #[description = "Month to pick for (defaults to current month in UTC)"] month: Option<Months>,
  #[description = "Minimum minutes (defaults to 30 minutes)"] minimum_minutes: Option<i64>,
  #[description = "Minimum sessions (defaults to 8 sessions)"] minimum_count: Option<u64>,
  #[description = "Allow multiple keys (defaults to false)"] allow_multiple_keys: Option<bool>,
) -> Result<()> {
  let guild_id = ctx
    .guild_id()
    .with_context(|| "Failed to retrieve guild ID from context")?;

  ctx.defer_ephemeral().await?;

  let mut transaction = ctx.data().db.start_transaction_with_retry(5).await?;

  if !DatabaseHandler::unused_key_exists(&mut transaction, &guild_id).await? {
    let msg = format!("{} No unused keys found.", EMOJI.mminfo);
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  }

  let year = year.unwrap_or_else(|| Utc::now().year());
  let month = month.map_or_else(
    || Utc::now().month(),
    |month| match month {
      Months::January => 1,
      Months::February => 2,
      Months::March => 3,
      Months::April => 4,
      Months::May => 5,
      Months::June => 6,
      Months::July => 7,
      Months::August => 8,
      Months::September => 9,
      Months::October => 10,
      Months::November => 11,
      Months::December => 12,
    },
  );

  let Some(start_date) = NaiveDate::from_ymd_opt(year, month, 1) else {
    let msg = "Invalid date.";
    ctx
      .send(CreateReply::default().content(msg).ephemeral(true))
      .await?;
    return Ok(());
  };

  let end_date = start_date + ChronoMonths::new(1);

  let time = NaiveTime::from_hms_opt(0, 0, 0)
    .with_context(|| "Failed to assign hardcoded 00:00:00 NaiveTime to time")?;

  let start = NaiveDateTime::new(start_date, time).and_utc();
  let end = NaiveDateTime::new(end_date, time).and_utc();

  // Since the stream is async, we can't use the same connection for the transaction.
  let mut conn = ctx.data().db.get_connection_with_retry(5).await?;
  let mut candidates = DatabaseHandler::get_candidates(&mut conn, &start, &end, &guild_id);
  let challenger_role = RoleId::new(ROLES.meditation_challenger);

  // The database randomizes the order, so we use the first candidate that meets all requirements.
  while let Some(winner) = candidates.next().await {
    let Ok(winner) = winner else {
      continue;
    };

    // User is a guild member.
    let Ok(member) = guild_id.member(ctx, winner).await else {
      continue;
    };

    // User has the challenger role.
    if !member.roles.contains(&challenger_role) {
      continue;
    }

    // User has not received a key or multiple keys is allowed.
    if !allow_multiple_keys.unwrap_or(false)
      && DatabaseHandler::steamkey_recipient_exists(&mut transaction, &guild_id, &winner).await?
    {
      continue;
    }

    let minutes =
      DatabaseHandler::get_candidate_sum(&mut transaction, &guild_id, &winner, &start, &end)
        .await?;
    let count =
      DatabaseHandler::get_candidate_count(&mut transaction, &guild_id, &winner, &start, &end)
        .await?;

    // Make sure user meets minimum tracking requirements.
    // Default is 30 minutes and 8 sessions during the challenge period.
    if minutes < minimum_minutes.unwrap_or(30) || count < minimum_count.unwrap_or(8) {
      continue;
    }

    let Some(reserved_key) =
      DatabaseHandler::reserve_key(&mut transaction, &guild_id, &winner).await?
    else {
      let msg = format!(
        "{} No unused keys found. Please add one and try again.",
        EMOJI.mminfo
      );
      ctx.send(CreateReply::default().content(msg)).await?;
      return Ok(());
    };

    DatabaseHandler::commit_transaction(transaction).await?;

    finalize_winner(ctx, guild_id, member, minutes, start, reserved_key).await?;

    return Ok(());
  }

  let msg = "No winner found.";
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}
