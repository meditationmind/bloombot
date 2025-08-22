use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::Query;
use sqlx::{Error as SqlxError, Postgres, Row};

use crate::commands::helpers::time::Timeframe;
use crate::handlers::database::UpdateQuery;

#[derive(Default, sqlx::FromRow)]
#[sqlx(default)]
pub struct Exists {
  pub exists: bool,
}

#[derive(Default, sqlx::FromRow)]
#[sqlx(default)]
pub struct Aggregate {
  #[sqlx(try_from = "i64")]
  pub count: u64,
  pub sum: i64,
}

pub struct Migration {
  pub guild: GuildId,
  pub old_user: UserId,
  pub new_user: UserId,
  pub kind: MigrationType,
}

pub enum MigrationType {
  TrackingProfile,
  MeditationEntries,
}

pub struct MaterializedView {}

pub enum ViewType {
  Leaderboard,
  ChartStats,
}

impl Migration {
  pub fn new(
    guild_id: impl Into<GuildId>,
    old_user_id: impl Into<UserId>,
    new_user_id: impl Into<UserId>,
    kind: MigrationType,
  ) -> Self {
    Self {
      guild: guild_id.into(),
      old_user: old_user_id.into(),
      new_user: new_user_id.into(),
      kind,
    }
  }
}

impl UpdateQuery for Migration {
  fn update_query(&'_ self) -> Query<'_, Postgres, PgArguments> {
    match self.kind {
      MigrationType::TrackingProfile => {
        query!(
          "UPDATE tracking_profile SET user_id = $3 WHERE user_id = $1 AND guild_id = $2",
          self.old_user.to_string(),
          self.guild.to_string(),
          self.new_user.to_string(),
        )
      }
      MigrationType::MeditationEntries => {
        query!(
          "UPDATE meditation SET user_id = $3 WHERE user_id = $1 AND guild_id = $2",
          self.old_user.to_string(),
          self.guild.to_string(),
          self.new_user.to_string(),
        )
      }
    }
  }
}

impl MaterializedView {
  pub fn refresh<'a>(
    view_type: &ViewType,
    timeframe: &Timeframe,
  ) -> Query<'a, Postgres, PgArguments> {
    let query = match view_type {
      ViewType::Leaderboard => match timeframe {
        Timeframe::Yearly => "REFRESH MATERIALIZED VIEW CONCURRENTLY yearly_leaderboard",
        Timeframe::Monthly => "REFRESH MATERIALIZED VIEW CONCURRENTLY monthly_leaderboard",
        Timeframe::Weekly => "REFRESH MATERIALIZED VIEW CONCURRENTLY weekly_leaderboard",
        Timeframe::Daily => "REFRESH MATERIALIZED VIEW CONCURRENTLY daily_leaderboard",
      },
      ViewType::ChartStats => match timeframe {
        Timeframe::Yearly => "REFRESH MATERIALIZED VIEW yearly_data",
        Timeframe::Monthly => "REFRESH MATERIALIZED VIEW monthly_data",
        Timeframe::Weekly => "REFRESH MATERIALIZED VIEW weekly_data",
        Timeframe::Daily => unreachable!("No daily_data materialized view"),
      },
    };
    sqlx::query(query)
  }
}

pub fn decode_id_row(row: &'_ PgRow, index: &str) -> Result<u64, SqlxError> {
  let string: String = row.try_get(index).unwrap_or("1".to_string());
  match string.parse::<u64>() {
    Ok(id) => Ok(id),
    Err(e) => Err(SqlxError::ColumnDecode {
      index: index.to_string(),
      source: Box::new(e),
    }),
  }
}

pub fn decode_option_id_row(row: &'_ PgRow, index: &str) -> Result<Option<u64>, SqlxError> {
  match row.try_get::<String, &str>(index) {
    Ok(string_id) => match string_id.parse::<u64>() {
      Ok(id) => Ok(Some(id)),
      Err(e) => Err(SqlxError::ColumnDecode {
        index: index.to_string(),
        source: Box::new(e),
      }),
    },
    Err(_) => Ok(None),
  }
}
