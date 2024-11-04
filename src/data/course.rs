use crate::{
  commands::helpers::pagination::{PageRow, PageType},
  handlers::database::ExistsQuery,
};
use poise::serenity_prelude::{self as serenity, Mentionable};
use sqlx::{postgres::PgArguments, query::QueryAs, Postgres};

pub struct Course {
  pub name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
}

impl PageRow for Course {
  fn title(&self, _page_type: PageType) -> String {
    self.name.clone()
  }

  fn body(&self) -> String {
    format!(
      "Participants: {}\nGraduates: {}",
      self.participant_role.mention(),
      self.graduate_role.mention()
    )
  }
}

impl ExistsQuery for Course {
  fn exists_query<'a, T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>>(
    guild_id: serenity::GuildId,
    course_name: impl Into<String>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as("SELECT EXISTS (SELECT 1 FROM course WHERE course_name = $1 AND guild_id = $2)")
      .bind(course_name.into())
      .bind(guild_id.to_string())
  }
}

#[allow(clippy::module_name_repetitions)]
pub struct ExtendedCourse {
  pub name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
  pub guild_id: serenity::GuildId,
}
