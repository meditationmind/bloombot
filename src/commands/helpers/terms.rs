#![allow(clippy::unused_async)]

use std::sync::{Arc, RwLock};

use tracing::info;

use crate::Context;
use crate::data::term::Term;

/// An autocomplete callback function used for commands that require selection of a term
/// from the glossary.
pub async fn autocomplete<'a>(
  ctx: Context<'a>,
  partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
  let term_names = match ctx.data().term_names.read() {
    Ok(term_names) => term_names.clone().into_iter(),
    Err(e) => {
      info!("Failed to acquire read lock for term names: {e}");
      vec![String::new()].into_iter()
    }
  };

  term_names.filter(move |term| {
    term
      .to_ascii_lowercase()
      .starts_with(&partial.to_ascii_lowercase())
  })
}

/// Updates the list of term names stored in [`Data`][data], which is used for
/// term selection [`autocomplete`].
///
/// [data]: crate::Data
pub async fn update_names(term_list: Vec<Term>, term_names: Arc<RwLock<Vec<String>>>) {
  let mut term_names = match term_names.write() {
    Ok(term_names) => term_names,
    Err(e) => {
      info!("Failed to acquire write lock for term names: {e}");
      return;
    }
  };

  let mut names = term_list
    .iter()
    .map(|term| term.name.to_string())
    .rev()
    .collect::<Vec<String>>();
  let mut aliases = vec![];
  for term in term_list {
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
  *term_names = names;
}
