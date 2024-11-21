use anyhow::Result;
use poise::serenity_prelude::{builder::*, ChannelId, Context};
use poise::serenity_prelude::{MessageFlags, Reaction, ReactionType};
use sqlx::{Postgres, Transaction};

use crate::config::{BloomBotEmbed, CHANNELS, EMOTES, MIN_STARS};
use crate::data::star_message::StarMessage;
use crate::database::DatabaseHandler;

async fn create_star_message(
  ctx: &Context,
  transaction: &mut Transaction<'_, Postgres>,
  reaction: &Reaction,
  star_count: u64,
) -> Result<()> {
  if star_count < MIN_STARS {
    return Ok(());
  }

  let starred_message = reaction.message(&ctx).await?;
  let author_nick_or_name = match reaction.guild_id {
    Some(guild_id) => &starred_message
      .author
      .nick_in(&ctx, guild_id)
      .await
      .unwrap_or_else(|| {
        starred_message
          .author
          .global_name
          .as_deref()
          .unwrap_or(&starred_message.author.name)
          .to_string()
      }),
    None => &starred_message.author.name,
  };

  let message_type = if starred_message
    .flags
    .is_some_and(|flags| flags.contains(MessageFlags::IS_VOICE_MESSAGE))
  {
    "voice message"
  } else {
    "message"
  };

  let mut embed = match starred_message.embeds.first() {
    // If the starred message is embed-only, just clone the embed.
    Some(embed) if starred_message.content.is_empty() => BloomBotEmbed::from(embed.clone()),
    // If there is user-created content, prioritize the content.
    Some(embed) => BloomBotEmbed::from(embed.clone())
      .title("")
      .description(&starred_message.content),
    None => BloomBotEmbed::new().description(&starred_message.content),
  };

  embed = embed
    .author(CreateEmbedAuthor::new(author_nick_or_name).icon_url(starred_message.author.face()))
    .field(
      "Link",
      format!(
        "**[Click to jump to {}.]({})**",
        message_type,
        starred_message.link()
      ),
      false,
    )
    .footer(CreateEmbedFooter::new(format!(
      "⭐ Times starred: {star_count}"
    )));

  if let Some(sticker) = starred_message.sticker_items.first() {
    if let Some(sticker_url) = sticker.image_url() {
      embed = embed.image(sticker_url);
    }
  }

  let starboard_channel = ChannelId::new(CHANNELS.starchannel);

  let starboard_message = match starred_message.attachments.first() {
    // Multi-image embed
    Some(attachment)
      if attachment
        .content_type
        .as_ref()
        .is_some_and(|content_type| content_type.starts_with("image"))
        && starred_message.attachments.len() > 1 =>
    {
      embed = embed.url(starred_message.link());
      let mut msg = CreateMessage::new();
      let mut image_count = 0;

      for attachment in &starred_message.attachments {
        if attachment
          .content_type
          .as_ref()
          .is_some_and(|content_type| content_type.starts_with("image"))
        {
          if image_count > 3 {
            break;
          }
          embed = embed.image(attachment.url.as_str());
          msg = msg.add_embed(embed.clone());
          image_count += 1;
        }
      }

      starboard_channel.send_message(ctx, msg).await?
    }
    // Single-image embed
    Some(attachment)
      if attachment
        .content_type
        .as_ref()
        .is_some_and(|content_type| content_type.starts_with("image")) =>
    {
      embed = embed.image(attachment.url.as_str());

      starboard_channel
        .send_message(ctx, CreateMessage::new().embed(embed))
        .await?
    }
    // Non-image attachment
    Some(attachment) => {
      starboard_channel
        .send_message(
          ctx,
          CreateMessage::new()
            .embed(embed)
            .add_file(CreateAttachment::url(ctx, attachment.url.as_str()).await?),
        )
        .await?
    }
    // Tenor GIF only
    None
      if starred_message.content.starts_with("https://tenor.com")
        && starred_message
          .content
          .split_whitespace()
          .collect::<Vec<&str>>()
          .len()
          == 1 =>
    {
      starboard_channel
        .send_message(
          ctx,
          CreateMessage::new().content(format!(
            "[★]({}) [Click to jump to message.]({})",
            starred_message.content,
            starred_message.link()
          )),
        )
        .await?
    }
    // No attachments
    None => {
      starboard_channel
        .send_message(ctx, CreateMessage::new().embed(embed))
        .await?
    }
  };

  let star_message = StarMessage::new(
    reaction.channel_id,
    reaction.message_id,
    starboard_message.id,
  );
  DatabaseHandler::add_star_message(transaction, &star_message).await?;

  Ok(())
}

