use anyhow::{Error, Result};
use poise::serenity_prelude::{ActivityData, Context as SerenityContext, FullEvent as Event};
use tracing::info;

use crate::{Data, events};

pub async fn listen(ctx: &SerenityContext, event: &Event, data: &Data) -> Result<(), Error> {
  let database = &data.db;

  match event {
    Event::GuildCreate { .. } => {
      events::guild_create(database).await?;
    }
    Event::GuildMemberRemoval { user, .. } => {
      events::guild_member_removal(ctx, user).await?;
    }
    Event::GuildMemberUpdate {
      old_if_available,
      new,
      ..
    } => {
      events::guild_member_update(ctx, old_if_available.as_ref(), new.as_ref()).await?;
    }
    Event::MessageDelete {
      deleted_message_id, ..
    } => {
      events::message_delete(database, deleted_message_id).await?;
    }
    Event::ReactionAdd { add_reaction } => {
      events::reaction_add(ctx, database, add_reaction).await?;
    }
    Event::ReactionRemove { removed_reaction } => {
      events::reaction_remove(ctx, database, removed_reaction).await?;
    }
    Event::Ready { .. } => {
      info!("Connected!");

      let default_activity_text = "Tracking your meditations";
      info!(
        "Setting default activity text: \"{}\"",
        default_activity_text
      );
      ctx.set_activity(Some(ActivityData::custom(default_activity_text)));
    }
    Event::VoiceStateUpdate { old, new } => {
      events::voice_state_update(ctx, data, old.as_ref(), new).await?;
    }
    _ => {}
  }
  Ok(())
}
