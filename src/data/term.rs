use poise::serenity_prelude::GuildId;
use poise::Modal;
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::query::QueryAs;
use sqlx::{FromRow, Postgres};

use crate::commands::helpers::pagination::{PageRow, PageType};
use crate::handlers::database::ExistsQuery;

#[derive(Debug)]
pub struct Term {
  pub guild_id: GuildId,
  pub name: String,
  pub meaning: String,
  pub usage: Option<String>,
  pub links: Option<Vec<String>>,
  pub category: Option<String>,
  pub aliases: Option<Vec<String>>,
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
pub struct SearchResult {
  pub term_name: String,
  pub meaning: String,
  pub distance_score: Option<f64>,
}

#[derive(Debug)]
pub struct Names {
  pub term_name: String,
  pub aliases: Option<Vec<String>>,
}

impl Term {
  /// Creates a new [`Term`] with a specified [`GuildId`][gid], `name`,
  /// and `meaning`. All other values are set to `None`.
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  pub fn new(
    guild_id: impl Into<GuildId>,
    name: impl Into<String>,
    meaning: impl Into<String>,
  ) -> Self {
    Self {
      guild_id: guild_id.into(),
      name: name.into(),
      meaning: meaning.into(),
      usage: None,
      links: None,
      category: None,
      aliases: None,
    }
  }

  /// Assigns a [`GuildId`][gid] to a [`Term`].
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  pub fn guild_id(mut self, guild_id: impl Into<GuildId>) -> Self {
    self.guild_id = guild_id.into();
    self
  }

  /// Assigns a `name` to a [`Term`].
  pub fn name(mut self, name: impl Into<String>) -> Self {
    self.name = name.into();
    self
  }

  /// Assigns a `meaning` to a [`Term`].
  pub fn meaning(mut self, meaning: impl Into<String>) -> Self {
    self.meaning = meaning.into();
    self
  }

  /// Assigns a `usage` to a [`Term`].
  pub fn usage(mut self, usage: Option<String>) -> Self {
    self.usage = usage;
    self
  }

  /// Takes an [`Option<String>`], with [`String`] being one or more hyperlinks
  /// separated by commas, splits the hyperlinks into a [`Vec<String>`] and assigns
  /// them to a [`Term`].
  pub fn links(mut self, links: Option<String>) -> Self {
    if let Some(links) = links {
      self.links = Some(links.split(',').map(|s| s.trim().to_string()).collect());
      self
    } else {
      self.links = None;
      self
    }
  }

  /// Assigns a `category` to a [`Term`].
  pub fn category(mut self, category: Option<String>) -> Self {
    self.category = category;
    self
  }

  /// Takes an [`Option<String>`], with [`String`] being one or more aliases
  /// separated by commas, splits the aliases into a [`Vec<String>`] and assigns
  /// them to a [`Term`].
  pub fn aliases(mut self, aliases: Option<String>) -> Self {
    if let Some(aliases) = aliases {
      self.aliases = Some(aliases.split(',').map(|s| s.trim().to_string()).collect());
      self
    } else {
      self.aliases = None;
      self
    }
  }

  /// Creates a new [`Term`] with a specified [`GuildId`][gid], `name`,
  /// and [`TermModal`], from which it receives all remaining values.
  ///
  /// [gid]: poise::serenity_prelude::model::id::GuildId
  pub fn from_modal(
    guild_id: impl Into<GuildId>,
    name: impl Into<String>,
    modal: TermModal,
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
    }
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

impl ExistsQuery for Term {
  type Item<'a> = &'a str;

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
