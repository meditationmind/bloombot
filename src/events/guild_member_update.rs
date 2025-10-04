use anyhow::Result;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, Member};

use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ROLES};

enum UpdateType {
  StoppedPending,
}

impl UpdateType {
  fn get_type(old: &Member, new: &Member) -> Option<Self> {
    if old.pending && !new.pending {
      Some(Self::StoppedPending)
    } else {
      None
    }
  }
}

pub async fn guild_member_update(
  ctx: &Context,
  old: Option<&Member>,
  new: Option<&Member>,
) -> Result<()> {
  let Some(old) = old else { return Ok(()) };
  let Some(new) = new else { return Ok(()) };
  let Some(update_type) = UpdateType::get_type(old, new) else {
    return Ok(());
  };

  match update_type {
    UpdateType::StoppedPending => {
      let welcome_channel = ChannelId::from(CHANNELS.welcome);
      let msg = format!(
        "Please give <@{}> a warm welcome, {}!",
        new.user.id, ROLES.welcome_team
      );
      let embed = BloomBotEmbed::new()
        .title(":tada: A new member has arrived! :tada:")
        .description(format!(
          "Welcome to the Meditation Mind community, <@{}>!\n\nCheck out <id:customize> to grab some roles and [customize your community experience](<https://meditationmind.org/curating-your-experience/>).\n\nWe're glad you've joined us! {}",
          new.user.id, EMOJI.aww
        ))
        .thumbnail(
          "https://meditationmind.org/wp-content/uploads/2020/04/Webp.net-resizeimage-1.png",
        );

      welcome_channel
        .send_message(&ctx, CreateMessage::new().content(msg).embed(embed))
        .await?;
    }
  }

  Ok(())
}
