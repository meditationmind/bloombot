use poise::serenity_prelude as serenity;

use crate::handlers::database::UpdateQuery;

#[derive(sqlx::FromRow)]
pub struct Exists {
  pub exists: bool,
}

pub struct Migration {
  pub guild: serenity::GuildId,
  pub old_user: serenity::UserId,
  pub new_user: serenity::UserId,
  pub kind: MigrationType,
}

pub enum MigrationType {
  TrackingProfile,
  MeditationEntries,
}

impl Migration {
  pub fn new(
    guild_id: impl Into<serenity::GuildId>,
    old_user_id: impl Into<serenity::UserId>,
    new_user_id: impl Into<serenity::UserId>,
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
  fn update_query(&self) -> sqlx::query::Query<sqlx::Postgres, sqlx::postgres::PgArguments> {
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
