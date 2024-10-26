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
