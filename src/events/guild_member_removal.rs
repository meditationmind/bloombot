use anyhow::Result;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, User};

use crate::config::{BloomBotEmbed, CHANNELS};

pub async fn guild_member_removal(ctx: &Context, user: &User) -> Result<()> {
  let welcome_channel = ChannelId::new(CHANNELS.welcome);
  let username = user
    .name
    .chars()
    .map(|c| {
      if matches!(c, '_') {
        c.to_string()
      } else {
        format!("\\{c}")
      }
    })
    .collect::<String>();

  welcome_channel
    .send_message(
      &ctx,
      CreateMessage::new().embed(
        BloomBotEmbed::new()
          .title("Member Left")
          .description(format!(
            "We wish you well on your future endeavors, {username} :pray:"
          )),
      ),
    )
    .await?;

  Ok(())
}
