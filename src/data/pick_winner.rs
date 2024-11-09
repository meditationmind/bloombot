use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;
use sqlx::{FromRow, Postgres};

/// Retrieves a monthly challenge winner candidate from the database.
/// Candidates include any user who logged time during the specified period.
/// Criteria further restricting the pool of candidates are defined in the
/// [`pick_winner`][pw] command.
///
/// [pw]: crate::commands::pick_winner
pub fn retrieve_candidate<'a, T: for<'r> FromRow<'r, PgRow>>(
  guild_id: GuildId,
  start_date: DateTime<Utc>,
  end_date: DateTime<Utc>,
) -> QueryAs<'a, Postgres, T, PgArguments> {
  // All entries between the start and end dates that are greater than 0 minutes.
  // We only want a user ID to show up once, so we group by user ID.
  sqlx::query_as(
    "SELECT user_id FROM meditation WHERE meditation_minutes > 0 AND occurred_at >= $1 AND occurred_at <= $2 AND guild_id = $3 GROUP BY user_id ORDER BY RANDOM()",
  )
  .bind(start_date)
  .bind(end_date)
  .bind(guild_id.to_string())
}

/// Calculates the sum of meditation minutes during the specified period
/// for a monthly challenge winner candidate.
pub fn candidate_sum<'a, T: for<'r> FromRow<'r, PgRow>>(
  guild_id: GuildId,
  user_id: UserId,
  start_date: DateTime<Utc>,
  end_date: DateTime<Utc>,
) -> QueryAs<'a, Postgres, T, PgArguments> {
  sqlx::query_as(
    "SELECT (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4",
  )
  .bind(user_id.to_string())
  .bind(guild_id.to_string())
  .bind(start_date)
  .bind(end_date)
}

/// Calculates the total count of meditation sessions during the specified period
/// for a monthly challenge winner candidate.
pub fn candidate_count<'a, T: for<'r> FromRow<'r, PgRow>>(
  guild_id: GuildId,
  user_id: UserId,
  start_date: DateTime<Utc>,
  end_date: DateTime<Utc>,
) -> QueryAs<'a, Postgres, T, PgArguments> {
  sqlx::query_as(
    "SELECT COUNT(record_id) AS count FROM meditation WHERE user_id = $1 AND guild_id = $2 AND occurred_at >= $3 AND occurred_at <= $4",
  )
  .bind(user_id.to_string())
  .bind(guild_id.to_string())
  .bind(start_date)
  .bind(end_date)
}
