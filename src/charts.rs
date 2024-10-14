#![allow(
  clippy::cast_possible_truncation,
  clippy::cast_precision_loss,
  clippy::cast_sign_loss
)]

use crate::commands::stats::{ChartStyle, LeaderboardType, SortBy, StatsType};
use crate::database::{Timeframe, TimeframeStats};
use anyhow::Result;
use charts_rs::{
  svg_to_webp, Align, BarChart, Box, LegendCategory, Series, TableCellStyle, TableChart,
};
use chrono::{Datelike, NaiveDate};
use std::path::PathBuf;

#[derive(Debug)]
pub struct Chart<'a> {
  file: tokio::fs::File,
  path: std::path::PathBuf,
  filename: &'a str,
}

impl<'a> Chart<'a> {
  pub async fn new() -> Result<Self> {
    let filename = "attachment.webp";
    let path = std::env::temp_dir().with_file_name(filename);
    let file = tokio::fs::File::create(&path).await?;

    Ok(Self {
      file,
      path,
      filename,
    })
  }

  pub async fn new_with_name(filename: &'a str) -> Result<Self> {
    let path = std::env::temp_dir().with_file_name(filename);
    let file = tokio::fs::File::create(&path).await?;

    Ok(Self {
      file,
      path,
      filename,
    })
  }

