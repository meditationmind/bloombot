use std::time::Duration;

use anyhow::{Result, anyhow};
use poise::serenity_prelude::{
  Channel, ChannelType, ComponentInteractionCollector, ComponentInteractionDataKind,
  CreateActionRow, CreateAllowedMentions, CreateInteractionResponse,
  CreateInteractionResponseMessage, CreateMessage, CreateSelectMenu, CreateSelectMenuKind,
  CreateThread, GetMessages, Mentionable, Message, MessageReference, MessageReferenceKind,
  MessageType,
};
use poise::{ApplicationContext, ChoiceParameter, Context as PoiseContext, CreateReply};
use tracing::warn;

use crate::config::EMOJI;
use crate::{Context, Data as AppData, Error as AppError};

#[derive(ChoiceParameter)]
enum Target {
  #[name = "Existing channel or thread"]
  ExistingChannel,
  #[name = "New public thread"]
  NewPublicThread,
}

/// Move a message
///
/// Moves a message to a different channel or public thread using forwarding and tags the original poster. Requires `Manage Messages` permissions.
///
/// To use, right-click the message that you want to move, then go to "Apps" > "Move Message".
#[poise::command(
  ephemeral,
  required_permissions = "MANAGE_MESSAGES",
  default_member_permissions = "MANAGE_MESSAGES",
  context_menu_command = "Move Message",
  category = "Context Menu Commands",
  guild_only
)]
pub async fn move_message(
  ctx: ApplicationContext<'_, AppData, AppError>,
  #[description = "The message to move"] message: Message,
) -> Result<()> {
  ctx.defer_ephemeral().await?;
  let ctx_id = ctx.id().to_string();

  let reply = {
    let channel_types = vec![ChannelType::Text, ChannelType::PublicThread];
    let channel_dropdown = vec![CreateActionRow::SelectMenu(
      CreateSelectMenu::new(
        ctx_id.as_str(),
        CreateSelectMenuKind::Channel {
          channel_types: Some(channel_types),
          default_channels: None,
        },
      )
      .min_values(1)
      .max_values(1)
      .placeholder("Choose a channel to move the message to"),
    )];

    CreateReply::default()
      .components(channel_dropdown)
      .ephemeral(true)
  };

  let msg = ctx.send(reply).await?;

  if let Some(mci) = ComponentInteractionCollector::new(ctx)
    .author_id(ctx.author().id)
    .channel_id(ctx.channel_id())
    .timeout(Duration::from_secs(20))
    .custom_ids(vec![ctx_id])
    .await
  {
    let choice = match &mci.data.kind {
      ComponentInteractionDataKind::ChannelSelect { values } => &values[0],
      _ => return Err(anyhow!("Unexpected interaction data kind")),
    };

    choice
      .send_message(
        ctx,
        CreateMessage::new()
          .content(format!("From {}:", &message.author.mention()))
          .allowed_mentions(CreateAllowedMentions::new().users([&message.author.id])),
      )
      .await?;

    choice
      .send_message(
        ctx,
        CreateMessage::new().reference_message(
          MessageReference::new(MessageReferenceKind::Forward, message.channel_id)
            .message_id(message.id)
            .fail_if_not_exists(false),
        ),
      )
      .await?;

    if let Err(e) = message.delete(ctx).await {
      warn!("Failed to delete original message: {e}");
      let msg = format!(
        "{} Original message could not be deleted. Please delete manually: {}",
        EMOJI.mminfo,
        message.link()
      );
      mci
        .create_response(
          ctx,
          CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
              .content(msg)
              .ephemeral(true),
          ),
        )
        .await?;
    }

    if let Err(e) = msg
      .edit(
        PoiseContext::Application(ctx),
        CreateReply::default()
          .content(format!(
            "{} Message successfully moved to <#{choice}>.",
            EMOJI.mmcheck
          ))
          .components(Vec::new()),
      )
      .await
    {
      // If the ephemeral response no longer exists, we get an Unknown
      // Message error, which we can safely ignore.
      if e.to_string() != "Unknown Message" {
        return Err(e.into());
      }
    }
    return Ok(());
  }

  msg
    .edit(
      PoiseContext::Application(ctx),
      CreateReply::default()
        .content(format!("{} Timed out waiting for response.", EMOJI.mminfo))
        .components(Vec::new()),
    )
    .await?;

  Ok(())
}

