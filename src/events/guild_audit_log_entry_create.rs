use anyhow::Result;
use poise::serenity_prelude::{AuditLogEntry, Change, ChannelId, Context, CreateMessage};
use poise::serenity_prelude::{MemberAction, RoleId, audit_log::Action};

use crate::config::{BloomBotEmbed, CHANNELS, EMOJI, ROLES};

enum Event {
  BecamePatreonDonator,
  BecameKofiDonator,
}

impl Event {
  fn get_event(entry: &AuditLogEntry) -> Option<Self> {
    match entry.action {
      Action::Member(MemberAction::RoleUpdate) => {
        let Some(changes) = &entry.changes else {
          return None;
        };
        match &changes[0] {
          Change::RolesAdded { new, .. } => {
            let Some(roles) = new else {
              return None;
            };
            if roles[0].id == RoleId::from(ROLES.patreon) {
              Some(Self::BecamePatreonDonator)
            } else if roles[0].id == RoleId::from(ROLES.kofi) {
              Some(Self::BecameKofiDonator)
            } else {
              None
            }
          }
          _ => None,
        }
      }
      _ => None,
    }
  }
}

pub async fn guild_audit_log_entry_create(ctx: &Context, entry: &AuditLogEntry) -> Result<()> {
  let Some(event) = Event::get_event(entry) else {
    return Ok(());
  };

  match event {
    Event::BecamePatreonDonator => {
      let donator_channel = ChannelId::from(CHANNELS.donators);
      let embed = BloomBotEmbed::new()
        .title(":tada: New Donator :tada:")
        .description(format!(
          "Please welcome <@{}> as a new donator on Patreon.\n\nThank you for your generosity! It helps keep this community alive {}",
          entry.target_id.unwrap_or_default(), EMOJI.loveit
        ));

      donator_channel
        .send_message(&ctx, CreateMessage::new().embed(embed))
        .await?;
    }
    Event::BecameKofiDonator => {
      let donator_channel = ChannelId::from(CHANNELS.donators);
      let embed = BloomBotEmbed::new()
        .title(":tada: New Donator :tada:")
        .description(format!(
          "Please welcome <@{}> as a new donator on Ko-fi.\n\nThank you for your generosity! It helps keep this community alive {}",
          entry.target_id.unwrap_or_default(), EMOJI.loveit,
        ));

      donator_channel
        .send_message(&ctx, CreateMessage::new().embed(embed))
        .await?;
    }
  }

  Ok(())
}
