use poise::serenity_prelude::{self as serenity};

#[allow(clippy::struct_field_names)]
pub struct StarMessage {
  pub record_id: String,
  pub starred_message_id: serenity::MessageId,
  pub board_message_id: serenity::MessageId,
  pub starred_channel_id: serenity::ChannelId,
}
