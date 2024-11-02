use crate::commands::helpers::pagination::{PageRow, PageType};
use chrono::Utc;
use poise::serenity_prelude::{self as serenity};

pub struct Erase {
  pub id: String,
  pub user_id: serenity::UserId,
  pub message_link: String,
  pub reason: String,
  pub occurred_at: chrono::DateTime<Utc>,
}

impl Erase {
  /// Creates a new [`Erase`] with a specified [`UserID`][uid].
  /// All other values are set to their defaults.
  ///
  /// [uid]: poise::serenity_prelude::model::id::UserId
  pub fn new(user_id: impl Into<serenity::UserId>) -> Self {
    Self {
      user_id: user_id.into(),
      ..Default::default()
    }
  }

  /// Assigns a [`UserID`][uid] to an [`Erase`].
  ///
  /// [uid]: poise::serenity_prelude::model::id::UserId
  pub fn user_id(mut self, user_id: impl Into<serenity::UserId>) -> Self {
    self.user_id = user_id.into();
    self
  }

  /// Sets the erase notification message link for an [`Erase`].
  pub fn link(mut self, message_link: impl Into<String>) -> Self {
    self.message_link = message_link.into();
    self
  }

  /// Sets the reason for an [`Erase`].
  pub fn reason(mut self, reason: impl Into<String>) -> Self {
    self.reason = reason.into();
    self
  }

  /// Sets the time and date when an [`Erase`] occurred.
  pub fn datetime(mut self, datetime: impl Into<chrono::DateTime<Utc>>) -> Self {
    self.occurred_at = datetime.into();
    self
  }
}

impl Default for Erase {
  fn default() -> Self {
    Self {
      id: String::default(),
      user_id: serenity::UserId::default(),
      message_link: "None".to_string(),
      reason: "No reason provided.".to_string(),
      occurred_at: chrono::DateTime::<Utc>::default(),
    }
  }
}

impl PageRow for Erase {
  fn title(&self, page_type: PageType) -> String {
    match page_type {
      PageType::Standard => {
        if self.occurred_at == (chrono::DateTime::<Utc>::default()) {
          "Date: `Not Available`".to_owned()
        } else {
          format!("Date: `{}`", self.occurred_at.format("%Y-%m-%d %H:%M"))
        }
      }
      PageType::Alternate => {
        if self.occurred_at == (chrono::DateTime::<Utc>::default()) {
          "Date: `Not Available`".to_owned()
        } else {
          format!("Date: `{}`", self.occurred_at.format("%e %B %Y %H:%M"))
        }
      }
    }
  }

  fn body(&self) -> String {
    if self.message_link == "None" {
      format!("**Reason:** {}\n-# Notification not available", self.reason)
    } else {
      format!(
        "**Reason:** {}\n[Go to erase notification]({})",
        self.reason, self.message_link
      )
    }
  }
}
