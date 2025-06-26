use poise::ChoiceParameter;
use poise::serenity_prelude::{ChannelId, GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::time;
use crate::data::common;
use crate::handlers::database::{DeleteQuery, InsertQuery, UpdateQuery};

#[derive(Debug, Clone, Copy, Default, PartialEq, ChoiceParameter)]
pub enum Privacy {
  #[name = "private"]
  Private,
  #[default]
  #[name = "public"]
  Public,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, ChoiceParameter)]
pub enum Status {
  #[default]
  #[name = "enabled"]
  Enabled,
  #[name = "disabled"]
  Disabled,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, ChoiceParameter)]
pub enum PrivateNotifications {
  #[name = "direct message"]
  DirectMessage,
  #[default]
  #[name = "private thread"]
  PrivateThread,
  #[name = "disabled"]
  Disabled,
}

#[derive(Debug)]
pub struct Tracking {
  pub privacy: Privacy,
}

#[derive(Debug)]
pub struct Streak {
  pub status: Status,
  pub privacy: Privacy,
}

#[derive(Debug)]
pub struct Stats {
  pub privacy: Privacy,
}

#[derive(Debug)]
pub struct TrackingProfile {
  pub user_id: UserId,
  pub guild_id: GuildId,
  pub utc_offset: i16,
  pub tracking: Tracking,
  pub streak: Streak,
  pub stats: Stats,
  pub vc_tracking: Option<Status>,
  pub notifications: PrivateNotifications,
  pub thread_id: Option<ChannelId>,
}

impl TrackingProfile {
  /// Creates a new [`TrackingProfile`] with a specified [`UserID`][uid]
  /// and [`GuildId`][gid]. All other values are set to their defaults.
  ///
  /// [uid]: poise::serenity_prelude::model::id::UserId
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  pub fn new(guild_id: impl Into<GuildId>, user_id: impl Into<UserId>) -> Self {
    Self {
      user_id: user_id.into(),
      guild_id: guild_id.into(),
      ..Default::default()
    }
  }

  /// Assigns a [`UserID`][uid] to a [`TrackingProfile`].
  ///
  /// [uid]: poise::serenity_prelude::model::id::UserId
  #[cfg(test)]
  pub fn with_user_id(mut self, user_id: impl Into<UserId>) -> Self {
    self.user_id = user_id.into();
    self
  }

  /// Assigns a [`GuildId`][gid] to a [`TrackingProfile`].
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  #[cfg(test)]
  pub fn with_guild_id(mut self, guild_id: impl Into<GuildId>) -> Self {
    self.guild_id = guild_id.into();
    self
  }

  /// Assigns a UTC offset, in number of minutes, to a [`TrackingProfile`].
  /// If the specified offset is not valid, the [`TrackingProfile`] is returned unchanged.
  /// Valid offsets can be found in [`PlusOffsetChoice`][poc] and [`MinusOffsetChoice`][moc].
  ///
  /// [poc]: crate::commands::helpers::time::PlusOffsetChoice
  /// [moc]: crate::commands::helpers::time::MinusOffsetChoice
  pub fn with_offset(mut self, utc_offset: i16) -> Self {
    if utc_offset != 0 && matches!(time::choice_from_offset(utc_offset), (None, None)) {
      self
    } else {
      self.utc_offset = utc_offset;
      self
    }
  }

  /// Sets tracking [`Privacy`] for a [`TrackingProfile`].
  /// Default is [`Privacy::Public`].
  pub fn with_tracking_privacy(mut self, privacy: Privacy) -> Self {
    self.tracking.privacy = privacy;
    self
  }

  /// Sets streak reporting [`Status`] for a [`TrackingProfile`].
  /// Default is [`Status::Enabled`].
  pub fn with_streak_status(mut self, status: Status) -> Self {
    self.streak.status = status;
    self
  }

  /// Sets streak [`Privacy`] for a [`TrackingProfile`].
  /// Default is [`Privacy::Public`].
  pub fn with_streak_privacy(mut self, privacy: Privacy) -> Self {
    self.streak.privacy = privacy;
    self
  }

  /// Sets stats [`Privacy`] for a [`TrackingProfile`].
  /// Default is [`Privacy::Public`].
  pub fn with_stats_privacy(mut self, privacy: Privacy) -> Self {
    self.stats.privacy = privacy;
    self
  }

  /// Sets VC tracking [`Status`] for a [`TrackingProfile`]. Default is `None`, which
  /// ensures the bot will offer to track individual sessions until a setting is chosen.
  pub fn with_vc_tracking(mut self, vc_tracking: Status) -> Self {
    self.vc_tracking = Some(vc_tracking);
    self
  }

