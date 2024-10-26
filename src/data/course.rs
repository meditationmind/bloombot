use crate::commands::helpers::pagination::{PageRow, PageType};
use poise::serenity_prelude::{self as serenity, Mentionable};

pub struct Course {
  pub course_name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
}

impl PageRow for Course {
  fn title(&self, _page_type: PageType) -> String {
    self.course_name.clone()
  }

  fn body(&self) -> String {
    format!(
      "Participants: {}\nGraduates: {}",
      self.participant_role.mention(),
      self.graduate_role.mention()
    )
  }
}

pub struct ExtendedCourse {
  pub course_name: String,
  pub participant_role: serenity::RoleId,
  pub graduate_role: serenity::RoleId,
  pub guild_id: serenity::GuildId,
}
