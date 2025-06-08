#![allow(dead_code)]

use std::env;
use std::path::PathBuf;

use anyhow::{Result, anyhow};
use image::Rgba;
use rusttype::{Font, Scale};
use text_on_image::{FontBundle, TextJustify, VerticalAnchor, WrapBehavior};
use tokio::fs;

use crate::data::stats::Bests;

const FONT: &[u8] = include_bytes!("../assets/NotoSans-Bold.ttf");

struct Coords {
  x: i32,
  y: i32,
}

struct BestData {
  total: Coords,
  date: Coords,
}

struct Periods {
  day: BestData,
  week: BestData,
  month: BestData,
  year: BestData,
}

struct BestsImage {
  times: Periods,
  sessions: Periods,
}

const BESTS: BestsImage = BestsImage {
  times: Periods {
    day: BestData {
      total: Coords { x: 169, y: 85 },
      date: Coords { x: 169, y: 118 },
    },
    week: BestData {
      total: Coords { x: 469, y: 85 },
      date: Coords { x: 469, y: 118 },
    },
    month: BestData {
      total: Coords { x: 169, y: 179 },
      date: Coords { x: 169, y: 212 },
    },
    year: BestData {
      total: Coords { x: 469, y: 179 },
      date: Coords { x: 469, y: 212 },
    },
  },
  sessions: Periods {
    day: BestData {
      total: Coords { x: 169, y: 320 },
      date: Coords { x: 169, y: 353 },
    },
    week: BestData {
      total: Coords { x: 469, y: 320 },
      date: Coords { x: 469, y: 353 },
    },
    month: BestData {
      total: Coords { x: 169, y: 415 },
      date: Coords { x: 169, y: 448 },
    },
    year: BestData {
      total: Coords { x: 469, y: 415 },
      date: Coords { x: 469, y: 448 },
    },
  },
};

#[derive(Debug)]
pub struct Image<'a> {
  path: PathBuf,
  filename: &'a str,
}

impl<'a> Image<'a> {
  pub fn new() -> Self {
    let filename = "attachment.webp";
    let path = env::temp_dir().with_file_name(filename);

    Self { path, filename }
  }

  pub fn new_with_name(filename: &'a str) -> Self {
    let path = env::temp_dir().with_file_name(filename);

    Self { path, filename }
  }

  pub fn bests(self, bests_data: &Bests) -> Result<Self> {
    // For debug build
    // let mut background = image::open("./assets/bests-template.png")?;
    let mut background = image::open("/app/assets/bests-template.png")?;

    // Prepare font
    let font = Vec::from(FONT);
    let Some(font) = Font::try_from_vec(font) else {
      return Err(anyhow!("Failed to initialize font"));
    };
    let stat_font = FontBundle::new(&font, Scale { x: 32., y: 32. }, Rgba([217, 217, 217, 255]));
    let date_font = FontBundle::new(&font, Scale { x: 22., y: 22. }, Rgba([150, 150, 150, 255]));

    // Write time bests (day) on image
    if let Some(day) = &bests_data.times.day {
      text_on_image::text_on_image(
        &mut background,
        day.total_to_hms(),
        &stat_font,
        BESTS.times.day.total.x,
        BESTS.times.day.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        day.date_to_day(),
        &date_font,
        BESTS.times.day.date.x,
        BESTS.times.day.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    } else {
      return Err(anyhow!("No tracking data found"));
    }

    // Write time bests (week) on image
    if let Some(week) = &bests_data.times.week {
      text_on_image::text_on_image(
        &mut background,
        week.total_to_hms(),
        &stat_font,
        BESTS.times.week.total.x,
        BESTS.times.week.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        week.date_to_week(),
        &date_font,
        BESTS.times.week.date.x,
        BESTS.times.week.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Write time bests (month) on image
    if let Some(month) = &bests_data.times.month {
      text_on_image::text_on_image(
        &mut background,
        month.total_to_hms(),
        &stat_font,
        BESTS.times.month.total.x,
        BESTS.times.month.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        month.date_to_month(),
        &date_font,
        BESTS.times.month.date.x,
        BESTS.times.month.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Write time bests (year) on image
    if let Some(year) = &bests_data.times.year {
      text_on_image::text_on_image(
        &mut background,
        year.total_to_hms(),
        &stat_font,
        BESTS.times.year.total.x,
        BESTS.times.year.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        year.date_to_year(),
        &date_font,
        BESTS.times.year.date.x,
        BESTS.times.year.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Write session bests (day) on image
    if let Some(day) = &bests_data.sessions.day {
      text_on_image::text_on_image(
        &mut background,
        day.total_to_sessions(),
        &stat_font,
        BESTS.sessions.day.total.x,
        BESTS.sessions.day.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        day.date_to_day(),
        &date_font,
        BESTS.sessions.day.date.x,
        BESTS.sessions.day.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Write session bests (week) on image
    if let Some(week) = &bests_data.sessions.week {
      text_on_image::text_on_image(
        &mut background,
        week.total_to_sessions(),
        &stat_font,
        BESTS.sessions.week.total.x,
        BESTS.sessions.week.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        week.date_to_week(),
        &date_font,
        BESTS.sessions.week.date.x,
        BESTS.sessions.week.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Write session bests (month) on image
    if let Some(month) = &bests_data.sessions.month {
      text_on_image::text_on_image(
        &mut background,
        month.total_to_sessions(),
        &stat_font,
        BESTS.sessions.month.total.x,
        BESTS.sessions.month.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        month.date_to_month(),
        &date_font,
        BESTS.sessions.month.date.x,
        BESTS.sessions.month.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    //  Write session bests (year) on image
    if let Some(year) = &bests_data.sessions.year {
      text_on_image::text_on_image(
        &mut background,
        year.total_to_sessions(),
        &stat_font,
        BESTS.sessions.year.total.x,
        BESTS.sessions.year.total.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
      text_on_image::text_on_image(
        &mut background,
        year.date_to_year(),
        &date_font,
        BESTS.sessions.year.date.x,
        BESTS.sessions.year.date.y,
        TextJustify::Center,
        VerticalAnchor::Center,
        WrapBehavior::NoWrap,
      );
    }

    // Save image
    background.save(&self.path)?;

    Ok(Self {
      path: self.path,
      filename: self.filename,
    })
  }

  pub fn path(&self) -> PathBuf {
    self.path.clone()
  }

  pub fn url(&self) -> String {
    format!("attachment://{}", self.filename)
  }

  pub async fn remove(&self) -> Result<()> {
    fs::remove_file(self.path()).await?;
    Ok(())
  }
}
