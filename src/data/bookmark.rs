use crate::commands::helpers::pagination::{PageRow, PageType};
use chrono::Utc;

pub struct Bookmark {
  pub id: String,
  pub link: String,
  pub description: Option<String>,
  pub added: chrono::DateTime<Utc>,
}

impl PageRow for Bookmark {
  fn title(&self, _page_type: PageType) -> String {
    self.link.clone()
  }

  fn body(&self) -> String {
    if let Some(description) = &self.description {
      format!(
        "> {}\n> -# Added: <t:{}:f>\n> -# ID: [{}](discord://{} \"For copying a bookmark ID on mobile. Not a working link.\")\n** **",
        description,
        self.added.timestamp(),
        self.id,
        self.id,
      )
    } else {
      format!(
        "> -# Added: <t:{}:f>\n> -# ID: [{}](discord://{} \"For copying a bookmark ID on mobile. Not a working link.\")\n** **",
        self.added.timestamp(),
        self.id,
        self.id,
      )
    }
  }
}
