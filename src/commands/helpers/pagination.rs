use std::fmt::Display;
use std::time::Duration;

use anyhow::Result;
use poise::serenity_prelude::ComponentInteractionCollector;
use poise::serenity_prelude::{CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter};
use poise::serenity_prelude::{CreateInteractionResponse, CreateInteractionResponseMessage};
use poise::CreateReply;

use crate::commands::helpers::common::Visibility;
use crate::config::BloomBotEmbed;
use crate::Context;

#[derive(Debug, Copy, Clone)]
pub enum PageType {
  Standard,
  Alternate,
}

pub trait PageRow {
  fn title(&self, page_type: PageType) -> String;
  fn body(&self) -> String;
}

pub type PageRowRef<'a> = &'a (dyn PageRow + Send + Sync);

pub struct Paginator<'a> {
  page_data: Vec<PaginationPage<'a>>,
  page_count: usize,
  title: String,
}

impl<'a> Paginator<'a> {
  /// Creates and initializes a new [`Paginator`] with a title, entry data as a [`PageRowRef`]
  /// vector slice, and the number of entries per page.
  ///
  /// The maximum entries per page is 25, a restriction imposed by the field limit for
  /// a single Discord embed object. Note that the combined sum of characters for an embed
  /// cannot exceed 6000, meaning that a sensible number of entries per page will generally
  /// fall between 5 and 10. See [Discord Embed Limits] for more info.
  ///
  /// [Discord Embed Limits]: https://discord.com/developers/docs/resources/message#embed-object-embed-limits
  pub fn new(
    title: impl Display,
    entries: &[&'a (dyn PageRow + Send + Sync)],
    entries_per_page: usize,
  ) -> Self {
    // Limit entries per page to embed fields limit (25)
    let entries_per_page = if entries_per_page > 25 {
      25
    } else {
      entries_per_page
    };

    let entries_count = entries.len();
    let page_count = if entries_count == 0 {
      1
    } else {
      (entries_count / entries_per_page) + usize::from(entries_count % entries_per_page > 0)
    };

    let page_data = if entries_count == 0 {
      vec![PaginationPage {
        entries: vec![],
        page_number: 0,
        page_count: 1,
        entries_per_page,
      }]
    } else {
      entries
        .chunks(entries_per_page)
        .enumerate()
        .map(|(page_number, entries)| PaginationPage {
          entries: entries.to_vec(),
          page_number,
          page_count,
          entries_per_page,
        })
        .collect()
    };

    Self {
      title: title.to_string(),
      page_data,
      page_count,
    }
  }

  pub fn get_page_count(&self) -> usize {
    self.page_count
  }

  pub fn get_last_page_number(&self) -> usize {
    // We can do this unchecked because we use entries.is_empty on instantiation
    self.page_count - 1
  }

  pub fn get_page(&self, page: usize) -> Option<&PaginationPage> {
    self.page_data.get(page)
  }

  pub fn update_page_number(&self, current_page: usize, change_by: isize) -> usize {
    if change_by < 0 {
      if change_by.unsigned_abs() > current_page {
        self.page_count - (change_by.unsigned_abs() - current_page)
      } else {
        current_page - change_by.unsigned_abs()
      }
    } else if current_page + change_by.unsigned_abs() >= self.page_count {
      (current_page + change_by.unsigned_abs()) - self.page_count
    } else {
      current_page + change_by.unsigned_abs()
    }
  }

  pub fn create_page_embed(&self, page: usize, page_type: PageType) -> CreateEmbed {
    let mut embed = BloomBotEmbed::new();
    let page = self.get_page(page);

    if let Some(page) = page {
      // If it is a valid page that is empty, it must be page 0.
      // This implies that there are no entries to display.
      if page.is_empty() {
        embed = embed
          .title(self.title.clone())
          .description("No entries have been added yet.");

        embed
      } else {
        page.to_embed(self.title.as_str(), page_type).clone()
      }
    } else {
      // This should never happen unless we have a bug in our pagination code
      embed = embed
        .title(self.title.clone())
        .description("This page does not exist.");

      embed
    }
  }

  /// Receives a [`Paginator`] initialized with [`Paginator::new()`] and initiates pagination.
  ///
  /// An optional `page` argument specifies the initial page, [`PageType`] allows for multiple
  /// page variations, and [`Visibility`] determines whether the pagination is displayed publicly
  /// or ephemerally, meaning via private in-channel messages.
  pub async fn paginate(
    self,
    ctx: Context<'_>,
    page: Option<usize>,
    page_type: PageType,
    visibility: Visibility,
  ) -> Result<()> {
    let ephemeral = match visibility {
      Visibility::Public => false,
      Visibility::Ephemeral => true,
    };

    // Define some unique identifiers for the navigation buttons
    let ctx_id = ctx.id();
    let prev_button_id = format!("{ctx_id}prev");
    let next_button_id = format!("{ctx_id}next");

    let mut current_page = page.unwrap_or(0).saturating_sub(1);

    if self.get_page(current_page).is_none() {
      current_page = self.get_last_page_number();
    }

    let first_page = self.create_page_embed(current_page, page_type);

    ctx
      .send({
        let mut f = CreateReply::default();
        if self.get_page_count() > 1 {
          f = f.components(vec![CreateActionRow::Buttons(vec![
            CreateButton::new(&prev_button_id).label("Previous"),
            CreateButton::new(&next_button_id).label("Next"),
          ])]);
        }
        f.embeds = vec![first_page];
        f.ephemeral(ephemeral)
      })
      .await?;

    // Loop through incoming interactions with the navigation buttons
    while let Some(press) = ComponentInteractionCollector::new(ctx)
      // We defined our button IDs to start with `ctx_id`. If they don't, some other command's
      // button was pressed
      .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
      // Timeout when no navigation button has been pressed for 24 hours
      .timeout(Duration::from_secs(3600 * 24))
      .await
    {
      // Depending on which button was pressed, go to next or previous page
      if press.data.custom_id == next_button_id {
        current_page = self.update_page_number(current_page, 1);
      } else if press.data.custom_id == prev_button_id {
        current_page = self.update_page_number(current_page, -1);
      } else {
        // This is an unrelated button interaction
        continue;
      }

      // Update the message with the new page contents
      press
        .create_response(
          ctx,
          CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
              .embed(self.create_page_embed(current_page, page_type)),
          ),
        )
        .await?;
    }
    Ok(())
  }
}