/// Move one or more messages
///
/// Moves one or more messages using forwarding and tags the original poster(s). Requires `Manage Messages` permissions.
///
/// Choose an existing channel or public thread as the target, or create a new public thread from a message and move up to 10 subsequent messages into the new thread.
#[poise::command(
  slash_command,
  required_permissions = "MANAGE_MESSAGES",
  default_member_permissions = "MANAGE_MESSAGES",
  category = "Moderator Commands",
  rename = "move",
  guild_only
)]
pub async fn move_messages(
  ctx: Context<'_>,
  #[description = "The (first) message to move"] message: Message,
  #[description = "Where to move the message(s)"] target: Target,
  #[channel_types("Text", "PublicThread")]
  #[description = "Existing channel or thread"]
  channel: Option<Channel>,
  #[min = 1]
  #[max = 10]
  #[description = "Number of additional messages to move (up to 10)"]
  number: Option<u8>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  let move_original = matches!(target, Target::ExistingChannel);
  let number = number.unwrap_or(0);
  let target = match target {
    Target::ExistingChannel => {
      let Some(channel) = channel else {
        let msg = format!("{} No existing channel or thread was selected.", EMOJI.mmx);
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      };
      channel.id()
    }
    Target::NewPublicThread => {
      if number < 1 {
        let msg = format!(
          "{} Must specify additional messages to create a thread.",
          EMOJI.mmx
        );
        ctx
          .send(CreateReply::default().content(msg).ephemeral(true))
          .await?;
        return Ok(());
      }
      let name = if message.content.chars().count() > 40 {
        let short_name = message.content.chars().take(40).collect::<String>();
        let short_name = short_name
          .rsplit_once(' ')
          .unwrap_or((short_name.as_str(), ""))
          .0;
        format!("{short_name}...")
      } else {
        message.content.clone()
      };
      let thread = message
        .channel_id
        .create_thread_from_message(ctx, message.id, CreateThread::new(name))
        .await?;
      thread.id
    }
  };
  let channel_id = message.channel_id;
  let message_id = message.id;
  let messages = if number == 0 {
    vec![message]
  } else {
    let mut messages = channel_id
      .messages(ctx, GetMessages::new().after(message_id).limit(number))
      .await?;
    if move_original {
      messages.push(message);
    }
    messages.reverse();
    messages
  };

  let mut num_moved = 0;

  for message in messages {
    if message.kind != MessageType::Regular {
      let msg = format!(
        "{} Unable to move message. Skipping: {}",
        EMOJI.mminfo,
        message.link()
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
      continue;
    }

    target
      .send_message(
        ctx,
        CreateMessage::new()
          .content(format!("From {}:", &message.author.mention()))
          .allowed_mentions(CreateAllowedMentions::new().users([&message.author.id])),
      )
      .await?;

    target
      .send_message(
        ctx,
        CreateMessage::new().reference_message(
          MessageReference::new(MessageReferenceKind::Forward, message.channel_id)
            .message_id(message.id)
            .fail_if_not_exists(false),
        ),
      )
      .await?;

    num_moved += 1;

    if let Err(e) = message.delete(ctx).await {
      warn!("Failed to delete message: {e}");
      let msg = format!(
        "{} Message could not be deleted. Please delete manually: {}",
        EMOJI.mminfo,
        message.link()
      );
      ctx
        .send(CreateReply::default().content(msg).ephemeral(true))
        .await?;
    }
  }

  let msg = format!(
    "{} {num_moved} message{} successfully moved to {}.",
    EMOJI.mmcheck,
    if num_moved == 1 { "" } else { "s" },
    target.mention()
  );
  ctx
    .send(CreateReply::default().content(msg).ephemeral(true))
    .await?;

  Ok(())
}
