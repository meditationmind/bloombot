#![allow(clippy::struct_excessive_bools, dead_code)]

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

impl TrackingProfile {
  pub fn new() -> Self {
    Self::default()
  }

  /// Manually assigns a user ID to a [`TrackingProfile`].
  pub fn user_id(mut self, user_id: u64) -> Self {
    self.user_id = serenity::UserId::new(user_id);
    self
  }

  /// Manually assigns a guild ID to a [`TrackingProfile`].
  pub fn guild_id(mut self, guild_id: u64) -> Self {
    self.guild_id = serenity::GuildId::new(guild_id);
    self
  }

  /// Manually assigns a UTC offset in number of minutes to a [`TrackingProfile`].
  /// If the specified offset is not valid, the [`TrackingProfile`] is returned unchanged.
  /// Valid offsets can be found in [`PlusOffsetChoice`][poc] and [`MinusOffsetChoice`][moc].
  ///
  /// [poc]: crate::commands::helpers::time::PlusOffsetChoice
  /// [moc]: crate::commands::helpers::time::MinusOffsetChoice
  pub fn utc_offset(mut self, utc_offset: i16) -> Self {
    if matches!(
      crate::commands::helpers::time::choice_from_offset(utc_offset),
      (None, None)
    ) {
      self
    } else {
      self.utc_offset = utc_offset;
      self
    }
  }

  /// Manually sets the state of anonymous tracking for a [`TrackingProfile`],
  /// with `true` for anonymous (private) reporting. Default is `false`.
  pub fn anonymous_tracking(mut self, anonymous_tracking: bool) -> Self {
    self.anonymous_tracking = anonymous_tracking;
    self
  }

  /// Manually sets the state of streak reporting for a [`TrackingProfile`],
  /// with `true` for active and `false` for inactive. Default is `true`.
  pub fn streaks_active(mut self, streaks_active: bool) -> Self {
    self.streaks_active = streaks_active;
    self
  }

  /// Manually sets streak privacy for a [`TrackingProfile`], with `true` for
  /// private and `false` for public. Default is `false`.
  pub fn streaks_private(mut self, streaks_private: bool) -> Self {
    self.streaks_private = streaks_private;
    self
  }

  /// Manually sets stats privacy for a [`TrackingProfile`], with `true` for
  /// private and `false` for public. Default is `false`.
  pub fn stats_private(mut self, stats_private: bool) -> Self {
    self.stats_private = stats_private;
    self
  }
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_tracking_profile_builder() {
    let profile1 = TrackingProfile {
      utc_offset: 180,
      streaks_private: true,
      stats_private: true,
      ..Default::default()
    };
    let profile2 = TrackingProfile::default()
      .utc_offset(180)
      .streaks_private(true)
      .stats_private(true);
    assert_eq!(profile1.utc_offset, profile2.utc_offset);
    assert_eq!(profile1.streaks_private, profile2.streaks_private);
    assert_eq!(profile1.stats_private, profile2.stats_private);

    assert_eq!(TrackingProfile::default().utc_offset, 0);
    assert_eq!(TrackingProfile::default().utc_offset(5).utc_offset, 0);
    assert_eq!(TrackingProfile::default().utc_offset(540).utc_offset, 540);
  }
}
