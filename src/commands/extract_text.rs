use anyhow::Result;
use poise::serenity_prelude::Message;
use poise::CreateReply;

use crate::config::EMOJI;
use crate::Context;

/// Extract body text from an AutoMod report or embedded message
///
/// Extracts body text from an AutoMod report or embedded message, making it possible to copy and paste on mobile.
///
/// To use, right-click a message, then go to "Apps" > "Extract Text".
#[poise::command(
  ephemeral,
  required_permissions = "MANAGE_MESSAGES",
  default_member_permissions = "MANAGE_MESSAGES",
  context_menu_command = "Extract Text",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn extract_text(
  ctx: Context<'_>,
  #[description = "Message to extract text from"] message: Message,
) -> Result<()> {
  let msg = if let Some(description) = &message.embeds[0].description {
    description
  } else if !message.content.is_empty() {
    &message.content
  } else {
    &format!("{} No message body.", EMOJI.mminfo)
  };
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}
