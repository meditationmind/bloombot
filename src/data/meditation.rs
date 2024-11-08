use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::PgRow;
use sqlx::query::{Query, QueryAs};
use sqlx::{postgres::PgArguments, FromRow, Postgres, Row};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{DeleteQuery, InsertQuery, UpdateQuery};

#[derive(Default)]
pub struct Meditation {
  pub id: String,
  pub guild_id: GuildId,
  pub user_id: UserId,
  pub minutes: i32,
  pub seconds: i32,
  pub occurred_at: DateTime<Utc>,
}

impl Meditation {
  /// Creates a new [`Meditation`] with the specified values and an automatically
  /// generated [`ULID`][ulid].
  ///
  /// [ulid]: https://github.com/ulid/spec
  pub fn new(
    guild_id: GuildId,
    user_id: UserId,
    minutes: i32,
    seconds: i32,
    datetime: &DateTime<Utc>,
  ) -> Self {
    Self {
      id: Ulid::new().to_string(),
      guild_id,
      user_id,
      minutes,
      seconds,
      occurred_at: *datetime,
    }
  }

  /// Creates a new [`Meditation`] with the specified `minutes`, `seconds`, and `datetime`,
  /// taking all other values from `self`. Used for updating a meditation entry, while still
  /// being able to reference the previous values.
  pub fn with_new(&self, minutes: i32, seconds: i32, datetime: &DateTime<Utc>) -> Meditation {
    Meditation {
      id: self.id.clone(),
      guild_id: self.guild_id,
      user_id: self.user_id,
      minutes,
      seconds,
      occurred_at: *datetime,
    }
  }

  /// Retrieves a [`Meditation`] entry from the database with all fields populated.
  pub fn full_entry(
    guild_id: GuildId,
    meditation_id: &str,
  ) -> QueryAs<'_, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, guild_id, user_id, meditation_minutes, meditation_seconds, occurred_at FROM meditation WHERE record_id = $1 AND guild_id = $2",
    )
    .bind(meditation_id)
    .bind(guild_id.to_string())
  }

  /// Retrieves a user's most recent [`Meditation`] entry from the database.
  pub fn latest_entry<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, meditation_minutes, meditation_seconds, occurred_at FROM meditation WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC LIMIT 1",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves all [`Meditation`] entries for a user from the database.
  pub fn user_entries<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, meditation_minutes, meditation_seconds, occurred_at FROM meditation WHERE user_id = $1 AND guild_id = $2 ORDER BY occurred_at DESC",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Removes all [`Meditation`] entries for a user from the database.
  pub fn remove_all<'a>(guild_id: GuildId, user_id: UserId) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      "DELETE FROM meditation WHERE user_id = $1 AND guild_id = $2",
      user_id.to_string(),
      guild_id.to_string(),
    )
  }
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

impl InsertQuery for Meditation {
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO meditation (record_id, user_id, meditation_minutes, meditation_seconds, guild_id, occurred_at) VALUES ($1, $2, $3, $4, $5, $6)",
      self.id,
      self.user_id.to_string(),
      self.minutes,
      self.seconds,
      self.guild_id.to_string(),
      self.occurred_at,
    )
  }
}

impl UpdateQuery for Meditation {
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "UPDATE meditation SET meditation_minutes = $1, meditation_seconds = $2, occurred_at = $3 WHERE record_id = $4",
      self.minutes,
      self.seconds,
      self.occurred_at,
      self.id,
    )
  }
}

impl DeleteQuery for Meditation {
  fn delete_query<'a>(
    _guild_id: GuildId,
    meditation_id: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      "DELETE FROM meditation WHERE record_id = $1",
      meditation_id.into(),
    )
  }
}

impl FromRow<'_, PgRow> for Meditation {
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
      id: row.try_get("record_id").unwrap_or_default(),
      guild_id,
      user_id,
      minutes: row.try_get("meditation_minutes").unwrap_or_default(),
      seconds: row.try_get("meditation_seconds").unwrap_or_default(),
      occurred_at: row.try_get("occurred_at").unwrap_or_default(),
    })
  }
}
