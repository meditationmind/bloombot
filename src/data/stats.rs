use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::time::Timeframe as StatsTimeframe;
use crate::commands::stats::{LeaderboardType, SortBy};
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

#[derive(Debug, FromRow)]
pub struct Timeframe {
  pub sum: Option<i64>,
  pub count: Option<i64>,
}

#[derive(Debug, Default, FromRow)]
#[sqlx(default)]
pub struct LeaderboardUser {
  pub name: Option<String>,
  pub minutes: Option<i64>,
  pub sessions: Option<i64>,
  pub streak: Option<i32>,
  pub anonymous_tracking: Option<bool>,
  pub streaks_active: Option<bool>,
  pub streaks_private: Option<bool>,
}

#[derive(Debug, Default, FromRow)]
#[sqlx(default)]
pub struct ByInterval {
  pub times_ago: Option<f64>,
  pub meditation_minutes: Option<i64>,
  pub meditation_count: Option<i64>,
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
    query!(
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

impl Timeframe {
  pub fn new(sum: Option<i64>, count: Option<i64>) -> Self {
    Self { sum, count }
  }

  pub fn user_sum_and_count<'a>(
    guild_id: GuildId,
    user_id: UserId,
    start_time: &'a DateTime<Utc>,
    end_time: &'a DateTime<Utc>,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum FROM meditation WHERE guild_id = $1 AND user_id = $2 AND occurred_at >= $3 AND occurred_at <= $4",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
    .bind(start_time)
    .bind(end_time)
  }

  pub fn user_total_sum_and_count<'a>(
    guild_id: GuildId,
    user_id: UserId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum FROM meditation WHERE guild_id = $1 AND user_id = $2",
    )
    .bind(guild_id.to_string())
    .bind(user_id.to_string())
  }

  pub fn guild_sum_and_count<'a>(
    guild_id: GuildId,
    start_time: &'a DateTime<Utc>,
    end_time: &'a DateTime<Utc>,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum FROM meditation WHERE guild_id = $1 AND occurred_at >= $2 AND occurred_at <= $3",
    )
    .bind(guild_id.to_string())
    .bind(start_time)
    .bind(end_time)
  }

  pub fn guild_total_sum_and_count<'a>(
    guild_id: GuildId,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT COUNT(record_id) AS count, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS sum FROM meditation WHERE guild_id = $1",
    )
    .bind(guild_id.to_string())
  }
}

impl LeaderboardUser {
  pub fn stats<'a>(
    guild_id: GuildId,
    timeframe: &StatsTimeframe,
    sort_by: &SortBy,
    leaderboard_type: &LeaderboardType,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    let limit = match leaderboard_type {
      LeaderboardType::Top5 => 5,
      LeaderboardType::Top10 => 10,
    };
    let query = match timeframe {
      StatsTimeframe::Daily => match sort_by {
        SortBy::Minutes => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM daily_leaderboard WHERE guild = $1 ORDER BY minutes DESC LIMIT $2"
        }
        SortBy::Sessions => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM daily_leaderboard WHERE guild = $1 ORDER BY sessions DESC LIMIT $2"
        }
        SortBy::Streak => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM daily_leaderboard WHERE guild = $1 ORDER BY streak DESC LIMIT $2"
        }
      },
      StatsTimeframe::Weekly => match sort_by {
        SortBy::Minutes => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM weekly_leaderboard WHERE guild = $1 ORDER BY minutes DESC LIMIT $2"
        }
        SortBy::Sessions => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM weekly_leaderboard WHERE guild = $1 ORDER BY sessions DESC LIMIT $2"
        }
        SortBy::Streak => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM weekly_leaderboard WHERE guild = $1 ORDER BY streak DESC LIMIT $2"
        }
      },
      StatsTimeframe::Monthly => match sort_by {
        SortBy::Minutes => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM monthly_leaderboard WHERE guild = $1 ORDER BY minutes DESC LIMIT $2"
        }
        SortBy::Sessions => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM monthly_leaderboard WHERE guild = $1 ORDER BY sessions DESC LIMIT $2"
        }
        SortBy::Streak => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM monthly_leaderboard WHERE guild = $1 ORDER BY streak DESC LIMIT $2"
        }
      },
      StatsTimeframe::Yearly => match sort_by {
        SortBy::Minutes => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM yearly_leaderboard WHERE guild = $1 ORDER BY minutes DESC LIMIT $2"
        }
        SortBy::Sessions => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM yearly_leaderboard WHERE guild = $1 ORDER BY sessions DESC LIMIT $2"
        }
        SortBy::Streak => {
          "SELECT name, minutes, sessions, streak, anonymous_tracking, streaks_active, streaks_private FROM yearly_leaderboard WHERE guild = $1 ORDER BY streak DESC LIMIT $2"
        }
      },
    };

    sqlx::query_as(query).bind(guild_id.to_string()).bind(limit)
  }
}

