#![allow(clippy::struct_excessive_bools)]

use poise::serenity_prelude::{self as serenity};

#[derive(Debug)]
pub struct TrackingProfile {
  pub user_id: serenity::UserId,
  pub guild_id: serenity::GuildId,
  pub utc_offset: i16,
  pub anonymous_tracking: bool,
  pub streaks_active: bool,
  pub streaks_private: bool,
  pub stats_private: bool,
}

//Default values for tracking customization
impl Default for TrackingProfile {
  fn default() -> Self {
    Self {
      user_id: serenity::UserId::default(),
      guild_id: serenity::GuildId::default(),
      utc_offset: 0,
      anonymous_tracking: false,
      streaks_active: true,
      streaks_private: false,
      stats_private: false,
    }
  }
}
