use crate::config::{self, CHANNELS};
use crate::database::DatabaseHandler;
use anyhow::Result;
use poise::serenity_prelude::{builder::*, ChannelId, Context, MessageFlags, Reaction};

// mod guild_member_addition;
mod guild_member_removal;
mod guild_member_update;
mod message_delete;
mod reaction_add;
mod reaction_remove;
pub mod leaderboards;

// pub use guild_member_addition::guild_member_addition;
pub use guild_member_removal::guild_member_removal;
pub use guild_member_update::guild_member_update;
pub use message_delete::message_delete;
pub use reaction_add::reaction_add;
pub use reaction_remove::reaction_remove;

pub async fn create_star_message(
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
      Some(embed) => if starred_message.content.is_empty() {
        config::BloomBotEmbed::from(embed.clone())
      } else {
        config::BloomBotEmbed::new().description(starred_message.content.clone())
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