impl ByInterval {
  pub fn user_fresh<'a>(
    guild_id: GuildId,
    user_id: UserId,
    timeframe: &StatsTimeframe,
    now_offset: &'a DateTime<Utc>,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    let query = match timeframe {
      StatsTimeframe::Yearly => {
        "WITH current_year_data AS (SELECT floor(extract(epoch from ((date_trunc('year', now()) + interval '1 year') - interval '1 second') - occurred_at) / extract(epoch from (((date_trunc('year', occurred_at) + interval '1 year') - interval '1 second') - (date_trunc('year', occurred_at)))))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1 AND user_id = $2) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_year_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Monthly => {
        "WITH current_month_data AS (SELECT floor(extract(epoch from ((date_trunc('month', now()) + interval '1 month') - interval '1 second') - occurred_at) / extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at)))))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1 AND user_id = $2) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_month_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Weekly => {
        "WITH current_week_data AS (SELECT floor(extract(epoch from ((date_trunc('week', now()) + interval '1 week') - interval '1 second') - occurred_at) / (60*60*24*7))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1 AND user_id = $2) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_week_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Daily => {
        "WITH daily_data AS (SELECT date_part('day', $1 - DATE_TRUNC('day', occurred_at)) AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $2 AND user_id = $3 AND occurred_at <= $1) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM daily_data WHERE times_ago <= 12 GROUP BY times_ago"
      }
    };

    match timeframe {
      StatsTimeframe::Daily => sqlx::query_as(query)
        .bind(now_offset)
        .bind(guild_id.to_string())
        .bind(user_id.to_string()),
      _ => sqlx::query_as(query)
        .bind(guild_id.to_string())
        .bind(user_id.to_string()),
    }
  }

  pub fn user_from_view<'a>(
    guild_id: GuildId,
    user_id: UserId,
    timeframe: &StatsTimeframe,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    let query = match timeframe {
      StatsTimeframe::Yearly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM yearly_data WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Monthly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM monthly_data WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Weekly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM weekly_data WHERE guild_id = $1 AND user_id = $2 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Daily => unreachable!("No daily_data materialized view"),
    };

    sqlx::query_as(query)
      .bind(guild_id.to_string())
      .bind(user_id.to_string())
  }

  pub fn guild_fresh<'a>(
    guild_id: GuildId,
    timeframe: &StatsTimeframe,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    let query = match timeframe {
      StatsTimeframe::Yearly => {
        "WITH current_year_data AS (SELECT floor(extract(epoch from ((date_trunc('year', now()) + interval '1 year') - interval '1 second') - occurred_at) / extract(epoch from (((date_trunc('year', occurred_at) + interval '1 year') - interval '1 second') - (date_trunc('year', occurred_at)))))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_year_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Monthly => {
        "WITH current_month_data AS (SELECT floor(extract(epoch from ((date_trunc('month', now()) + interval '1 month') - interval '1 second') - occurred_at) / extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at)))))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_month_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Weekly => {
        "WITH current_week_data AS (SELECT floor(extract(epoch from ((date_trunc('week', now()) + interval '1 week') - interval '1 second') - occurred_at) / (60*60*24*7))::float AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM current_week_data WHERE times_ago = 0 GROUP BY times_ago"
      }
      StatsTimeframe::Daily => {
        "WITH daily_data AS (SELECT date_part('day', NOW() - DATE_TRUNC('day', occurred_at)) AS times_ago, meditation_minutes, meditation_seconds FROM meditation WHERE guild_id = $1) SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM daily_data WHERE times_ago <= 12 GROUP BY times_ago"
      }
    };

    sqlx::query_as(query).bind(guild_id.to_string())
  }

  pub fn guild_from_view<'a>(
    guild_id: GuildId,
    timeframe: &StatsTimeframe,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    let query = match timeframe {
      StatsTimeframe::Yearly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM yearly_data WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Monthly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM monthly_data WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Weekly => {
        "SELECT times_ago, (SUM(meditation_minutes) + (SUM(meditation_seconds) / 60)) AS meditation_minutes, COUNT(*) AS meditation_count FROM weekly_data WHERE guild_id = $1 AND times_ago > 0 AND times_ago <= 12 GROUP BY times_ago"
      }
      StatsTimeframe::Daily => unreachable!("No daily_data materialized view"),
    };

    sqlx::query_as(query).bind(guild_id.to_string())
  }
}

impl User {
  pub fn new(
    total_minutes: i64,
    total_sessions: u64,
    timeframe_stats: Timeframe,
    streak: Streak,
  ) -> Self {
    Self {
      all_minutes: total_minutes,
      all_count: total_sessions,
      timeframe_stats,
      streak,
    }
  }

  pub fn challenge_stats(sum_and_count: Timeframe, streak: Streak) -> Self {
    Self {
      all_minutes: 0,
      all_count: 0,
      timeframe_stats: sum_and_count,
      streak,
    }
  }
}

impl Guild {
  pub fn new(total_minutes: i64, total_sessions: u64, timeframe_stats: Timeframe) -> Self {
    Self {
      all_minutes: total_minutes,
      all_count: total_sessions,
      timeframe_stats,
    }
  }
}
