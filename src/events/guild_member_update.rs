use anyhow::Result;
use poise::serenity_prelude::{ChannelId, Context, CreateMessage, Member, RoleId};

use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ROLES};

enum UpdateType {
  BecamePatreonDonator,
  BecameKofiDonator,
  StoppedPending,
}

impl UpdateType {
  fn get_type(old: &Member, new: &Member) -> Option<Self> {
    let patreon_role = RoleId::new(ROLES.patreon);
    let kofi_role = RoleId::new(ROLES.kofi);

    if old.pending && !new.pending {
      Some(Self::StoppedPending)
    } else if !old.roles.contains(&patreon_role) && new.roles.contains(&patreon_role) {
      Some(Self::BecamePatreonDonator)
    } else if !old.roles.contains(&kofi_role) && new.roles.contains(&kofi_role) {
      Some(Self::BecameKofiDonator)
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
      let welcome_channel = ChannelId::new(CHANNELS.welcome);
      let msg = format!(
        "Please give <@{}> a warm welcome, <@&{}>!",
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
    UpdateType::BecamePatreonDonator => {
      let donator_channel = ChannelId::new(CHANNELS.donators);
      let embed = BloomBotEmbed::new()
        .title(":tada: New Donator :tada:")
        .description(format!(
          "Please welcome <@{}> as a new donator on Patreon.\n\nThank you for your generosity! It helps keep this community alive {}",
          new.user.id, EMOJI.loveit
        ));

      donator_channel
        .send_message(&ctx, CreateMessage::new().embed(embed))
        .await?;
    }
    UpdateType::BecameKofiDonator => {
      let donator_channel = ChannelId::new(CHANNELS.donators);
      let embed = BloomBotEmbed::new()
        .title(":tada: New Donator :tada:")
        .description(format!(
          "Please welcome <@{}> as a new donator on Ko-fi.\n\nThank you for your generosity! It helps keep this community alive {}",
          new.user.id, EMOJI.loveit,
        ));

      donator_channel
        .send_message(&ctx, CreateMessage::new().embed(embed))
        .await?;
    }
  }

  Ok(())
}
