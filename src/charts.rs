#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use crate::commands::stats::StatsType;
use crate::database::{Timeframe, TimeframeStats};
use anyhow::{Context, Result};
use plotters::prelude::*;
use std::path::PathBuf;
use tempfile::NamedTempFile;

pub struct Chart {
  file: NamedTempFile,
}

pub struct ChartDrawer {
  file: NamedTempFile,
}

fn next_largest_factor(x: u32) -> u32 {
  let n = x.checked_ilog10().unwrap_or(0) + 1;
  let factor = 10 * n;

  // Find the current quotient of x divided by 5n
  let quotient = x / factor;

  // Find the next largest number that is a multiple of 5n
  (quotient + 1) * factor
}

impl ChartDrawer {
  pub fn new() -> Result<Self> {
    // let file = NamedTempFile::new_in("/tmp").with_context(|| "Could not create new temporary file")?;
    let file = tempfile::Builder::new()
      .prefix("attachment")
      .suffix(".png")
      .tempfile()?;

    Ok(Self { file })
  }

  #[allow(clippy::unused_async)]
  pub async fn draw(
    self,
    stats: &[TimeframeStats],
    timeframe: &Timeframe,
    stats_type: &StatsType,
    bar_color: (u8, u8, u8, f64),
    light_mode: bool,
  ) -> Result<Chart> {
    let path = self.file.path().to_path_buf();

    let text_color = if light_mode { &BLACK } else { &WHITE };

    let background_color = if light_mode { &WHITE } else { &BLACK };

    let root = BitMapBackend::new(&path, (640, 480)).into_drawing_area();
    //root.fill(&WHITE).unwrap();
    root.fill(background_color)?;

    let header = match stats_type {
      StatsType::MeditationMinutes => String::from("# of Minutes"),
      StatsType::MeditationCount => String::from("# of Sessions"),
    };

    let upper_bound = match stats_type {
      StatsType::MeditationMinutes => {
        let largest = stats.iter().map(|x| x.sum.unwrap_or(0)).max().unwrap_or(0);
        next_largest_factor(largest.try_into()?)
      }
      StatsType::MeditationCount => {
        let largest = stats
          .iter()
          .map(|x| x.count.unwrap_or(0))
          .max()
          .unwrap_or(0);
        next_largest_factor(largest.try_into()?)
      }
    };

    let mut chart = ChartBuilder::on(&root)
      .caption(header, ("sans-serif", 35).into_font().color(text_color))
      .margin(15)
      .margin_right(45)
      .x_label_area_size(45)
      .y_label_area_size(50)
      .build_cartesian_2d(0u32..13u32, 0u32..upper_bound)
      .with_context(|| "Could not build chart")?;

    let now = chrono::Utc::now();

    chart
      .configure_mesh()
      .axis_style(text_color)
      .light_line_style(text_color.mix(0.1))
      .bold_line_style(text_color.mix(0.2))
      .x_label_style(("sans-serif", 25).into_font().color(text_color))
      .y_label_style(("sans-serif", 25).into_font().color(text_color))
      .x_label_formatter(&|x| {
        // Dates
        let x: i64 = <i64>::from(*x);
        match timeframe {
          Timeframe::Daily => {
            let date = now - chrono::Duration::days(12 - x);
            date.format("%m/%d").to_string()
          }
          Timeframe::Weekly => {
            let date = now - chrono::Duration::weeks(12 - x);
            date.format("%m/%d").to_string()
          }
          Timeframe::Monthly => {
            let date = now - chrono::Duration::days((12 * 30) - (x * 30));
            date.format("%y/%m").to_string()
          }
          Timeframe::Yearly => {
            let date = now - chrono::Duration::days((12 * 365) - (x * 365));
            date.format("%Y").to_string()
          }
        }
      })
      .y_label_formatter(&|y| {
        let mut index: usize = 0;
        let base: f64 = 1000.0;
        let mut value: f64 = <f64>::from(*y);

        loop {
          if value < base {
            break;
          }

          value /= base;
          index += 1;
        }

        let unit = match index {
          1 => "K",
          2 => "M",
          3 => "B",
          _ => "",
        };

        let y_label = format!("{value}{unit}");

        y_label
      })
      .draw()?;

    let shape_color = ShapeStyle {
      //color: RGBAColor(253, 172, 46, 1.0),
      color: RGBAColor(bar_color.0, bar_color.1, bar_color.2, bar_color.3),
      filled: true,
      stroke_width: 1,
    };

    // We want to throw an error if there are not enough stats to draw a chart
    if stats.len() != 12 {
      return Err(anyhow::anyhow!("Not enough stats to draw chart"));
    }

    let stats = match stats_type {
      StatsType::MeditationMinutes => stats
        .iter()
        .map(|x| {
          x.sum
            .unwrap_or(0)
            .try_into()
            .expect("sum should not be larger than u32")
        })
        .collect::<Vec<u32>>(),
      StatsType::MeditationCount => stats
        .iter()
        .map(|x| {
          x.count
            .unwrap_or(0)
            .try_into()
            .expect("count should not be larger than u32")
        })
        .collect::<Vec<u32>>(),
    };

    chart.draw_series((0..12).map(|x: u32| {
      let height = stats[x as usize];
      let mut rect = Rectangle::new([(x + 1, 0), (x + 1, height)], shape_color.filled());

      rect.set_margin(0, 0, 15, 15);

      rect
    }))?;

    root.present().with_context(|| "Could not present chart")?;

    Ok(Chart { file: self.file })
  }
}

impl Chart {
  pub fn get_file_path(&self) -> PathBuf {
    self.file.path().to_path_buf()
  }

  pub fn get_file_name(&self) -> String {
    if let Some(filename) = self.file.path().file_name() {
      filename.to_string_lossy().to_string()
    } else {
      // This should never happen, but if it does, assign default filename
      String::from("attachment.png")
    }
  }

  pub fn get_attachment_url(&self) -> String {
    format!("attachment://{}", self.get_file_name())
  }
}
