use pgvector::Vector;
use poise::Modal;
use poise::serenity_prelude::GuildId;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::{Query, QueryAs};
use sqlx::{Error as SqlxError, FromRow, Postgres, Result as SqlxResult, Row};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::data::common;
use crate::handlers::database::{DeleteQuery, ExistsQuery, InsertQuery, UpdateQuery};

#[derive(Debug, Default)]
pub struct Term {
  guild_id: GuildId,
  pub name: String,
  pub meaning: String,
  pub usage: Option<String>,
  pub links: Option<Vec<String>>,
  pub category: Option<String>,
  pub aliases: Option<Vec<String>>,
  vector: Option<Vector>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Modal)]
#[name = "Add/Edit Term"]
pub struct TermModal {
  #[name = "The definition of the term"]
  #[placeholder = "The first paragraph should be a concise summary (used by /whatis)"]
  #[paragraph]
  #[max_length = 1000]
  pub meaning: String,
  #[name = "An example of the term in use"]
  pub usage: Option<String>,
  #[name = "The category of the term"]
  pub category: Option<String>,
  #[name = "Links to further reading, comma separated"]
  pub links: Option<String>,
  #[name = "Term aliases, comma separated"]
  pub aliases: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct VectorSearch {
  pub term_name: String,
  pub meaning: String,
  pub distance_score: Option<f64>,
}

impl Term {
  /// Creates a new [`Term`] with a specified [`GuildId`][gid], `name`,
  /// and [`TermModal`], from which it receives all remaining values.
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  pub fn from_modal(
    guild_id: impl Into<GuildId>,
    name: impl Into<String>,
    modal: TermModal,
    vector: Option<Vector>,
  ) -> Self {
    Self {
      guild_id: guild_id.into(),
      name: name.into(),
      meaning: modal.meaning,
      usage: modal.usage,
      links: modal
        .links
        .map(|links| links.split(',').map(|s| s.trim().to_string()).collect()),
      category: modal.category,
      aliases: modal
        .aliases
        .map(|aliases| aliases.split(',').map(|s| s.trim().to_string()).collect()),
      vector,
    }
  }

  /// Takes a list of [`Term`]s as [`Vec<Term>`][Term] and generates an alphabetically
  /// sorted list of term names and aliases only, returned as a [`Vec<String>`]. This is
  /// used to generate the list of terms for term name autocompletion in glossary commands.
  pub fn names_and_aliases(terms: Vec<Self>) -> Vec<String> {
    if terms.is_empty() || terms[0].name.is_empty() {
      return vec![String::new()];
    }
    let mut names = terms
      .iter()
      .map(|term| term.name.to_string())
      .rev()
      .collect::<Vec<String>>();
    let mut aliases = vec![];
    for term in terms {
      if let Some(term_aliases) = term.aliases {
        if !term_aliases.is_empty() {
          for alias in term_aliases {
            aliases.push(alias);
          }
        }
      }
    }
    names.append(&mut aliases);
    names.sort_by_key(|name| name.to_lowercase());
    names
  }

  /// Updates the vector embeddings for a [`Term`] in the database.
  pub fn update_embedding(
    guild_id: GuildId,
    term_name: impl Into<String>,
    vector: Option<&Vector>,
  ) -> Query<'_, Postgres, PgArguments> {
    sqlx::query(
      "UPDATE term SET embedding = $3 WHERE guild_id = $1 AND (LOWER(term_name) = LOWER($2))",
    )
    .bind(guild_id.to_string())
    .bind(term_name.into())
    .bind(vector)
  }

  /// Retrieves a [`Term`] from the database.
  pub fn retrieve<'a>(
    guild_id: GuildId,
    term_name: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT term_name, meaning, usage, links, category, aliases FROM term WHERE guild_id = $2 AND (LOWER(term_name) = LOWER($1)) OR (f_textarr2text(aliases) ~* ('(?:^|,)' || $1 || '(?:$|,)'))",
    )
    .bind(term_name.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves a [`Term`] definition from the database.
  pub fn retrieve_meaning<'a>(
    guild_id: GuildId,
    term_name: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT meaning FROM term WHERE guild_id = $2 AND (LOWER(term_name) = LOWER($1))",
    )
    .bind(term_name.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves a list of [`Term`]s and their aliases from the database.
  pub fn retrieve_list<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT term_name, aliases FROM term WHERE guild_id = $1 ORDER BY term_name DESC",
    )
    .bind(guild_id.to_string())
  }

  /// Retrieves up to five [`Term`]s from the database with names most similar to the specified
  /// `term_name`, with the similarity threshold set by `similarity`.
  pub fn retrieve_similar<'a>(
    guild_id: GuildId,
    term_name: &str,
    similarity: f32,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT term_name, meaning, usage, links, category, aliases, SET_LIMIT($2) FROM term WHERE guild_id = $3 AND (LOWER(term_name) % LOWER($1)) OR (f_textarr2text(aliases) ILIKE '%' || $1 || '%') ORDER BY SIMILARITY(LOWER(term_name), LOWER($1)) DESC LIMIT 5",
    )
    .bind(term_name.to_string())
    .bind(similarity)
    .bind(guild_id.to_string())
  }

  /// Calculates the total count of [`Term`]s in the database.
  pub fn count<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as("SELECT COUNT(record_id) AS count FROM term WHERE guild_id = $1")
      .bind(guild_id.to_string())
  }
}

