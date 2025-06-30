use anyhow::Result;
use poise::serenity_prelude::{ChannelId, ComponentInteraction, CreateInteractionResponse};
use poise::serenity_prelude::{CreateInteractionResponseMessage, Mentionable};

use crate::{config::EMOJI, data::bloom::Context};

/// Checks if the user who initiated the command is present in the voice channel of the event they
/// are attempting to start. If not present, asks user to join the voice channel and returns `true`.
/// Returns `false` if user is present or [`ScheduledEvent::channel_id`][event_channel] is `None`.
///
/// [event_channel]: poise::serenity_prelude::ScheduledEvent::channel_id
pub async fn not_present(
  ctx: Context<'_>,
  guild_id: poise::serenity_prelude::GuildId,
  event_channel: Option<ChannelId>,
  press: &ComponentInteraction,
) -> Result<bool> {
  let Some(event_vc) = event_channel else {
    return Ok(false);
  };

  let not_present = {
    guild_id.to_guild_cached(&ctx).is_some_and(|guild| {
      !guild.voice_states.contains_key(&ctx.author().id)
        || guild
          .voice_states
          .get(&ctx.author().id)
          .is_some_and(|state| state.channel_id.is_some_and(|vc| vc != event_vc))
    })
  };

  if not_present {
    let msg = format!(
      "{} Please join {} before starting the event. \
          Events may end automatically if no one is present in the VC.",
      EMOJI.mminfo,
      event_vc.mention()
    );
    press
      .create_response(
        ctx,
        CreateInteractionResponse::Message(
          CreateInteractionResponseMessage::new()
            .content(msg)
            .ephemeral(true)
            .components(Vec::new()),
        ),
      )
      .await?;
  }

  Ok(not_present)
}
