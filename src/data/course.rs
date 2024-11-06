use poise::serenity_prelude::{GuildId, Mentionable, RoleId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;
use sqlx::{FromRow, Postgres};

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::ExistsQuery;

pub struct Course {
  pub name: String,
  pub participant_role: RoleId,
  pub graduate_role: RoleId,
}

#[allow(clippy::module_name_repetitions)]
pub struct ExtendedCourse {
  pub name: String,
  pub participant_role: RoleId,
  pub graduate_role: RoleId,
  pub guild_id: GuildId,
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
  type Item<'a> = &'a str;

  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    course_name: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as("SELECT EXISTS (SELECT 1 FROM course WHERE course_name = $1 AND guild_id = $2)")
      .bind(course_name)
      .bind(guild_id.to_string())
  }
}
