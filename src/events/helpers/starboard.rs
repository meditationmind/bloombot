use crate::config::{self, CHANNELS, EMOTES};
use crate::database::DatabaseHandler;
use anyhow::Result;
use poise::serenity_prelude::{
  builder::*, ChannelId, Context, MessageFlags, Reaction, ReactionType,
};

async fn create_star_message(
  ctx: &Context,
  transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
  reaction: &Reaction,
  star_count: u64,
) -> Result<()> {
  if star_count >= config::MIN_STARS {
    let starred_message = reaction.message(&ctx).await?;
    let author_nick_or_name = match reaction.guild_id {
      Some(guild_id) => starred_message
        .author
        .nick_in(&ctx, guild_id)
        .await
        .unwrap_or_else(|| {
          starred_message
            .author
            .global_name
            .as_ref()
            .unwrap_or(&starred_message.author.name)
            .clone()
        }),
      None => starred_message.author.name.clone(),
    };

    let message_type = match starred_message.flags {
      Some(flags) => {
        if flags.contains(MessageFlags::IS_VOICE_MESSAGE) {
          "voice message"
        } else {
          "message"
        }
      }
      None => "message",
    };

    let mut embed = match starred_message.embeds.first() {
      Some(embed) => {
        if starred_message.content.is_empty() {
          config::BloomBotEmbed::from(embed.clone())
        } else {
          config::BloomBotEmbed::new().description(starred_message.content.clone())
        }
      }
      None => config::BloomBotEmbed::new().description(starred_message.content.clone()),
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
        embed = embed.image(sticker_url.clone());
      }
    }

    let starboard_channel = ChannelId::new(CHANNELS.starchannel);

    let starboard_message = match starred_message.attachments.first() {
      Some(attachment) => {
        if attachment
          .content_type
          .as_ref()
          .is_some_and(|content_type| content_type.starts_with("image"))
        {
          if starred_message.attachments.len() > 1 {
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
                embed = embed.image(attachment.url.clone());
                msg = msg.add_embed(embed.clone());
                image_count += 1;
              }
            }

            starboard_channel.send_message(ctx, msg).await?
          } else {
            embed = embed.image(attachment.url.clone());

            starboard_channel
              .send_message(ctx, CreateMessage::new().embed(embed))
              .await?
          }
        } else {
          starboard_channel
            .send_message(
              ctx,
              CreateMessage::new()
                .embed(embed)
                .add_file(CreateAttachment::url(ctx, attachment.url.as_str()).await?),
            )
            .await?
        }
      }
      None => {
        starboard_channel
          .send_message(ctx, CreateMessage::new().embed(embed))
          .await?
      }
    };

    DatabaseHandler::add_star_message(
      transaction,
      &reaction.message_id,
      &starboard_message.id,
      &reaction.channel_id,
    )
    .await?;
  }

  Ok(())
}

pub async fn add_star(
  ctx: &Context,
  database: &DatabaseHandler,
  reaction: &Reaction,
) -> Result<()> {
  if let ReactionType::Unicode(emoji) = &reaction.emoji {
    if emoji == EMOTES.star && reaction.channel_id != CHANNELS.starchannel {
      // Get count of star emoji on message
      let star_count = reaction
        .message(&ctx)
        .await?
        .reactions
        .iter()
        .find(|r| r.reaction_type == ReactionType::Unicode(EMOTES.star.to_owned()))
        .map_or(0, |r| r.count);

      let mut transaction = database.start_transaction().await?;
      let star_message =
        DatabaseHandler::get_star_message_by_message_id(&mut transaction, &reaction.message_id)
          .await?;

      if let Some(star_message) = star_message {
        // Already exists, find the starboard channel
        let starboard_channel = ChannelId::new(config::CHANNELS.starchannel);

        // Get the starboard message
        let mut starboard_message = starboard_channel
          .message(&ctx, star_message.board_message_id)
          .await?;

        // Check to see if message was created by previous bot
        if starboard_message.author.id == ctx.cache.current_user().id {
          let existing_embeds = starboard_message.embeds.clone();
          let mut updated_embeds: Vec<CreateEmbed> = Vec::new();

          for embed in existing_embeds {
            let updated_embed = CreateEmbed::from(embed).footer(CreateEmbedFooter::new(format!(
              "⭐ Times starred: {star_count}"
            )));
            updated_embeds.push(updated_embed);
          }

          starboard_message
            .edit(ctx, EditMessage::new().embeds(updated_embeds))
            .await?;
        } else {
          starboard_channel
            .delete_message(&ctx, starboard_message.id)
            .await?;
          DatabaseHandler::remove_star_message(&mut transaction, &star_message.record_id).await?;

          create_star_message(ctx, &mut transaction, reaction, star_count).await?;
          transaction.commit().await?;
        }
      } else {
        create_star_message(ctx, &mut transaction, reaction, star_count).await?;
        transaction.commit().await?;
      }
    }
  }

  Ok(())
}

pub async fn remove_star(
  ctx: &Context,
  database: &DatabaseHandler,
  reaction: &Reaction,
) -> Result<()> {
  if let ReactionType::Unicode(emoji) = &reaction.emoji {
    if emoji == EMOTES.star {
      let mut transaction = database.start_transaction().await?;
      let star_message =
        DatabaseHandler::get_star_message_by_message_id(&mut transaction, &reaction.message_id)
          .await?;

      if let Some(star_message) = star_message {
        let star_count = reaction
          .message(&ctx)
          .await?
          .reactions
          .iter()
          .find(|r| r.reaction_type == ReactionType::Unicode(EMOTES.star.to_owned()))
          .map_or(0, |r| r.count);

        let starboard_channel = ChannelId::new(config::CHANNELS.starchannel);

        if star_count >= config::MIN_STARS {
          // Get the starboard message
          let mut starboard_message = starboard_channel
            .message(&ctx, star_message.board_message_id)
            .await?;

          // Check to see if message was created by previous bot
          if starboard_message.author.id == ctx.cache.current_user().id {
            let existing_embeds = starboard_message.embeds.clone();
            let mut updated_embeds: Vec<CreateEmbed> = Vec::new();

            for embed in existing_embeds {
              let updated_embed = CreateEmbed::from(embed).footer(CreateEmbedFooter::new(format!(
                "⭐ Times starred: {star_count}"
              )));
              updated_embeds.push(updated_embed);
            }

            starboard_message
              .edit(ctx, EditMessage::new().embeds(updated_embeds))
              .await?;
          } else {
            starboard_channel
              .delete_message(&ctx, starboard_message.id)
              .await?;
            DatabaseHandler::remove_star_message(&mut transaction, &star_message.record_id).await?;

            create_star_message(ctx, &mut transaction, reaction, star_count).await?;
            transaction.commit().await?;
          }
        } else {
          starboard_channel
            .delete_message(&ctx, star_message.board_message_id)
            .await?;
          DatabaseHandler::remove_star_message(&mut transaction, &star_message.record_id).await?;
          transaction.commit().await?;
        }
      }
    }
  }

  Ok(())
}
