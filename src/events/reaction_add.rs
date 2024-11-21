use anyhow::{Context as AnyhowContext, Result};
use poise::serenity_prelude::{builder::*, ChannelId, Context, Reaction, ReactionType};

use crate::config::{BloomBotEmbed, CHANNELS, EMOTES, ROLES};
use crate::database::DatabaseHandler;
use crate::events::helpers::starboard;

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
  starboard::add_star(ctx, database, add_reaction).await?;

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
      let message_link = message.link();
      let message_user = message.author;
      let message_channel_name = message.channel_id.name(ctx).await?;
      let reporting_user = reaction.user(&ctx).await?;

      let message_content = if message.content.is_empty() {
        match message.attachments.first() {
          Some(attachment) => format!("**Attachment**\n{}", attachment.url),
          None => message.content,
        }
      } else {
        message.content
      };

      report_channel_id
        .send_message(
          &ctx,
          CreateMessage::new()
            .content(format!("<@&{}> Message Reported", ROLES.staff))
            .embed(
              BloomBotEmbed::new()
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
            BloomBotEmbed::new()
              .title("Report")
              .description("Your report has been sent to the moderation team."),
          ),
        )
        .await?;
    }
  }

  Ok(())
}
