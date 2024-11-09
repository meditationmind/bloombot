use anyhow::{anyhow, Result};
use poise::serenity_prelude::GuildId;
use poise::Modal;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;
use sqlx::{FromRow, Postgres};
use ulid::Ulid;

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::{DeleteQuery, ExistsQuery, InsertQuery, UpdateQuery};

#[allow(clippy::struct_field_names)]
#[derive(Default, FromRow)]
#[sqlx(default)]
pub struct Quote {
  #[sqlx(rename = "record_id")]
  id: String,
  pub quote: String,
  pub author: Option<String>,
  #[sqlx(skip)]
  guild_id: GuildId,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Modal)]
#[name = "Add/Edit Quote"]
pub struct QuoteModal {
  #[name = "Quote text"]
  #[placeholder = "Input quote text here"]
  #[paragraph]
  #[max_length = 300]
  quote: String,
  #[name = "Author's name"]
  #[placeholder = "Defaults to \"Anonymous\""]
  author: Option<String>,
}

impl Quote {
  /// Creates a new [`Quote`] with a specified `guild_id` and [`QuoteModal`],
  /// from which it receives the quote data. A new [ULID][ulid] is automatically
  /// generated and assigned.
  ///
  /// [ulid]: https://github.com/ulid/spec
  pub fn new_from_modal(guild_id: GuildId, modal: QuoteModal) -> Self {
    Self {
      id: Ulid::new().to_string(),
      quote: modal.quote,
      author: modal.author,
      guild_id,
    }
  }

  /// Retrieves a specific [`Quote`] from the database.
  pub fn retrieve<'a>(
    guild_id: GuildId,
    quote_id: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, quote, author FROM quote WHERE record_id = $1 AND guild_id = $2",
    )
    .bind(quote_id.to_string())
    .bind(guild_id.to_string())
  }

  /// Retrieves a random [`Quote`] from the database.
  pub fn retrieve_random<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, quote, author FROM quote WHERE guild_id = $1 ORDER BY RANDOM() LIMIT 1",
    )
    .bind(guild_id.to_string())
  }

  /// Retrieves a random [`Quote`] from the database, with the quote pool refined by
  /// a [PostgreSQL websearch query][ws] defined in `keyword`.
  ///
  /// [ws]: https://www.postgresql.org/docs/17/textsearch-controls.html#TEXTSEARCH-PARSING-QUERIES
  pub fn retrieve_random_with_keyword<'a>(
    guild_id: GuildId,
    keyword: &str,
  ) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, quote, author FROM quote WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2)) ORDER BY RANDOM() LIMIT 1",
    )
    .bind(guild_id.to_string())
    .bind(keyword.to_string())
  }

  /// Retrieves all [`Quote`]s from the database.
  pub fn retrieve_all<'a>(guild_id: GuildId) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as("SELECT record_id, quote, author FROM quote WHERE guild_id = $1")
      .bind(guild_id.to_string())
  }

  /// Searches available [`Quote`]s using a [PostgreSQL websearch query][ws] defined in `keyword`.
  ///
  /// [ws]: https://www.postgresql.org/docs/17/textsearch-controls.html#TEXTSEARCH-PARSING-QUERIES
  pub fn search<'a>(guild_id: GuildId, keyword: &str) -> QueryAs<'a, Postgres, Self, PgArguments> {
    sqlx::query_as(
      "SELECT record_id, quote, author FROM quote WHERE guild_id = $1 AND (quote_tsv @@ websearch_to_tsquery('english', $2))",
    )
    .bind(guild_id.to_string())
    .bind(keyword.to_string())
  }
}

impl InsertQuery for Quote {
  /// Adds a new [`Quote`] to the database.
  fn insert_query(&self) -> sqlx::query::Query<Postgres, PgArguments> {
    sqlx::query!(
      "INSERT INTO quote (record_id, quote, author, guild_id) VALUES ($1, $2, $3, $4)",
      self.id,
      self.quote,
      self.author,
      self.guild_id.to_string(),
    )
  }
}

impl UpdateQuery for Quote {
  /// Updates a [`Quote`] in the database.
  fn update_query(&self) -> sqlx::query::Query<Postgres, PgArguments> {
    sqlx::query!(
      "UPDATE quote SET quote = $1, author = $2 WHERE record_id = $3",
      self.quote,
      self.author,
      self.id,
    )
  }
}

impl DeleteQuery for Quote {
  /// Removes a [`Quote`] from the database.
  fn delete_query<'a>(
    guild_id: GuildId,
    quote_id: impl Into<String>,
  ) -> sqlx::query::Query<'a, Postgres, PgArguments> {
    sqlx::query!(
      "DELETE FROM quote WHERE record_id = $1 AND guild_id = $2",
      quote_id.into(),
      guild_id.to_string(),
    )
  }
}

impl ExistsQuery for Quote {
  type Item<'a> = &'a str;

  fn exists_query<'a, T: for<'r> FromRow<'r, PgRow>>(
    guild_id: GuildId,
    quote_id: Self::Item<'a>,
  ) -> QueryAs<'a, Postgres, T, PgArguments> {
    sqlx::query_as("SELECT EXISTS(SELECT 1 FROM quote WHERE record_id = $1 AND guild_id = $2)")
      .bind(quote_id)
      .bind(guild_id.to_string())
  }
}

impl PageRow for Quote {
  fn title(&self, _page_type: PageType) -> String {
    format!("`ID: {}`", self.id)
  }

  fn body(&self) -> String {
    format!(
      "{}\n― {}",
      self.quote.clone(),
      self.author.clone().unwrap_or("Anonymous".to_owned())
    )
  }
}

impl QuoteModal {
  /// Converts a [`QuoteModal`] into a [`Quote`] with the provided `guild_id` and `quote_id`.
  ///
  /// # Errors
  /// Produces a [`DecodeError`][de] if the `quote_id` supplied is not a valid [ULID][ulid].
  ///
  /// [de]: ulid::base32::DecodeError
  /// [ulid]: https://github.com/ulid/spec
  pub fn into_quote(self, guild_id: GuildId, quote_id: String) -> Result<Quote> {
    // We already check for ID validity in the `/quotes edit` command, but we'll
    // also make sure the ID is a valid ULID here, just in case this function ever
    // gets used in another place.
    match Ulid::from_string(&quote_id) {
      Ok(_) => Ok(Quote {
        id: quote_id,
        quote: self.quote,
        author: self.author,
        guild_id,
      }),
      Err(e) => Err(anyhow!(
        "Attempt to convert QuoteModal with invalid ID: {e}"
      )),
    }
  }
}

impl From<Quote> for QuoteModal {
  /// Converts a [`Quote`] into a [`QuoteModal`]. Note that the `id` and `guild_id`
  /// fields will be lost in the conversion. To convert back to a [`Quote`], use the
  /// [`QuoteModal::into_quote()`] method with the original `id` and `guild_id`.
  fn from(quote: Quote) -> Self {
    Self {
      quote: quote.quote,
      author: quote.author,
    }
  }
}
