use anyhow::Result;
use poise::serenity_prelude::{builder::*, ChannelId, Message};
use poise::CreateReply;

use crate::config::{BloomBotEmbed, CHANNELS, ROLES};
use crate::Context;

/// Report a message to server staff
///
/// Reports a message to server staff.
///
/// To use, right-click the message that you want to report, then go to "Apps" > "Report Message".
#[poise::command(
  ephemeral,
  context_menu_command = "Report Message",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn report_message(
  ctx: Context<'_>,
  #[description = "Message to report"] message: Message,
) -> Result<()> {
  let reporting_user = ctx.author();
  let report_channel_id = ChannelId::new(CHANNELS.reportchannel);
  let message_link = message.link().clone();
  let message_user = message.author;
  let message_channel_name = message.channel_id.name(ctx).await?;

  let message_content = if message.content.is_empty() {
    match message.attachments.first() {
      Some(attachment) => format!("**Attachment**\n{}", attachment.url),
      None => String::new(),
    }
  } else {
    message.content
  };

  let embed = BloomBotEmbed::new()
    .author(CreateEmbedAuthor::new(&message_user.name).icon_url(message_user.face()))
    .description(message_content)
    .field("Link", format!("[Go to message]({message_link})"), false)
    .footer(CreateEmbedFooter::new(format!(
      "Author ID: {}\nReported via context menu in #{} by {} ({})",
      &message_user.id, message_channel_name, reporting_user.name, reporting_user.id
    )))
    .timestamp(message.timestamp);

  report_channel_id
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(format!("<@&{}> Message Reported", ROLES.staff))
        .embed(embed),
    )
    .await?;

  ctx
    .send(
      CreateReply::default()
        .content("Your report has been sent to the moderation team.")
        .ephemeral(true),
    )
    .await?;

  Ok(())
}
