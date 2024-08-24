use crate::config::{self, CHANNELS, EMOTES, ROLES};
use crate::database::DatabaseHandler;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{
  builder::*, ChannelId, Context, MessageFlags, Reaction, ReactionType,
};

pub async fn reaction_add(
  ctx: &Context,
  database: &DatabaseHandler,
  add_reaction: &Reaction,
) -> Result<()> {
  if add_reaction.user_id.is_none() {
    // Should only happen if reaction is added by bot when cache is not available.
    // That should never happen, so we'll remove the reaction here just to be safe.
    add_reaction
      .delete(&ctx)
      .await
      .with_context(|| "Failed to remove reaction from message")?;
    return Ok(());
  }

  check_report(ctx, add_reaction).await?;
  add_star(ctx, database, add_reaction).await?;

  Ok(())
}

async fn check_report(ctx: &Context, reaction: &Reaction) -> Result<()> {
  if let ReactionType::Custom { id, .. } = reaction.emoji {
    if id == EMOTES.report {
      // Remove reaction from message
      reaction
        .delete(&ctx)
        .await
        .with_context(|| "Failed to remove report reaction from message")?;

      let report_channel_id = ChannelId::new(CHANNELS.reportchannel);
      let message = reaction.message(&ctx).await?;
      let message_link = message.link().clone();
      let message_user = message.author;
      let message_channel_name = message.channel_id.name(ctx).await?;
      let reporting_user = reaction.user(&ctx).await?;

      let message_content = if message.content.is_empty() {
        match message.attachments.first() {
          Some(attachment) => format!("**Attachment**\n{}", attachment.url.clone()),
          None => message.content.clone(),
        }
      } else {
        message.content.clone()
      };

      report_channel_id
        .send_message(
          &ctx,
          CreateMessage::new()
            .content(format!("<@&{}> Message Reported", ROLES.staff))
            .embed(
              config::BloomBotEmbed::new()
                .author(CreateEmbedAuthor::new(&message_user.name).icon_url(message_user.face()))
                .description(message_content)
                .field("Link", format!("[Go to message]({message_link})"), false)
                .footer(CreateEmbedFooter::new(format!(
                  "Author ID: {}\nReported via reaction in #{} by {} ({})",
                  &message_user.id, message_channel_name, reporting_user.name, reporting_user.id
                )))
                .timestamp(message.timestamp),
            ),
        )
        .await?;

      reporting_user
        .dm(
          &ctx,
          CreateMessage::new().embed(
            config::BloomBotEmbed::new()
              .title("Report")
              .description("Your report has been sent to the moderation team."),
          ),
        )
        .await?;
    }
  }

  Ok(())
}

async fn add_star(ctx: &Context, database: &DatabaseHandler, reaction: &Reaction) -> Result<()> {
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

        let existing_embed = starboard_message.embeds.first().with_context(|| {
          format!(
            "Failed to get embed from starboard message {}",
            starboard_message.id
          )
        })?;

        let updated_embed = CreateEmbed::from(existing_embed.clone()).footer(
          CreateEmbedFooter::new(format!("⭐ Times starred: {star_count}")),
        );

        // Check to see if message was created by previous bot
        if starboard_message.author.id == ctx.cache.current_user().id {
          starboard_message
            .edit(ctx, EditMessage::new().embed(updated_embed))
            .await?;
        } else {
          _ = starboard_channel
            .delete_message(&ctx, starboard_message.id)
            .await;

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
      Some(embed) => config::BloomBotEmbed::from(embed.clone()),
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
                let image = CreateAttachment::url(ctx, attachment.url.as_str()).await?;
                let filename = image.filename.clone();
                msg = msg.add_file(image);
                embed = embed.attachment(filename);
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

    DatabaseHandler::insert_star_message(
      transaction,
      &reaction.message_id,
      &starboard_message.id,
      &reaction.channel_id,
    )
    .await?;
  }

  Ok(())
}