pub async fn add_star(
  ctx: &Context,
  database: &DatabaseHandler,
  reaction: &Reaction,
) -> Result<()> {
  let ReactionType::Unicode(emoji) = &reaction.emoji else {
    return Ok(());
  };

  if emoji == EMOTES.star && reaction.channel_id != CHANNELS.starchannel {
    // Get count of star emojis on message.
    let star_count = reaction
      .message(&ctx)
      .await?
      .reactions
      .iter()
      .find(|r| r.reaction_type == ReactionType::Unicode(EMOTES.star.to_owned()))
      .map_or(0, |r| r.count);

    let mut transaction = database.start_transaction().await?;

    let Some(star_message) =
      DatabaseHandler::get_star_message(&mut transaction, &reaction.message_id).await?
    else {
      // No message found in the database. Create a new starboard message and return.
      create_star_message(ctx, &mut transaction, reaction, star_count).await?;
      transaction.commit().await?;
      return Ok(());
    };

    let starboard_channel = ChannelId::new(CHANNELS.starchannel);

    // Get the existing starboard message from the starboard channel.
    let mut starboard_message = starboard_channel
      .message(&ctx, star_message.board_message)
      .await?;

    // No processing needed for Tenor GIFs.
    if starboard_message.content.starts_with("[★]") {
      return Ok(());
    }

    // Check to see if message was created by current Bloom. If so, edit the message.
    if starboard_message.author.id == ctx.cache.current_user().id {
      let existing_embeds = starboard_message.embeds.clone();
      let updated_embeds: Vec<CreateEmbed> = existing_embeds
        .into_iter()
        .map(|embed| {
          CreateEmbed::from(embed).footer(CreateEmbedFooter::new(format!(
            "⭐ Times starred: {star_count}"
          )))
        })
        .collect();

      starboard_message
        .edit(ctx, EditMessage::new().embeds(updated_embeds))
        .await?;
    } else {
      // If message was created by the previous bot, delete and recreate.
      starboard_channel
        .delete_message(&ctx, starboard_message.id)
        .await?;
      DatabaseHandler::remove_star_message(&mut transaction, &star_message.id).await?;

      create_star_message(ctx, &mut transaction, reaction, star_count).await?;
      transaction.commit().await?;
    }
  }

  Ok(())
}

pub async fn remove_star(
  ctx: &Context,
  database: &DatabaseHandler,
  reaction: &Reaction,
) -> Result<()> {
  let ReactionType::Unicode(emoji) = &reaction.emoji else {
    return Ok(());
  };

  if emoji == EMOTES.star {
    let mut transaction = database.start_transaction().await?;
    let Some(star_message) =
      DatabaseHandler::get_star_message(&mut transaction, &reaction.message_id).await?
    else {
      return Ok(());
    };

    // Get count of star emojis on message.
    let star_count = reaction
      .message(&ctx)
      .await?
      .reactions
      .iter()
      .find(|r| r.reaction_type == ReactionType::Unicode(EMOTES.star.to_owned()))
      .map_or(0, |r| r.count);

    let starboard_channel = ChannelId::new(CHANNELS.starchannel);

    if star_count < MIN_STARS {
      starboard_channel
        .delete_message(&ctx, star_message.board_message)
        .await?;
      DatabaseHandler::remove_star_message(&mut transaction, &star_message.id).await?;
      transaction.commit().await?;

      return Ok(());
    }

    // Get the existing starboard message from the starboard channel.
    let mut starboard_message = starboard_channel
      .message(&ctx, star_message.board_message)
      .await?;

    // No processing needed for Tenor GIFs.
    if starboard_message.content.starts_with("[★]") {
      return Ok(());
    }

    // Check to see if message was created by current Bloom. If so, edit the message.
    if starboard_message.author.id == ctx.cache.current_user().id {
      let existing_embeds = starboard_message.embeds.clone();
      let updated_embeds: Vec<CreateEmbed> = existing_embeds
        .into_iter()
        .map(|embed| {
          CreateEmbed::from(embed).footer(CreateEmbedFooter::new(format!(
            "⭐ Times starred: {star_count}"
          )))
        })
        .collect();

      starboard_message
        .edit(ctx, EditMessage::new().embeds(updated_embeds))
        .await?;
    } else {
      // If message was created by the previous bot, delete and recreate.
      starboard_channel
        .delete_message(&ctx, starboard_message.id)
        .await?;
      DatabaseHandler::remove_star_message(&mut transaction, &star_message.id).await?;

      create_star_message(ctx, &mut transaction, reaction, star_count).await?;
      transaction.commit().await?;
    }
  }

  Ok(())
}
