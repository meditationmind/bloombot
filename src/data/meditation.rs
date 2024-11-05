use chrono::{DateTime, Utc};
use poise::serenity_prelude::UserId;

use crate::commands::helpers::pagination::{PageRow, PageType};

pub struct Meditation {
  pub id: String,
  pub user_id: UserId,
  pub minutes: i32,
  pub seconds: i32,
  pub occurred_at: DateTime<Utc>,
}

impl PageRow for Meditation {
  fn title(&self, _page_type: PageType) -> String {
    if self.seconds > 0 {
      format!(
        "{} {} {} {}",
        self.minutes,
        if self.minutes == 1 {
          "minute"
        } else {
          "minutes"
        },
        self.seconds,
        if self.seconds == 1 {
          "second"
        } else {
          "seconds"
        },
      )
    } else {
      format!(
        "{} {}",
        self.minutes,
        if self.minutes == 1 {
          "minute"
        } else {
          "minutes"
        },
      )
    }
  }

  fn body(&self) -> String {
    format!(
      "Date: `{}`\nID: `{}`",
      self.occurred_at.format("%Y-%m-%d %H:%M"),
      self.id
    )
  }
}
