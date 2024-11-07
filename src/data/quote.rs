use poise::serenity_prelude::GuildId;
use poise::Modal;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;
use sqlx::{FromRow, Postgres};

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::ExistsQuery;

#[allow(clippy::struct_field_names)]
pub struct Quote {
  pub id: String,
  pub quote: String,
  pub author: Option<String>,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Modal)]
#[name = "Add/Edit Quote"]
pub struct QuoteModal {
  #[name = "Quote text"]
  #[placeholder = "Input quote text here"]
  #[paragraph]
  #[max_length = 300]
  pub quote: String,
  #[name = "Author's name"]
  #[placeholder = "Defaults to \"Anonymous\""]
  pub author: Option<String>,
}

impl Quote {
  /// Creates a new [`Quote`] with a specified `id` and [`QuoteModal`],
  /// from which it receives all remaining values.
  pub fn from_modal(id: String, modal: QuoteModal) -> Self {
    Self {
      id,
      quote: modal.quote,
      author: modal.author,
    }
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

impl QuoteModal {
  /// Converts a [`QuoteModal`] into a [`Quote`] with the provided `id`.
  pub fn into_quote(self, id: String) -> Quote {
    Quote {
      id,
      quote: self.quote,
      author: self.author,
    }
  }
}

impl From<Quote> for QuoteModal {
  /// Converts a [`Quote`] into a [`QuoteModal`]. Note that the `id` field will be lost
  /// in the conversion. To convert back to a [`Quote`], use the [`QuoteModal::into_quote()`]
  /// method with the original `id`.
  fn from(quote: Quote) -> Self {
    Self {
      quote: quote.quote,
      author: quote.author,
    }
  }
}
