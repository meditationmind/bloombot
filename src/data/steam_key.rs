use crate::commands::helpers::pagination::{PageRow, PageType};
use poise::serenity_prelude::{self as serenity, Mentionable};

pub struct SteamKey {
  pub key: String,
  pub used: bool,
  pub reserved: Option<serenity::UserId>,
  pub guild_id: serenity::GuildId,
}

impl PageRow for SteamKey {
  fn title(&self, _page_type: PageType) -> String {
    self.key.clone()
  }

  fn body(&self) -> String {
    format!(
      "Used: {}\nReserved for: {}",
      if self.used { "Yes" } else { "No" },
      match self.reserved {
        Some(reserved) => reserved.mention().to_string(),
        None => "Nobody".to_owned(),
      },
    )
  }
}

pub struct Recipient {
  pub user_id: serenity::UserId,
  pub guild_id: serenity::GuildId,
  pub challenge_prize: Option<bool>,
  pub donator_perk: Option<bool>,
  pub total_keys: i16,
}

impl PageRow for Recipient {
  fn title(&self, _page_type: PageType) -> String {
    "__Recipient__".to_owned()
  }

  fn body(&self) -> String {
    format!(
      "Name: {}\nDonator Perk: {}\nChallenge Prize: {}\nTotal Keys: {}",
      self.user_id.mention(),
      match self.donator_perk {
        Some(value) =>
          if value {
            "Yes"
          } else {
            "No"
          },
        None => "No",
      },
      match self.challenge_prize {
        Some(value) =>
          if value {
            "Yes"
          } else {
            "No"
          },
        None => "No",
      },
      self.total_keys,
    )
  }
}
