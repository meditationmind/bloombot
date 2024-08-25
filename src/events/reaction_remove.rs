use crate::config::{self, EMOTES};
use crate::database::DatabaseHandler;
use crate::events::create_star_message;
use anyhow::Result;
use poise::serenity_prelude::{builder::*, ChannelId, Context, Reaction, ReactionType};

pub async fn reaction_remove(
  ctx: &Context,
  database: &DatabaseHandler,
  remove_reaction: &Reaction,
) -> Result<()> {
  remove_star(ctx, database, remove_reaction).await?;

  Ok(())
}

async fn remove_star(ctx: &Context, database: &DatabaseHandler, reaction: &Reaction) -> Result<()> {
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
            DatabaseHandler::delete_star_message(&mut transaction, &star_message.record_id).await?;

            create_star_message(ctx, &mut transaction, reaction, star_count).await?;
            transaction.commit().await?;
          }
        } else {
          starboard_channel
            .delete_message(&ctx, star_message.board_message_id)
            .await?;
          DatabaseHandler::delete_star_message(&mut transaction, &star_message.record_id).await?;
          transaction.commit().await?;
        }
      }
    }
  }

  Ok(())
}