  /// Sets the [`PrivateNotifications`] option for a [`TrackingProfile`].
  /// Default is [`PrivateNotifications::PrivateThread`].
  pub fn with_notifications(mut self, notifications: PrivateNotifications) -> Self {
    self.notifications = notifications;
    self
  }

  /// Sets the [`ChannelId`] of the private thread associated with a [`TrackingProfile`].
  /// Used for VC tracking, where ephemeral messages are not available. Primarily used with
  /// anonymous tracking, but may also be used with public tracking when [`Streak`] notifications
  /// are set to private.
  pub fn with_thread_id(mut self, thread_id: Option<ChannelId>) -> Self {
    self.thread_id = thread_id;
    self
  }

  /// Retrieves a [`TrackingProfile`] for a specified [`UserId`].
  pub fn retrieve<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT user_id, guild_id, utc_offset, anonymous_tracking, streaks_active, streaks_private, stats_private, vc_tracking, notifications, thread_id FROM tracking_profile WHERE user_id = $1 AND guild_id = $2",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }
}

impl InsertQuery for TrackingProfile {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    query!(
      "INSERT INTO tracking_profile (record_id, user_id, guild_id, utc_offset, anonymous_tracking, streaks_active, streaks_private, stats_private, vc_tracking, notifications, thread_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
      Ulid::new().to_string(),
      self.user_id.to_string(),
      self.guild_id.to_string(),
      self.utc_offset,
      privacy!(self.tracking.privacy),
      matches!(self.streak.status, Status::Enabled),
      privacy!(self.streak.privacy),
      privacy!(self.stats.privacy),
      self.vc_tracking.map(|vc_t| matches!(vc_t, Status::Enabled)),
      match self.notifications {
        PrivateNotifications::DirectMessage => "dm",
        PrivateNotifications::PrivateThread => "thread",
        PrivateNotifications::Disabled => "off",
      },
      self.thread_id.map(|id| id.to_string()),
    )
  }
}

impl UpdateQuery for TrackingProfile {
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    query!(
      "UPDATE tracking_profile SET utc_offset = $1, anonymous_tracking = $2, streaks_active = $3, streaks_private = $4, stats_private = $5, vc_tracking = $6, notifications = $7, thread_id = $8 WHERE user_id = $9 AND guild_id = $10",
      self.utc_offset,
      privacy!(self.tracking.privacy),
      matches!(self.streak.status, Status::Enabled),
      privacy!(self.streak.privacy),
      privacy!(self.stats.privacy),
      self.vc_tracking.map(|vc_t| matches!(vc_t, Status::Enabled)),
      match self.notifications {
        PrivateNotifications::DirectMessage => "dm",
        PrivateNotifications::PrivateThread => "thread",
        PrivateNotifications::Disabled => "off",
      },
      self.thread_id.map(|id| id.to_string()),
      self.user_id.to_string(),
      self.guild_id.to_string(),
    )
  }
}

impl DeleteQuery for TrackingProfile {
  fn delete_query<'a>(
    guild_id: GuildId,
    user_id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    query!(
      "DELETE FROM tracking_profile WHERE user_id = $1 AND guild_id = $2",
      user_id.into(),
      guild_id.to_string(),
    )
  }
}

//Default values for tracking customization
impl Default for TrackingProfile {
  fn default() -> Self {
    Self {
      user_id: UserId::default(),
      guild_id: GuildId::default(),
      utc_offset: 0,
      tracking: Tracking {
        privacy: Privacy::Public,
      },
      streak: Streak {
        status: Status::Enabled,
        privacy: Privacy::Public,
      },
      stats: Stats {
        privacy: Privacy::Public,
      },
      vc_tracking: None,
      notifications: PrivateNotifications::PrivateThread,
      thread_id: None,
    }
  }
}

impl FromRow<'_, PgRow> for TrackingProfile {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let user_id = UserId::new(common::decode_id_row(row, "user_id")?);
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);
    let tracking_privacy = if row.try_get::<bool, &str>("anonymous_tracking")? {
      Privacy::Private
    } else {
      Privacy::Public
    };
    let streak_status = if row.try_get::<bool, &str>("streaks_active")? {
      Status::Enabled
    } else {
      Status::Disabled
    };
    let streak_privacy = if row.try_get::<bool, &str>("streaks_private")? {
      Privacy::Private
    } else {
      Privacy::Public
    };
    let stats_privacy = if row.try_get::<bool, &str>("stats_private")? {
      Privacy::Private
    } else {
      Privacy::Public
    };
    let vc_tracking = if let Some(vc_t) = row.try_get::<Option<bool>, &str>("vc_tracking")? {
      if vc_t {
        Some(Status::Enabled)
      } else {
        Some(Status::Disabled)
      }
    } else {
      None
    };
    let notifications = {
      let value = row.try_get::<String, &str>("notifications")?;
      match value {
        val if val == "dm" => PrivateNotifications::DirectMessage,
        val if val == "thread" => PrivateNotifications::PrivateThread,
        val if val == "off" => PrivateNotifications::Disabled,
        _ => PrivateNotifications::PrivateThread,
      }
    };
    let thread_id = common::decode_option_id_row(row, "thread_id")?.map(ChannelId::new);

    Ok(Self {
      user_id,
      guild_id,
      utc_offset: row.try_get("utc_offset").unwrap_or_default(),
      tracking: Tracking {
        privacy: tracking_privacy,
      },
      streak: Streak {
        status: streak_status,
        privacy: streak_privacy,
      },
      stats: Stats {
        privacy: stats_privacy,
      },
      vc_tracking,
      notifications,
      thread_id,
    })
  }
}

