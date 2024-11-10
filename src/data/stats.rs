use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::data::common;
use crate::handlers::database::UpdateQuery;

#[derive(Default)]
pub struct Streak {
  guild_id: GuildId,
  user_id: UserId,
  pub current: i32,
  pub longest: i32,
}

#[derive(Debug, FromRow)]
pub struct MeditationCountByDay {
  pub days_ago: i32,
}

#[derive(Debug)]
pub struct Timeframe {
  pub sum: Option<i64>,
  pub count: Option<i64>,
}

pub struct User {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: Timeframe,
  pub streak: Streak,
}

pub struct Guild {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: Timeframe,
}

#[derive(Debug)]
pub struct LeaderboardUser {
  pub name: Option<String>,
  pub minutes: Option<i64>,
  pub sessions: Option<i64>,
  pub streak: Option<i32>,
  pub anonymous_tracking: Option<bool>,
  pub streaks_active: Option<bool>,
  pub streaks_private: Option<bool>,
}

impl Streak {
  pub fn new(guild_id: GuildId, user_id: UserId, current: i32, longest: i32) -> Self {
    Self {
      guild_id,
      user_id,
      current,
      longest,
    }
  }

  pub fn calculate<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT current_streak, longest_streak FROM streak WHERE guild_id = $1 AND user_id = $2",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
  }
}

impl UpdateQuery for Streak {
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO streak (record_id, user_id, guild_id, current_streak, longest_streak) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (user_id) DO UPDATE SET current_streak = $4, longest_streak = $5",
      Ulid::new().to_string(),
      self.user_id.to_string(),
      self.guild_id.to_string(),
      self.current,
      self.longest,
    )
  }
}

impl FromRow<'_, PgRow> for Streak {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);
    let user_id = UserId::new(common::decode_id_row(row, "user_id")?);

    Ok(Self {
      guild_id,
      user_id,
      current: row.try_get("current_streak").unwrap_or_default(),
      longest: row.try_get("longest_streak").unwrap_or_default(),
    })
  }
}

impl MeditationCountByDay {
  pub fn calculate<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "WITH cte AS (SELECT date_part('day', NOW() - DATE_TRUNC('day', occurred_at))::int AS days_ago FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at::date <= NOW()::date) SELECT days_ago FROM cte GROUP BY days_ago ORDER BY days_ago ASC",
    )
    .bind(user_id.to_string())
    .bind(guild_id.to_string())
  }
}
