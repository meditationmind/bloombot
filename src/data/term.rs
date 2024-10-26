use crate::commands::helpers::pagination::{PageRow, PageType};

#[derive(Debug)]
pub struct Term {
  pub id: String,
  pub name: String,
  pub meaning: String,
  pub usage: Option<String>,
  pub links: Option<Vec<String>>,
  pub category: Option<String>,
  pub aliases: Option<Vec<String>>,
}

impl PageRow for Term {
  fn title(&self, _page_type: PageType) -> String {
    format!("__{}__", self.name.clone())
  }

  fn body(&self) -> String {
    /*let meaning = match self.meaning.len() > 157 {
      true => {
        let truncate = self.meaning.chars().take(157).collect::<String>();
        let truncate_split = match truncate.rsplit_once(' ') {
          Some(pair) => pair.0.to_string(),
          None => truncate
        };
        let truncate_final = if truncate_split.chars().last().unwrap().is_ascii_punctuation() {
          truncate_split.chars().take(truncate_split.chars().count() - 1).collect::<String>()
        } else {
          truncate_split
        };
        format!("{}...", truncate_final)
      },
      false => self.meaning.clone(),
    };
    meaning*/
    self.meaning.clone()
  }
}

#[derive(Debug, sqlx::FromRow)]
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
