#![allow(
  clippy::cast_possible_truncation,
  clippy::cast_precision_loss,
  clippy::cast_sign_loss
)]

use crate::commands::stats::StatsType;
use crate::database::{Timeframe, TimeframeStats};
use anyhow::Result;
use charts_rs::{svg_to_webp, BarChart, Box, Series};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Chart<'a> {
  file: tokio::fs::File,
  path: std::path::PathBuf,
  filename: &'a str,
}

impl Chart<'_> {
  pub async fn new() -> Result<Self> {
    let filename = "attachment.webp";
    let path = std::env::temp_dir().with_file_name(filename);
    let file = tokio::fs::File::create(&path).await?;

    Ok(Self { file, path, filename })
  }

  pub async fn stats(
    mut self,
    stats: &[TimeframeStats],
    timeframe: &Timeframe,
    stats_type: &StatsType,
    bar_color: (u8, u8, u8, u8),
    light_mode: bool,
    labels: bool,
  ) -> Result<Self> {
    let header = match stats_type {
      StatsType::MeditationMinutes => String::from("Meditation Minutes"),
      StatsType::MeditationCount => String::from("Meditation Sessions"),
    };

    let series_name = match stats_type {
      StatsType::MeditationMinutes => String::from("Minutes"),
      StatsType::MeditationCount => String::from("Sessions"),
    };

    let now = chrono::Utc::now();

    if stats.len() != 12 {
      return Err(anyhow::anyhow!("Not enough stats to draw chart"));
    }

    let stats = match stats_type {
      StatsType::MeditationMinutes => stats
        .iter()
        .map(|x| x.sum.unwrap_or(0) as f32)
        .collect::<Vec<f32>>(),
      StatsType::MeditationCount => stats
        .iter()
        .map(|x| x.count.unwrap_or(0) as f32)
        .collect::<Vec<f32>>(),
    };

    /*
    let minutes = stats
      .iter()
      .map(|x| x.sum.unwrap_or(0) as f32)
      .collect::<Vec<f32>>();
    let sessions = stats
      .iter()
      .map(|x| x.count.unwrap_or(0) as f32)
      .collect::<Vec<f32>>();
    */

    let series_data = vec![
      Series::new(series_name, stats),
      //Series::new("Minutes".to_string(), minutes),
      //Series::new("Sessions".to_string(), sessions),
    ];

    let mut x_labels: Vec<String> = vec![];
    for n in 1..13 {
      let label = match timeframe {
        Timeframe::Daily => {
          let date = now - chrono::Duration::days(12 - n);
          date.format("%m/%d").to_string()
        }
        Timeframe::Weekly => {
          let date = now - chrono::Duration::weeks(12 - n);
          date.format("%m/%d").to_string()
        }
        Timeframe::Monthly => {
          let date = now - chrono::Duration::days((12 * 30) - (n * 30));
          date.format("%y/%m").to_string()
        }
        Timeframe::Yearly => {
          let date = now - chrono::Duration::days((12 * 365) - (n * 365));
          date.format("%Y").to_string()
        }
      };
      x_labels.push(label);
    }

    let mut bar_chart = BarChart::new(series_data, x_labels);
    bar_chart.height = 480.0;
    bar_chart.width = 640.0;
    bar_chart.margin = Box {
      left: 15.0,
      top: 15.0,
      right: 35.0,
      bottom: 15.0,
    };
    bar_chart.grid_stroke_width = 0.5;
    bar_chart.legend_show = Some(false);
    //bar_chart.legend_category = LegendCategory::Rect;
    //bar_chart.legend_align = Align::Left;
    //bar_chart.legend_font_size = 18.0;
    //bar_chart.legend_margin = Some(Box {
    //  top: 35.0,
    //  left: 25.0,
    //  bottom: 25.0,
    //  ..Default::default()
    //});
    bar_chart.title_text = header;
    bar_chart.title_font_size = 30.0;
    bar_chart.title_height = 35.0;
    bar_chart.title_margin = Some(Box {
      left: 0.0,
      top: 5.0,
      right: 0.0,
      bottom: 10.0,
    });
    bar_chart.x_axis_name_rotate = 120.0;
    bar_chart.x_axis_name_gap = 5.0;
    bar_chart.x_boundary_gap = Some(true);
    bar_chart.x_axis_font_size = 14.0;
    bar_chart.x_axis_height = 40.0;
    bar_chart.y_axis_configs[0].axis_font_size = 22.0;
    bar_chart.y_axis_configs[0].axis_split_number = 7;
    bar_chart.series_colors = vec![(bar_color.0, bar_color.1, bar_color.2).into()];
    bar_chart.series_list[0].label_show = labels;
    //bar_chart.series_list[1].category = Some(SeriesCategory::Line);
    //bar_chart.series_list[1].y_axis_index = 1;
    //bar_chart.series_list[1].label_show = true;

    if light_mode {
      bar_chart.background_color = (227, 229, 232).into();
      bar_chart.grid_stroke_color = (140, 140, 140).into();
      bar_chart.title_font_color = (30, 31, 34).into();
      bar_chart.legend_font_color = (30, 31, 34).into();
      bar_chart.x_axis_font_color = (30, 31, 34).into();
      bar_chart.series_label_font_color = (30, 31, 34).into();
      bar_chart.x_axis_stroke_color = (30, 31, 34).into();
      bar_chart.y_axis_configs[0].axis_font_color = (30, 31, 34).into();
    } else {
      bar_chart.background_color = (30, 31, 34).into();
      bar_chart.grid_stroke_color = (185, 184, 206).into();
      bar_chart.title_font_color = (216, 217, 218).into();
      bar_chart.legend_font_color = (216, 217, 218).into();
      bar_chart.x_axis_font_color = (216, 217, 218).into();
      bar_chart.series_label_font_color = (216, 217, 218).into();
      bar_chart.x_axis_stroke_color = (185, 184, 206).into();
      bar_chart.y_axis_configs[0].axis_font_color = (216, 217, 218).into();
    }

    let svg = bar_chart.svg()?;
    let webp = svg_to_webp(&svg)?;

    tokio::io::AsyncWriteExt::write_all(&mut self.file, &webp).await?;
    tokio::io::AsyncWriteExt::flush(&mut self.file).await?;

    Ok(Self {
      file: self.file,
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
    tokio::fs::remove_file(self.path()).await?;
    Ok(())
  }
}
