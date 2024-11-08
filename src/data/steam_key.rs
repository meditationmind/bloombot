use poise::serenity_prelude::{GuildId, Mentionable, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{FromRow, Postgres, Row};
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
  /// Creates a new [`SteamKey`] with a specified [`GuildId`] and `key`,
  /// setting all other fields to their defaults.
  pub fn new(guild_id: GuildId, key: impl Into<String>) -> Self {
    Self {
      guild_id,
      key: key.into(),
      ..Default::default()
    }
  }

  /// Marks a [`SteamKey`] as reserved for a user.
  pub fn reserve<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "UPDATE steamkey SET reserved = $1 WHERE steam_key = (SELECT steam_key FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $2 ORDER BY RANDOM() LIMIT 1) RETURNING steam_key",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Marks a [`SteamKey`] as unreserved.
  pub fn unreserve(key: &str) -> Query<'_, Postgres, PgArguments> {
    sqlx::query!(
      "UPDATE steamkey SET reserved = NULL WHERE steam_key = $1",
      key,
    )
  }

  /// Marks a [`SteamKey`] as used.
  pub fn mark_used(key: &str) -> Query<'_, Postgres, PgArguments> {
    sqlx::query!("UPDATE steamkey SET used = TRUE WHERE steam_key = $1", key,)
  }

  /// Retrieves a [`SteamKey`] and marks it as used.
  pub fn consume<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "UPDATE steamkey SET used = TRUE WHERE steam_key = (SELECT steam_key FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $1 ORDER BY RANDOM() LIMIT 1) RETURNING steam_key",
    )
    .bind(guild_id.to_string())
  }

  /// Retrieves all [`SteamKey`]s from the database.
  pub fn retrieve_all<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as("SELECT steam_key, reserved, used, guild_id FROM steamkey WHERE guild_id = $1")
      .bind(guild_id.to_string())
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
  /// Adds a [`SteamKey`] to the database.
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
  /// Deletes a [`SteamKey`] from the database.
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
  type Item<'a> = Option<&'a str>;

  /// If `key` is [`Some<&str>`], checks the database to see if the specified [`SteamKey`]
  /// exists. If `key` is [`None`], checks to see if an unused [`SteamKey`] exists.
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    key: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    match key {
      Some(key) => sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM steamkey WHERE steam_key = $1 AND guild_id = $2)",
      )
      .bind(key)
      .bind(guild_id.to_string()),
      None => sqlx::query_as(
        "SELECT EXISTS(SELECT 1 FROM steamkey WHERE used = FALSE AND reserved IS NULL AND guild_id = $1)",
      )
      .bind(guild_id.to_string()),
    }
  }
}

impl FromRow<'_, PgRow> for SteamKey {
  fn from_row(row: &'_ PgRow) -> sqlx::Result<Self, sqlx::Error> {
    let guild_id: String = row.try_get("guild_id").unwrap_or("1".to_string());
    let guild_id = match guild_id.parse::<u64>() {
      Ok(id) => GuildId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "guild_id".to_string(),
          source: Box::new(e),
        })
      }
    };
    let reserved = match row.try_get::<String, &str>("reserved") {
      Ok(string_id) => match string_id.parse::<u64>() {
        Ok(id) => Some(UserId::new(id)),
        Err(e) => {
          return Err(sqlx::Error::ColumnDecode {
            index: "reserved".to_string(),
            source: Box::new(e),
          })
        }
      },
      Err(_) => None,
    };

    Ok(Self {
      guild_id,
      key: row.try_get("steam_key").unwrap_or_default(),
      used: row.try_get("used").unwrap_or_default(),
      reserved,
    })
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

  /// Retrieves a single [`Recipient`] from the database.
  pub fn retrieve_one<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT user_id, guild_id, challenge_prize, donator_perk, total_keys FROM steamkey_recipients WHERE user_id = $1 AND guild_id = $2",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves all [`Recipient`]s from the database.
  pub fn retrieve_all<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT user_id, guild_id, challenge_prize, donator_perk, total_keys FROM steamkey_recipients WHERE guild_id = $1",
    )
    .bind(guild_id.to_string())
  }

  /// Records a key redemption for a [`Recipient`] who won the monthly challenge and
  /// chose to accept the prize. If the recipient already exists in the database, the
  /// total keys will be updated. Otherwise, a new record will be added.
  pub fn record_win<'a>(
    guild_id: GuildId,
    user_id: UserId,
    exists: bool,
  ) -> Query<'a, Postgres, PgArguments> {
    if exists {
      sqlx::query!(
        "UPDATE steamkey_recipients SET challenge_prize = TRUE, total_keys = total_keys + 1 WHERE user_id = $1 AND guild_id = $2",
        user_id.to_string(),
        guild_id.to_string(),
      )
    } else {
      sqlx::query!(
        "INSERT INTO steamkey_recipients (record_id, user_id, guild_id, challenge_prize, total_keys) VALUES ($1, $2, $3, TRUE, 1)",
        Ulid::new().to_string(),
        user_id.to_string(),
        guild_id.to_string(),
      )
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
  /// Adds a Steam key [`Recipient`] to the database.
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO steamkey_recipients (record_id, user_id, guild_id, challenge_prize, donator_perk, total_keys) VALUES ($1, $2, $3, $4, $5, $6)",
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
  /// Updates Steam key [`Recipient`] details in the database.
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "UPDATE steamkey_recipients SET challenge_prize = $1, donator_perk = $2, total_keys = $3 WHERE user_id = $4 AND guild_id = $5",
      self.challenge_prize,
      self.donator_perk,
      self.total_keys,
      self.user_id.to_string(),
      self.guild_id.to_string(),
    )
  }
}

impl DeleteQuery for Recipient {
  /// Deletes a Steam key [`Recipient`] from the database.
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
  type Item<'a> = UserId;

  /// Checks the database to see if a Steam key [`Recipient`] exists.
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    user_id: Self::Item<'_>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as(
      "SELECT EXISTS(SELECT 1 FROM steamkey_recipients WHERE guild_id = $1 AND user_id = $2)",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
  }
}

impl FromRow<'_, PgRow> for Recipient {
  fn from_row(row: &'_ PgRow) -> sqlx::Result<Self, sqlx::Error> {
    let guild_id: String = row.try_get("guild_id").unwrap_or("1".to_string());
    let guild_id = match guild_id.parse::<u64>() {
      Ok(id) => GuildId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "guild_id".to_string(),
          source: Box::new(e),
        })
      }
    };
    let user_id: String = row.try_get("user_id").unwrap_or("1".to_string());
    let user_id = match user_id.parse::<u64>() {
      Ok(id) => UserId::new(id),
      Err(e) => {
        return Err(sqlx::Error::ColumnDecode {
          index: "user_id".to_string(),
          source: Box::new(e),
        })
      }
    };

    Ok(Self {
      guild_id,
      user_id,
      challenge_prize: row.try_get("challenge_prize").unwrap_or_default(),
      donator_perk: row.try_get("donator_perk").unwrap_or_default(),
      total_keys: row.try_get("total_keys").unwrap_or_default(),
    })
  }
}