#[allow(clippy::module_name_repetitions)]
pub struct PaginationPage<'a> {
  entries: Vec<&'a (dyn PageRow + Send + Sync)>,
  page_number: usize,
  page_count: usize,
  entries_per_page: usize,
}

impl PaginationPage<'_> {
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  fn to_embed(&self, title: &str, page_type: PageType) -> CreateEmbed {
    let mut embed = BloomBotEmbed::new().title(title).description(format!(
      "Showing entries {} to {}.",
      (self.page_number * self.entries_per_page) + 1,
      (self.page_number * self.entries_per_page) + self.entries.len()
    ));

    let fields: Vec<(String, String, bool)> = self
      .entries
      .iter()
      .map(|entry| (entry.title(page_type), entry.body(), false))
      .collect();
    embed = embed.fields(fields);

    embed = embed.footer(CreateEmbedFooter::new(format!(
      "Page {} of {}",
      self.page_number + 1,
      self.page_count
    )));

    embed
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_update_page_number() {
    let test_data = Paginator {
      page_data: vec![],
      page_count: 4,
      title: "title".to_string(),
    };

    assert_eq!(Paginator::update_page_number(&test_data, 1, -2), 3);
    assert_eq!(Paginator::update_page_number(&test_data, 3, -1), 2);
    assert_eq!(Paginator::update_page_number(&test_data, 3, 2), 1);
    assert_eq!(Paginator::update_page_number(&test_data, 1, 2), 3);
  }
}
