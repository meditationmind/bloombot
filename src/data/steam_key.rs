use crate::{
  commands::helpers::pagination::{PageRow, PageType},
  handlers::database::ExistsQuery,
};
use poise::serenity_prelude::{self as serenity, Mentionable};

pub struct SteamKey {
  pub key: String,
  pub used: bool,
  pub reserved: Option<serenity::UserId>,
  pub guild_id: serenity::GuildId,
}

pub struct Recipient {
  pub user_id: serenity::UserId,
  pub guild_id: serenity::GuildId,
  pub challenge_prize: Option<bool>,
  pub donator_perk: Option<bool>,
  pub total_keys: i16,
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

impl ExistsQuery for SteamKey {
  fn exists_query<'a, T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>>(
    guild_id: serenity::GuildId,
    key: impl Into<String>,
  ) -> sqlx::query::QueryAs<'a, sqlx::Postgres, T, sqlx::postgres::PgArguments> {
    let key: String = key.into();
    if key == "none" {
      sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $1)",
      )
      .bind(guild_id.to_string())
    } else {
      sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM steamkey WHERE steam_key = $1 AND guild_id = $2)",
      )
      .bind(key)
      .bind(guild_id.to_string())
    }
  }
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

impl ExistsQuery for Recipient {
  fn exists_query<'a, T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>>(
    guild_id: serenity::GuildId,
    user_id: impl Into<String>,
  ) -> sqlx::query::QueryAs<'a, sqlx::Postgres, T, sqlx::postgres::PgArguments> {
    sqlx::query_as(
      "SELECT EXISTS (SELECT 1 FROM steamkey_recipients WHERE guild_id = $1 AND user_id = $2)",
    )
    .bind(guild_id.to_string())
    .bind(user_id.into())
  }
}
