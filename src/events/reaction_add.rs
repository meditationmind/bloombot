use crate::config::{self, CHANNELS, EMOTES, ROLES};
use crate::database::DatabaseHandler;
use crate::events::create_star_message;
use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{builder::*, ChannelId, Context, Reaction, ReactionType};

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
          DatabaseHandler::delete_star_message(&mut transaction, &star_message.record_id).await?;

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
