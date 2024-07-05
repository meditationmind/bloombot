use crate::config::{self, EMOTES};
use crate::database::DatabaseHandler;
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

          let existing_embed = starboard_message.embeds.first().expect(
            "existing Embed should always be available because starboard messages are created as Embeds",
          );
          let updated_embed = CreateEmbed::from(existing_embed.clone()).footer(
            CreateEmbedFooter::new(format!("⭐ Times starred: {star_count}")),
          );

          starboard_message
            .edit(ctx, EditMessage::new().embed(updated_embed))
            .await?;
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
