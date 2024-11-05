use poise::serenity_prelude::{ChannelId, MessageId};

#[allow(clippy::struct_field_names)]
pub struct StarMessage {
  pub record_id: String,
  pub starred_message_id: MessageId,
  pub board_message_id: MessageId,
  pub starred_channel_id: ChannelId,
}
