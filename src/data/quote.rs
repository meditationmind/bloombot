use crate::commands::helpers::pagination::{PageRow, PageType};

pub struct Quote {
  pub id: String,
  pub quote: String,
  pub author: Option<String>,
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
