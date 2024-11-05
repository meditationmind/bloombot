use poise::serenity_prelude::{GuildId, UserId};
use sqlx::postgres::PgArguments;
use sqlx::query::Query;
use sqlx::Postgres;

use crate::handlers::database::UpdateQuery;

#[derive(sqlx::FromRow)]
pub struct Exists {
  pub exists: bool,
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
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    match self.kind {
      MigrationType::TrackingProfile => {
        sqlx::query!(
          "UPDATE tracking_profile SET user_id = $3 WHERE user_id = $1 AND guild_id = $2",
          self.old_user.to_string(),
          self.guild.to_string(),
          self.new_user.to_string(),
        )
      }
      MigrationType::MeditationEntries => {
        sqlx::query!(
          "UPDATE meditation SET user_id = $3 WHERE user_id = $1 AND guild_id = $2",
          self.old_user.to_string(),
          self.guild.to_string(),
          self.new_user.to_string(),
        )
      }
    }
  }
}
