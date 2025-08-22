use poise::serenity_prelude::{GuildId, Mentionable, RoleId};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::data::common;
use crate::handlers::database::{DeleteQuery, ExistsQuery, InsertQuery, UpdateQuery};

pub struct Course {
  pub name: String,
  pub participant_role: RoleId,
  pub graduate_role: RoleId,
  pub guild_id: GuildId,
}

impl Course {
  pub fn new(
    name: impl Into<String>,
    participant_role: RoleId,
    graduate_role: RoleId,
    guild_id: GuildId,
  ) -> Self {
    Self {
      name: name.into(),
      participant_role,
      graduate_role,
      guild_id,
    }
  }

  /// Retrieves a [`Course`] from the database.
  pub fn retrieve<'a>(
    guild_id: GuildId,
    course_name: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT course_name, participant_role, graduate_role FROM course WHERE LOWER(course_name) = LOWER($1) AND guild_id = $2",
    )
    .bind(course_name.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves a [`Course`] from the database while in DMs, matching by course name only.
  pub fn retrieve_in_dm<'a>(course_name: &str) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT course_name, participant_role, graduate_role, guild_id FROM course WHERE LOWER(course_name) = LOWER($1)",
    )
    .bind(course_name.to_string())
  }

  /// Retrieves the [`Course`] from the database with the name most similar to the specified
  /// `course_name`, unless none meets the similarity threshold set by `similarity`.
  pub fn retrieve_similar<'a>(
    guild_id: GuildId,
    course_name: &str,
    similarity: f32,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT course_name, participant_role, graduate_role, SET_LIMIT($2) FROM course WHERE LOWER(course_name) % LOWER($1) AND guild_id = $3 ORDER BY SIMILARITY(LOWER(course_name), LOWER($1)) DESC LIMIT 1",
    )
    .bind(course_name.to_string())
    .bind(similarity)
    .bind(guild_id.to_string())
  }

  /// Retrieves all [`Course`]s from the database.
  pub fn retrieve_all<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT course_name, participant_role, graduate_role FROM course WHERE guild_id = $1 ORDER BY course_name ASC",
    )
    .bind(guild_id.to_string())
  }
}

impl InsertQuery for Course {
  /// Adds a [`Course`] to the database.
  fn insert_query(&'_ self) -> Query<'_, Postgres, PgArguments> {
    query!(
      "INSERT INTO course (record_id, course_name, participant_role, graduate_role, guild_id) VALUES ($1, $2, $3, $4, $5)",
      Ulid::new().to_string(),
      self.name,
      self.participant_role.to_string(),
      self.graduate_role.to_string(),
      self.guild_id.to_string(),
    )
  }
}

impl UpdateQuery for Course {
  /// Updates a [`Course`] in the database.
  fn update_query(&'_ self) -> Query<'_, Postgres, PgArguments> {
    query!(
      "UPDATE course SET participant_role = $1, graduate_role = $2 WHERE LOWER(course_name) = LOWER($3)",
      self.participant_role.to_string(),
      self.graduate_role.to_string(),
      self.name,
    )
  }
}

impl DeleteQuery for Course {
  /// Removes a [`Course`] from the database.
  fn delete_query<'a>(
    guild_id: GuildId,
    course_name: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    query!(
      "DELETE FROM course WHERE course_name = $1 AND guild_id = $2",
      course_name.into(),
      guild_id.to_string(),
    )
  }
}

impl ExistsQuery for Course {
  type Item<'a> = &'a str;

  /// Checks to see if a [`Course`] exists in the database.
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    course_name: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as("SELECT EXISTS(SELECT 1 FROM course WHERE course_name = $1 AND guild_id = $2)")
      .bind(course_name)
      .bind(guild_id.to_string())
  }
}

impl PageRow for Course {
  fn title(&self, _page_type: PageType) -> String {
    format!("`{}`", self.name.clone())
  }

  fn body(&self) -> String {
    format!(
      "Participants: {}\nGraduates: {}",
      self.participant_role.mention(),
      self.graduate_role.mention()
    )
  }
}

impl FromRow<'_, PgRow> for Course {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);
    let participant_role = RoleId::new(common::decode_id_row(row, "participant_role")?);
    let graduate_role = RoleId::new(common::decode_id_row(row, "graduate_role")?);

    Ok(Self {
      name: row.try_get("course_name").unwrap_or_default(),
      participant_role,
      graduate_role,
      guild_id,
    })
  }
}
