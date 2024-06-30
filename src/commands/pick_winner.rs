use crate::config::{BloomBotEmbed, CHANNELS, ROLES};
use crate::database::DatabaseHandler;
use crate::Context;
use anyhow::Result;
use chrono::Datelike;
use futures::StreamExt;
use poise::serenity_prelude::{self as serenity, builder::*};
use poise::CreateReply;

#[derive(Debug, Clone, Copy, poise::ChoiceParameter)]
pub enum Months {
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
  reserved_key: String,
  ctx: Context<'_>,
  winner: serenity::Member,
  minutes: i64,
  selected_date: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
  let now = chrono::Utc::now();
  let guild_name = ctx
    .guild()
    .expect("should only be called in an available guild that is currently cached")
    .name
    .clone();

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
      ))
    ).clone();

  let dm_embed = BloomBotEmbed::new()
    .title(":tada: You've won a key! :tada:")
    .thumbnail(winner.user.avatar_url().unwrap_or_default())
    .field(
      "**Congratulations on winning the giveaway!** 🥳",
      "You've won a key for [Playne: The Meditation Game](<https://store.steampowered.com/app/865540/PLAYNE__The_Meditation_Game/>) on Steam!\n\n**Would you like to redeem your key? If yes, press 'Redeem' below! Otherwise, click 'Cancel' to leave it for someone else :)**",
      false,
    )
    .footer(CreateEmbedFooter::new(format!(
        "From {guild_name} | If you need any assistance, please contact server staff."
      ))
    ).clone();

  let announcement_channel = serenity::ChannelId::new(CHANNELS.announcement);
  let dm_channel = winner.user.create_dm_channel(ctx).await?;

  announcement_channel
    .send_message(ctx, CreateMessage::new().embed(announcement_embed))
    .await?;

  let ctx_id = ctx.id();
  let redeem_id = format!("{ctx_id}redeem");
  let cancel_id = format!("{ctx_id}cancel");

  let Ok(mut dm_message) = dm_channel
    .send_message(
      ctx,
      CreateMessage::new()
        .embed(dm_embed)
        .components(vec![CreateActionRow::Buttons(vec![
          CreateButton::new(redeem_id.clone())
            .label("Redeem")
            .style(serenity::ButtonStyle::Success),
          CreateButton::new(cancel_id.clone())
            .label("Cancel")
            .style(serenity::ButtonStyle::Danger),
        ])]),
    )
    .await
  else {
    ctx
      .send(CreateReply::default().content(":x: Could not send DM to member. Please run `/usekey` and copy a key manually if they want one.\n\n**No key has been used.**"))
      .await?;
    return Ok(());
  };

  ctx
    .send(CreateReply::default().content(format!(
      ":white_check_mark: Sent DM to {} and sent announcement!",
      winner.user
    )))
    .await?;

  // Loop through incoming interactions with the buttons
  while let Some(press) = serenity::ComponentInteractionCollector::new(ctx)
    // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
    // button was pressed
    .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
    // Timeout when no navigation button has been pressed for 24 hours
    .timeout(std::time::Duration::from_secs(3600 * 24))
    .await
  {
    // Depending on which button was pressed, confirm or cancel
    if press.data.custom_id == redeem_id {
      let mut conn = ctx.data().db.get_connection_with_retry(5).await?;
      DatabaseHandler::mark_key_used(&mut conn, &reserved_key).await?;
      let hyperlink = format!(
        "[Redeem your key](https://store.steampowered.com/account/registerkey?key={reserved_key})"
      );
      DatabaseHandler::record_steamkey_receipt(
        &mut conn,
        &ctx.guild_id().unwrap(),
        &winner.user.id,
      )
      .await?;

      dm_message
        .edit(ctx, EditMessage::new().components(Vec::new()))
        .await?;

      dm_channel
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
        )
        .clone();

      let log_channel = serenity::ChannelId::new(CHANNELS.logs);

      log_channel
        .send_message(ctx, CreateMessage::new().embed(log_embed))
        .await?;

      return Ok(());
    } else if press.data.custom_id == cancel_id {
      let mut conn = ctx.data().db.get_connection_with_retry(5).await?;
      DatabaseHandler::unreserve_key(&mut conn, &reserved_key).await?;

      dm_message
        .edit(ctx, EditMessage::new().components(Vec::new()))
        .await?;

      dm_channel
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
        )
        .clone();

      let log_channel = serenity::ChannelId::new(CHANNELS.logs);

      log_channel
        .send_message(ctx, CreateMessage::new().embed(log_embed))
        .await?;

      return Ok(());
    }

    // This is an unrelated button interaction
    continue;
  }

  let timeout_embed = BloomBotEmbed::new()
    .title("**Congratulations on winning the giveaway!** 🥳")
    .description("You've won a key for [Playne: The Meditation Game](<https://store.steampowered.com/app/865540/PLAYNE__The_Meditation_Game/>) on Steam!\n\n**Would you like to redeem your key? Please contact server staff and we'll get one to you!**")
    .footer(CreateEmbedFooter::new(format!("From {guild_name}"))).clone();

  dm_message
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
    .footer(CreateEmbedFooter::new(format!("{} ({})", winner.user.name, winner.user.id))
      .icon_url(winner.user.avatar_url().unwrap_or_default())
    ).clone();

  let log_channel = serenity::ChannelId::new(CHANNELS.logs);

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
/// - Has the `@meditation challengers` role
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
  //hide_in_help,
  guild_only
)]
pub async fn pick_winner(
  ctx: Context<'_>,
  #[description = "The year to pick a winner for (defaults to this year in UTC)"] year: Option<i32>,
  #[description = "The month to pick a winner for (defaults to this month in UTC)"] month: Option<
    Months,
  >,
  #[description = "Minimum minutes for eligibility (defaults to 30 minutes)"]
  minimum_minutes: Option<i64>,
  #[description = "Minimum session count for eligibility (defaults to 8 sessions)"]
  minimum_count: Option<u64>,
  #[description = "Include users who have already received a Playne key (defaults to false)"]
  allow_multiple_keys: Option<bool>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let data = ctx.data();

  let guild_id = ctx.guild_id().unwrap();

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  if !DatabaseHandler::unused_key_exists(&mut transaction, &guild_id).await? {
    ctx
      .send(
        CreateReply::default()
          .content(":x: No unused keys found.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  }

  let year = year.unwrap_or_else(|| {
    let now = chrono::Utc::now();
    now.year()
  });

  let month = if let Some(month) = month {
    match month {
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
    }
  } else {
    let now = chrono::Utc::now();
    now.month()
  };

  let Some(start_date) = chrono::NaiveDate::from_ymd_opt(year, month, 1) else {
    ctx
      .send(
        CreateReply::default()
          .content("Invalid date.")
          .ephemeral(true),
      )
      .await?;
    return Ok(());
  };

  let end_date = start_date + chrono::Months::new(1);

  let time = chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap();

  let start_datetime = chrono::NaiveDateTime::new(start_date, time).and_utc();
  let end_datetime = chrono::NaiveDateTime::new(end_date, time).and_utc();

  let mut conn = data.db.get_connection_with_retry(5).await?;
  // Since the stream is async, we can't use the same connection for the transaction
  let mut database_winner_candidates =
    DatabaseHandler::get_winner_candidates(&mut conn, start_datetime, end_datetime, &guild_id);

  // The database already randomizes the order... we can use the first one that has the role
  let winner_role_id = serenity::RoleId::new(ROLES.meditation_challenger);

  while let Some(winner) = database_winner_candidates.next().await {
    let Ok(winner) = winner else {
      continue;
    };

    let Ok(member) = guild_id.member(ctx, winner).await else {
      continue;
    };

    if !member.roles.contains(&winner_role_id) {
      continue;
    }

    if !allow_multiple_keys.unwrap_or(false)
      && DatabaseHandler::steamkey_recipient_exists(&mut transaction, &guild_id, &member.user.id)
        .await?
    {
      continue;
    }

    let challenge_minutes = DatabaseHandler::get_winner_candidate_meditation_sum(
      &mut transaction,
      &guild_id,
      &member.user.id,
      start_datetime,
      end_datetime,
    )
    .await?;

    let challenge_count = DatabaseHandler::get_winner_candidate_meditation_count(
      &mut transaction,
      &guild_id,
      &member.user.id,
      start_datetime,
      end_datetime,
    )
    .await?;

    // Make sure user has at least 30 minutes and 8 sessions during the challenge period
    if challenge_minutes < minimum_minutes.unwrap_or(30)
      || challenge_count < minimum_count.unwrap_or(8)
    {
      continue;
    }

    let Some(reserved_key) =
      DatabaseHandler::reserve_key(&mut transaction, &guild_id, &member.user.id).await?
    else {
      ctx
      .send(CreateReply::default().content(":x: No unused keys found. Please add one and run `/usekey` to give them one if they want one."))
      .await?;
      return Ok(());
    };

    DatabaseHandler::commit_transaction(transaction).await?;

    finalize_winner(reserved_key, ctx, member, challenge_minutes, start_datetime).await?;

    return Ok(());
  }

  ctx
    .send(
      CreateReply::default()
        .content("No winner found.")
        .ephemeral(true),
    )
    .await?;

  Ok(())
}