impl InsertQuery for Term {
  /// Adds a [`Term`] to the database.
  fn insert_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query(
      "
        INSERT INTO term (record_id, term_name, meaning, usage, links, category, aliases, guild_id, embedding) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
      ",
    )
    .bind(Ulid::new().to_string())
    .bind(self.name.clone())
    .bind(self.meaning.clone())
    .bind(self.usage.clone())
    .bind(self.links.clone())
    .bind(self.category.clone())
    .bind(self.aliases.clone())
    .bind(self.guild_id.to_string())
    .bind(self.vector.clone())
  }
}

impl UpdateQuery for Term {
  /// Updates a [`Term`] in the database.
  fn update_query(&self) -> Query<Postgres, PgArguments> {
    sqlx::query(
      "UPDATE term SET meaning = $1, usage = $2, links = $3, category = $4, aliases = $5, embedding = COALESCE($6, embedding) WHERE LOWER(term_name) = LOWER($7)",
    )
    .bind(self.meaning.clone())
    .bind(self.usage.clone())
    .bind(self.links.clone())
    .bind(self.category.clone())
    .bind(self.aliases.clone())
    .bind(self.vector.clone())
    .bind(self.name.clone())
  }
}

impl DeleteQuery for Term {
  /// Removes a [`Term`] from the database.
  fn delete_query<'a>(
    guild_id: GuildId,
    term_name: impl Into<String>,
  ) -> Query<'a, Postgres, PgArguments> {
    query!(
      "DELETE FROM term WHERE (LOWER(term_name) = LOWER($1)) AND guild_id = $2",
      term_name.into(),
      guild_id.to_string(),
    )
  }
}

impl ExistsQuery for Term {
  type Item<'a> = &'a str;

  /// Checks to see if a [`Term`] exists in the database.
  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    term_name: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as(
      "SELECT EXISTS (SELECT 1 FROM term WHERE (LOWER(term_name) = LOWER($1)) AND guild_id = $2)",
    )
    .bind(term_name)
    .bind(guild_id.to_string())
  }
}

impl PageRow for Term {
  fn title(&self, _page_type: PageType) -> String {
    format!("__{}__", self.name.clone())
  }

  fn body(&self) -> String {
    self.meaning.clone()
  }
}

impl FromRow<'_, PgRow> for Term {
  fn from_row(row: &'_ PgRow) -> SqlxResult<Self, SqlxError> {
    let guild_id = GuildId::new(common::decode_id_row(row, "guild_id")?);

    Ok(Self {
      guild_id,
      name: row.try_get("term_name").unwrap_or_default(),
      meaning: row.try_get("meaning").unwrap_or_default(),
      usage: row.try_get("usage").unwrap_or_default(),
      links: row.try_get("links").unwrap_or_default(),
      category: row.try_get("category").unwrap_or_default(),
      aliases: row.try_get("aliases").unwrap_or_default(),
      vector: row.try_get("embedding").unwrap_or_default(),
    })
  }
}

impl From<Term> for TermModal {
  /// Converts a [`Term`] into a [`TermModal`]. Note that the [`GuildId`][gid]
  /// and `name` fields will be lost in the conversion. To convert back to a [`Term`],
  /// use the [`Term::from_modal()`] method with the original [`GuildId`][gid] and `name`.
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  fn from(term: Term) -> Self {
    Self {
      meaning: term.meaning,
      usage: term.usage,
      category: term.category,
      links: term.links.map(|links| links.join(", ")),
      aliases: term.aliases.map(|aliases| aliases.join(", ")),
    }
  }
}

impl VectorSearch {
  pub fn result(
    guild_id: GuildId,
    search_vector: &Vector,
    limit: i64,
  ) -> QueryAs<'_, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT term_name, meaning, embedding <=> $1 AS distance_score FROM term WHERE guild_id = $2 ORDER BY distance_score ASC LIMIT $3",
    )
    .bind(search_vector)
    .bind(guild_id.to_string())
    .bind(limit)
  }
}