/// Takes [`Privacy`][priv] as an argument and returns `true` for [`Privacy::Private`]
/// or `false` for [`Privacy::Public`].
///
/// [`Option<Privacy>`][priv] can also be passed as an argument, along with a default value
/// of type [`Privacy`][priv], specified via a second argument. If [`Option<Privacy>`][priv]
/// is `Some`, the same matching as above is applied to the unwrapped value. If `None`, the
/// matching is applied to the default value. This means the first value takes precedence.
///
/// In most cases, the default value should be taken from the user's [`TrackingProfile`][tp].
///
/// # Examples
///
/// ```rust
/// let privacy = Privacy::Private;
/// assert!(privacy!(privacy));
///
/// let privacy = None;
/// let profile = TrackingProfile::default().streak_privacy(Privacy::Private);
/// assert!(privacy!(privacy, profile.streak.privacy));
///
/// let privacy = Privacy::Public;
/// let profile = TrackingProfile::default().streak_privacy(Privacy::Private);
/// assert!(!(privacy!(privacy, profile.streak.privacy)));
/// ```
///
/// [priv]: crate::data::tracking_profile::Privacy
/// [tp]: crate::data::tracking_profile::TrackingProfile
macro_rules! privacy {
  ($privacy:expr, $default:expr) => {
    match $privacy {
      Some(privacy) => match privacy {
        Privacy::Private => true,
        Privacy::Public => false,
      },
      None => match $default {
        Privacy::Private => true,
        Privacy::Public => false,
      },
    }
  };
  ($privacy:expr) => {
    match $privacy {
      Privacy::Private => true,
      Privacy::Public => false,
    }
  };
}
pub(crate) use privacy;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_builder() {
    let profile1 = TrackingProfile {
      utc_offset: 180,
      streak: Streak {
        status: Status::Enabled,
        privacy: Privacy::Private,
      },
      stats: Stats {
        privacy: Privacy::Private,
      },
      ..Default::default()
    };
    let profile2 = TrackingProfile::default()
      .with_offset(180)
      .with_streak_privacy(Privacy::Private)
      .with_stats_privacy(Privacy::Private);
    assert_eq!(profile1.utc_offset, profile2.utc_offset);
    assert_eq!(profile1.streak.privacy, profile2.streak.privacy);
    assert_eq!(profile1.stats.privacy, profile2.stats.privacy);

    assert_eq!(TrackingProfile::default().utc_offset, 0);
    assert_eq!(TrackingProfile::default().with_offset(5).utc_offset, 0);
    assert_eq!(TrackingProfile::default().with_offset(540).utc_offset, 540);
  }

  #[test]
  #[allow(clippy::unreadable_literal)]
  fn test_id_methods() {
    let guild_id = GuildId::new(1300863845429936139);
    let profile = TrackingProfile::default().with_guild_id(guild_id);
    assert_eq!(profile.guild_id, GuildId::new(1300863845429936139));

    let int_user_id = 1300863845429936139;
    let str_guild_id = 1300863845429936139;

    let profile = TrackingProfile::default()
      .with_user_id(int_user_id)
      .with_guild_id(str_guild_id);
    assert_eq!(profile.user_id, UserId::new(1300863845429936139));
    assert_eq!(profile.guild_id, GuildId::new(1300863845429936139));

    let profile = TrackingProfile::new(str_guild_id, int_user_id);
    assert_eq!(profile.user_id, UserId::new(1300863845429936139));
    assert_eq!(profile.guild_id, GuildId::new(1300863845429936139));
  }

  #[test]
  fn test_privacy_macro() {
    let profile = TrackingProfile::default().with_streak_privacy(Privacy::Private);
    assert!(privacy!(Privacy::Private));
    assert!(!(privacy!(Some(Privacy::Public), profile.streak.privacy)));
    assert!(privacy!(None, profile.streak.privacy));
  }
}
