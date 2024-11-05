use poise::serenity_prelude::{GuildId, Mentionable, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{FromRow, Postgres};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{DeleteQuery, ExistsQuery, InsertQuery, UpdateQuery};

#[derive(Default)]
pub struct SteamKey {
  pub guild_id: GuildId,
  pub key: String,
  pub used: bool,
  pub reserved: Option<UserId>,
}

#[derive(Default)]
pub struct Recipient {
  pub guild_id: GuildId,
  pub user_id: UserId,
  pub challenge_prize: Option<bool>,
  pub donator_perk: Option<bool>,
  pub total_keys: i16,
}

impl SteamKey {
  pub fn new(guild_id: GuildId, key: impl Into<String>) -> Self {
    Self {
      guild_id,
      key: key.into(),
      ..Default::default()
    }
  }

  pub fn set_used(mut self, used: bool) -> Self {
    self.used = used;
    self
  }

  pub fn reserved_for(mut self, reserved: UserId) -> Self {
    self.reserved = Some(reserved);
    self
  }
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

impl InsertQuery for SteamKey {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO steamkey (record_id, steam_key, guild_id, used) VALUES ($1, $2, $3, $4)",
      Ulid::new().to_string(),
      self.key,
      self.guild_id.to_string(),
      self.used,
    )
  }
}

impl DeleteQuery for SteamKey {
  fn delete_query<'a>(
    guild_id: GuildId,
    key: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      "DELETE FROM steamkey WHERE steam_key = $1 AND guild_id = $2",
      key.into(),
      guild_id.to_string(),
    )
  }
}

impl ExistsQuery for SteamKey {
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    key: impl Into<String>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
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

impl Recipient {
  pub fn new(
    guild_id: GuildId,
    user_id: UserId,
    challenge_prize: Option<bool>,
    donator_perk: Option<bool>,
    total_keys: i16,
  ) -> Self {
    Self {
      guild_id,
      user_id,
      challenge_prize,
      donator_perk,
      total_keys,
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

impl InsertQuery for Recipient {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "
        INSERT INTO
          steamkey_recipients (
            record_id,
            user_id,
            guild_id,
            challenge_prize,
            donator_perk,
            total_keys
          )
        VALUES
          ($1, $2, $3, $4, $5, $6)
      ",
      Ulid::new().to_string(),
      self.user_id.to_string(),
      self.guild_id.to_string(),
      self.challenge_prize,
      self.donator_perk,
      self.total_keys
    )
  }
}

impl UpdateQuery for Recipient {
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "
        UPDATE steamkey_recipients
        SET challenge_prize = $1, donator_perk = $2, total_keys = $3
        WHERE user_id = $4 AND guild_id = $5
      ",
      self.challenge_prize,
      self.donator_perk,
      self.total_keys,
      self.user_id.to_string(),
      self.guild_id.to_string(),
    )
  }
}

impl DeleteQuery for Recipient {
  fn delete_query<'a>(
    guild_id: GuildId,
    user_id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      "DELETE FROM steamkey_recipients WHERE user_id = $1 AND guild_id = $2",
      user_id.into(),
      guild_id.to_string(),
    )
  }
}

impl ExistsQuery for Recipient {
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    user_id: impl Into<String>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as(
      "SELECT EXISTS (SELECT 1 FROM steamkey_recipients WHERE guild_id = $1 AND user_id = $2)",
    )
    .bind(guild_id.to_string())
    .bind(user_id.into())
  }
}