  pub async fn open(filename: &'a str) -> Result<Self> {
    let path = std::env::temp_dir().with_file_name(filename);
    let file = tokio::fs::File::open(&path).await?;

    Ok(Self {
      file,
      path,
      filename,
    })
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn stats(
    mut self,
    stats: &[TimeframeStats],
    timeframe: &Timeframe,
    offset: i16,
    stats_type: &StatsType,
    chart_style: &ChartStyle,
    bar_color: (u8, u8, u8, u8),
    light_mode: bool,
  ) -> Result<Self> {
    if stats.len() != 12 {
      return Err(anyhow::anyhow!("Not enough stats to draw chart"));
    }

    let title = if let ChartStyle::BarCombined = chart_style {
      String::new()
    } else {
      match stats_type {
        StatsType::MeditationMinutes => String::from("Meditation Minutes"),
        StatsType::MeditationCount => String::from("Meditation Sessions"),
      }
    };

    let series_data = if let ChartStyle::BarCombined = chart_style {
      let minutes = stats
        .iter()
        .map(|x| x.sum.unwrap_or(0) as f32)
        .collect::<Vec<f32>>();
      let sessions = stats
        .iter()
        .map(|x| x.count.unwrap_or(0) as f32)
        .collect::<Vec<f32>>();
      vec![
        Series::new("Minutes".to_string(), minutes),
        Series::new("Sessions".to_string(), sessions),
      ]
    } else {
      let (series_name, stats) = match stats_type {
        StatsType::MeditationMinutes => (
          String::from("Minutes"),
          stats
            .iter()
            .map(|x| x.sum.unwrap_or(0) as f32)
            .collect::<Vec<f32>>(),
        ),
        StatsType::MeditationCount => (
          String::from("Sessions"),
          stats
            .iter()
            .map(|x| x.count.unwrap_or(0) as f32)
            .collect::<Vec<f32>>(),
        ),
      };
      vec![Series::new(series_name, stats)]
    };

    let now = chrono::Utc::now();
    let mut x_labels: Vec<String> = vec![];
    for n in 1..13 {
      let label = match timeframe {
        Timeframe::Daily => {
          let date = (now + chrono::Duration::minutes(offset.into())) - chrono::Duration::days(12 - n);
          date.format("%m/%d").to_string()
        }
        Timeframe::Weekly => {
          let date = now.date_naive().week(chrono::Weekday::Mon).first_day()
            - chrono::Duration::weeks(12 - n);
          date.format("%m/%d").to_string()
        }
        Timeframe::Monthly => {
          let date = NaiveDate::from_ymd_opt(
            now.year(),
            now
              .month()
              .saturating_sub(12u32.saturating_sub(n.try_into()?)),
            1,
          )
          .unwrap_or_else(|| now.date_naive() - chrono::Duration::days((12 * 30) - (n * 30)));
          date.format("%y/%m").to_string()
        }
        Timeframe::Yearly => {
          let date = NaiveDate::from_ymd_opt(
            now
              .year()
              .saturating_sub(12i32.saturating_sub(n.try_into()?)),
            1,
            1,
          )
          .unwrap_or_else(|| now.date_naive() - chrono::Duration::days((12 * 365) - (n * 365)));
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
    bar_chart.title_text = title;
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
    bar_chart.series_colors = vec![
      (bar_color.0, bar_color.1, bar_color.2, bar_color.3).into(),
      (bar_color.0, bar_color.1, bar_color.2, 190).into(),
    ];
    bar_chart.series_list[0].label_show = false;

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

    if let ChartStyle::Bar = chart_style {
    } else {
      bar_chart.series_fill = true;
      bar_chart.series_smooth = true;
      bar_chart.series_list[0].category = Some(charts_rs::SeriesCategory::Line);
      bar_chart.series_list[0].label_show = true;
      bar_chart.series_label_font_size = 16.0;
      bar_chart.series_label_font_weight = Some("bold".to_string());
      bar_chart.series_label_formatter = "{t}".to_string();
      bar_chart.x_boundary_gap = Some(false);
      bar_chart.y_axis_hidden = true;
      bar_chart.margin = Box {
        left: 45.0,
        top: 15.0,
        right: 45.0,
        bottom: 15.0,
      };
      // Add a second line
      if let ChartStyle::BarCombined = chart_style {
        bar_chart.y_axis_hidden = false;
        bar_chart.y_axis_configs[0].axis_width = Some(0.0);
        bar_chart.series_list[0].category = Some(charts_rs::SeriesCategory::Bar);
        bar_chart.series_list[1].category = Some(charts_rs::SeriesCategory::Bar);
        bar_chart.series_list[1].y_axis_index = 1;
        bar_chart.series_list[1].label_show = true;
        bar_chart.legend_show = Some(true);
        bar_chart.legend_category = LegendCategory::Rect;
        bar_chart.legend_align = Align::Left;
        bar_chart.legend_font_size = 16.0;
        bar_chart.legend_margin = Some(Box {
          top: 10.0,
          left: 0.0,
          bottom: 30.0,
          right: 0.0,
        });
      }
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

  pub async fn leaderboard(
    mut self,
    mut data: Vec<Vec<String>>,
    timeframe: &Timeframe,
    sort_by: &SortBy,
    leaderboard_type: &LeaderboardType,
    light_mode: bool,
  ) -> Result<Self> {
    match leaderboard_type {
      LeaderboardType::Top5 => data.truncate(6),
      LeaderboardType::Top10 => data.truncate(11),
    };
    let title = match leaderboard_type {
      LeaderboardType::Top5 => String::from("Leaderboard (Top 5)"),
      LeaderboardType::Top10 => String::from("Leaderboard (Top 10)"),
    };
    let subtitle = match timeframe {
      Timeframe::Daily => chrono::Utc::now().format("%B %-d, %Y").to_string(),
      Timeframe::Weekly => format!(
        "Week starting {}",
        chrono::Utc::now()
          .date_naive()
          .week(chrono::Weekday::Sun)
          .first_day()
          .format("%B %d"),
      ),
      Timeframe::Monthly => chrono::Utc::now().format("%B %Y").to_string(),
      Timeframe::Yearly => chrono::Utc::now().format("%Y").to_string(),
    };

    let mut cell_styles: Vec<TableCellStyle> = vec![];
    for i in 1..=data.len() {
      cell_styles.push(TableCellStyle {
        font_color: if light_mode {
          Some((102, 103, 108).into())
        } else {
          Some((235, 236, 236).into())
        },
        font_weight: Some("bold".to_string()),
        background_color: if light_mode {
          Some((202, 204, 207).into())
        } else {
          Some((44, 46, 50).into())
        },
        indexes: vec![
          i,
          match sort_by {
            SortBy::Minutes => 1,
            SortBy::Sessions => 2,
            SortBy::Streak => 3,
          },
        ],
      });
    }

    let mut leaderboard = TableChart::new(data);
    leaderboard.height = 400.0;
    leaderboard.width = 500.0;
    leaderboard.title_text = title;
    leaderboard.title_font_size = 22.0;
    leaderboard.title_font_weight = Some("bold".to_string());
    leaderboard.title_height = 40.0;
    leaderboard.title_margin = Some(Box {
      left: 0.0,
      top: 10.0,
      right: 0.0,
      bottom: 0.0,
    });
    leaderboard.sub_title_text = subtitle;
    leaderboard.sub_title_font_color = (142, 142, 143).into();
    leaderboard.sub_title_margin = Some(Box {
      left: 0.0,
      top: 0.0,
      right: 0.0,
      bottom: 10.0,
    });
    leaderboard.spans = vec![0.4, 0.2, 0.2, 0.2];
    leaderboard.text_aligns = vec![Align::Left, Align::Center, Align::Center, Align::Center];
    leaderboard.header_font_size = 16.0;
    leaderboard.header_font_weight = Some("bold".to_string());
    leaderboard.header_row_padding = Box {
      left: 15.0,
      top: 3.0,
      right: 15.0,
      bottom: 2.0,
    };
    leaderboard.body_row_padding = Box {
      left: 15.0,
      top: 5.0,
      right: 15.0,
      bottom: 3.0,
    };
    leaderboard.cell_styles = cell_styles;

    if light_mode {
      leaderboard.background_color = (227, 229, 232).into();
      leaderboard.border_color = (197, 199, 200).into();
      leaderboard.header_background_color = (180, 183, 187).into();
      leaderboard.header_font_color = (255, 255, 255).into();
      leaderboard.title_font_color = (64, 66, 72).into();
      leaderboard.body_font_color = (64, 66, 72).into();
      leaderboard.body_background_colors = vec![(227, 229, 232).into()];
    } else {
      leaderboard.background_color = (30, 31, 34).into();
      leaderboard.border_color = (64, 66, 72).into();
      leaderboard.header_background_color = (64, 66, 72).into();
      leaderboard.header_font_color = (235, 236, 236).into();
      leaderboard.title_font_color = (216, 217, 218).into();
      leaderboard.body_font_color = (216, 217, 218).into();
      leaderboard.body_background_colors = vec![(30, 31, 34).into()];
    }

    let svg = leaderboard.svg()?;
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
