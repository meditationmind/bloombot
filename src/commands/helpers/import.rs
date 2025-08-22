#![allow(dead_code)]

use std::str::FromStr;

use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDateTime, TimeDelta, Utc};
use csv::{Reader, WriterBuilder};
use poise::ChoiceParameter;
use poise::serenity_prelude::{GuildId, UserId};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::commands::import::Type;
use crate::data::meditation::Meditation;

pub struct Import {
  pub data: Vec<BloomRecord>,
  pub minutes: i32,
  pub seconds: i32,
  pub source: String,
}

#[derive(Debug, Serialize)]
pub struct BloomRecord {
  occurred_at: DateTime<Utc>,
  meditation_minutes: i32,
  meditation_seconds: i32,
}

#[derive(ChoiceParameter)]
pub enum Source {
  #[name = "Apple Health"]
  AppleHealth,
  #[name = "Finch Breathing Sessions"]
  FinchBreathing,
  #[name = "Finch Meditation Sessions"]
  FinchMeditation,
  #[name = "Insight Timer"]
  InsightTimer,
  #[name = "VA Mindfulness Coach"]
  MindfulnessCoach,
  #[name = "Waking Up"]
  WakingUp,
}

#[derive(Debug, PartialEq, Deserialize)]
struct AppleHealthRecord {
  #[serde(rename = "App Name")]
  app_name: String,
  #[serde(rename = "Start Time")]
  occurred_at: DateTime<Utc>,
  #[serde(rename = "Minutes")]
  meditation_minutes: i32,
  #[serde(rename = "Seconds")]
  meditation_seconds: i32,
}

#[derive(Debug, Deserialize)]
struct InsightTimerRecord {
  #[serde(rename = "Started At")]
  start_time: String,
  #[serde(rename = "Duration")]
  duration: String,
  #[serde(rename = "Preset")]
  preset: Option<String>,
  #[serde(rename = "Activity")]
  activity: String,
}

#[derive(Debug, Deserialize)]
struct MindfulnessCoachRecord {
  #[serde(rename = "Date")]
  date: String,
  #[serde(rename = "Duration")]
  duration: String,
}

#[derive(Debug, Deserialize)]
struct WakingUpRecord {
  #[serde(rename = "Finished On")]
  date: String,
  #[serde(rename = "Title")]
  title: String,
  #[serde(rename = "Duration")]
  duration: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct FinchTimerSession {
  data: Vec<FinchTimerSessionRecord>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FinchTimerSessionRecord {
  #[serde(rename = "timerTypeIndex")]
  timer_type: i32,
  #[serde(rename = "selectedDurationSeconds")]
  selected_duration: i32,
  #[serde(rename = "startTime")]
  start_time: String,
  #[serde(rename = "completedTime")]
  completed_time: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct FinchBreathingSession {
  data: Vec<FinchBreathingSessionRecord>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FinchBreathingSessionRecord {
  breathing_type: String,
  duration: i32,
  start_time: String,
  completed_time: String,
}

pub struct SqlQueries {
  pub insert: String,
  pub delete: String,
}

impl Import {
  pub fn apple_health(
    rdr: &mut Reader<&[u8]>,
    current_data: &[Meditation],
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Result<Self> {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut total_seconds = 0;
    let mut import_source = String::new();
    let mut sources: Vec<String> = vec![];

    'result: for result in rdr.deserialize() {
      let row: AppleHealthRecord = result?;
      if !sources.contains(&row.app_name) {
        sources.push(row.app_name);
      }
      let datetime_utc = row.occurred_at;
      if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
        continue;
      }
      let minutes = row.meditation_minutes;
      for entry in current_data {
        if time_overlaps(entry, datetime_utc, minutes) {
          continue 'result;
        }
      }
      total_minutes += minutes;
      total_seconds += row.meditation_seconds;
      user_data.push(BloomRecord {
        occurred_at: datetime_utc,
        meditation_minutes: minutes,
        meditation_seconds: row.meditation_seconds,
      });
    }
    import_source.push_str("Apple Health Mindful Sessions (");
    for (i, source) in sources.iter().enumerate() {
      import_source.push_str(source);
      import_source.push_str(if i + 1 < sources.len() { ", " } else { ")" });
    }

    Ok(Self {
      data: user_data,
      minutes: total_minutes,
      seconds: total_seconds,
      source: import_source,
    })
  }

  pub fn finch_breathing(
    rdr: &mut Reader<&[u8]>,
    current_data: &[Meditation],
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Self {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut total_seconds = 0;
    let mut import_source = String::new();

    'result: for result in rdr.deserialize::<FinchBreathingSessionRecord>().flatten() {
      if !result.completed_time.is_empty()
        && let Ok(valid_starttime) =
          NaiveDateTime::parse_from_str(&result.start_time, "%a, %d %b %Y %H:%M:%S")
      {
        let datetime_utc = valid_starttime.and_utc();
        if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
          continue;
        }
        let minutes = result.duration / 60;
        if minutes < 1 {
          continue;
        }
        for entry in current_data {
          if time_overlaps(entry, datetime_utc, minutes) {
            continue 'result;
          }
        }
        total_minutes += minutes;
        total_seconds += result.duration % 60;
        user_data.push(BloomRecord {
          occurred_at: datetime_utc,
          meditation_minutes: minutes,
          meditation_seconds: result.duration % 60,
        });
      }
    }
    import_source.push_str("Finch Breathing Sessions");

    Self {
      data: user_data,
      minutes: total_minutes,
      seconds: total_seconds,
      source: import_source,
    }
  }

  pub fn finch_meditation(
    rdr: &mut Reader<&[u8]>,
    current_data: &[Meditation],
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Self {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut total_seconds = 0;
    let mut import_source = String::new();

    'result: for result in rdr.deserialize::<FinchTimerSessionRecord>().flatten() {
      if result.timer_type == 0
        && let Ok(valid_starttime) =
          NaiveDateTime::parse_from_str(&result.start_time, "%a, %d %b %Y %H:%M:%S")
      {
        let datetime_utc = valid_starttime.and_utc();
        if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
          continue;
        }
        #[allow(clippy::cast_possible_truncation)]
        let (minutes, seconds) = if let Ok(valid_endtime) =
          NaiveDateTime::parse_from_str(&result.completed_time, "%a, %d %b %Y %H:%M:%S")
        {
          let num_seconds = (valid_endtime - valid_starttime).num_seconds() as i32;
          (num_seconds / 60, num_seconds % 60)
        } else {
          (result.selected_duration / 60, result.selected_duration % 60)
        };
        if minutes < 1 {
          continue;
        }
        for entry in current_data {
          if time_overlaps(entry, datetime_utc, minutes) {
            continue 'result;
          }
        }
        total_minutes += minutes;
        total_seconds += seconds;
        user_data.push(BloomRecord {
          occurred_at: datetime_utc,
          meditation_minutes: minutes,
          meditation_seconds: seconds,
        });
      }
    }
    import_source.push_str("Finch Meditation Sessions");

    Self {
      data: user_data,
      minutes: total_minutes,
      seconds: total_seconds,
      source: import_source,
    }
  }

  pub fn insight_timer(
    rdr: &mut Reader<&[u8]>,
    current_data: &[Meditation],
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Result<Self> {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut total_seconds = 0;
    let mut import_source = String::new();

    'result: for result in rdr.deserialize::<InsightTimerRecord>().flatten() {
      if (result.activity == "PracticeType.Meditation"
        || result.activity == "Meditation"
        || result.activity == "瞑想")
        && let Ok(valid_datetime) =
          NaiveDateTime::parse_from_str(&result.start_time, "%m/%d/%Y %H:%M:%S")
      {
        let datetime_utc = valid_datetime.and_utc();
        if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
          continue;
        }
        let (minutes, seconds) = {
          let h_m_s: Vec<&str> = result.duration.split(':').collect();
          let hours = <i32 as FromStr>::from_str(h_m_s[0])?;
          let minutes = <i32 as FromStr>::from_str(h_m_s[1])?;
          let seconds = <i32 as FromStr>::from_str(h_m_s[2])?;
          ((hours * 60) + minutes, seconds)
        };
        for entry in current_data {
          if time_overlaps(entry, datetime_utc, minutes) {
            continue 'result;
          }
        }
        total_minutes += minutes;
        total_seconds += seconds;
        user_data.push(BloomRecord {
          occurred_at: datetime_utc,
          meditation_minutes: minutes,
          meditation_seconds: seconds,
        });
      }
    }
    import_source.push_str("Insight Timer");

    Ok(Self {
      data: user_data,
      minutes: total_minutes,
      seconds: total_seconds,
      source: import_source,
    })
  }

  pub fn mindfulness_coach(
    rdr: &mut Reader<&[u8]>,
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Result<Self> {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut import_source = String::new();

    for result in rdr.deserialize() {
      let row: MindfulnessCoachRecord = result?;
      if let Ok(valid_datetime) = NaiveDateTime::parse_from_str(
        format!("{} 00:00:00", &row.date).as_str(),
        "%Y-%m-%d %H:%M:%S",
      ) {
        let datetime_utc = valid_datetime.and_utc();
        if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
          continue;
        }
        if let Some(duration) = row.duration.split_whitespace().next() {
          let minutes = <i32 as FromStr>::from_str(duration)?;
          total_minutes += minutes;
          user_data.push(BloomRecord {
            occurred_at: datetime_utc,
            meditation_minutes: minutes,
            meditation_seconds: 0,
          });
        }
      }
    }
    import_source.push_str("VA Mindfulness Coach");

    Ok(Self {
      data: user_data,
      minutes: total_minutes,
      seconds: 0,
      source: import_source,
    })
  }

  pub fn waking_up(
    rdr: &mut Reader<&[u8]>,
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Result<Self> {
    let mut user_data: Vec<BloomRecord> = vec![];
    let mut total_minutes = 0;
    let mut total_seconds = 0;
    let mut import_source = String::new();

    for result in rdr.deserialize() {
      let row: WakingUpRecord = result?;
      if let Ok(valid_datetime) = NaiveDateTime::parse_from_str(
        format!("{} 00:00:00", &row.date).as_str(),
        "%m/%d/%Y %H:%M:%S",
      ) {
        let datetime_utc = valid_datetime.and_utc();
        if matches!(import_type, Type::NewEntries) && datetime_utc.le(&latest_time) {
          continue;
        }
        let minutes = <i32 as FromStr>::from_str(&row.duration)? / 60;
        let seconds = <i32 as FromStr>::from_str(&row.duration)? % 60;
        total_minutes += minutes;
        total_seconds += seconds;
        user_data.push(BloomRecord {
          occurred_at: datetime_utc,
          meditation_minutes: minutes,
          meditation_seconds: seconds,
        });
      }
    }
    import_source.push_str("Waking Up");

    Ok(Self {
      data: user_data,
      minutes: total_minutes,
      seconds: total_seconds,
      source: import_source,
    })
  }
}

impl Source {
  pub fn autodetect(rdr: &mut Reader<&[u8]>) -> Result<Self> {
    let headers = rdr.headers()?;
    if headers == vec!["App Name", "Start Time", "Minutes", "Seconds"] {
      return Ok(Self::AppleHealth);
    }
    if headers == vec!["Started At", "Duration", "Preset", "Activity"] {
      return Ok(Self::InsightTimer);
    }
    if headers == vec!["Date", "Exercise", "Duration", "Comments"] {
      return Ok(Self::MindfulnessCoach);
    }
    if headers == vec!["Finished On", "Title", "Duration"] {
      return Ok(Self::WakingUp);
    }
    if headers
      == vec![
        "timerTypeIndex",
        "selectedDurationSeconds",
        "startTime",
        "completedTime",
      ]
    {
      return Ok(Self::FinchMeditation);
    }
    if headers == vec!["breathing_type", "duration", "start_time", "completed_time"] {
      return Ok(Self::FinchBreathing);
    }
    Err(anyhow!("Unrecognized headers: {headers:?}"))
  }

  pub fn import(
    &self,
    rdr: &mut Reader<&[u8]>,
    current: &[Meditation],
    latest_time: DateTime<Utc>,
    import_type: &Type,
  ) -> Result<Import> {
    let import = match self {
      Self::AppleHealth => Import::apple_health(rdr, current, latest_time, import_type)?,
      Self::FinchBreathing => Import::finch_breathing(rdr, current, latest_time, import_type),
      Self::FinchMeditation => Import::finch_meditation(rdr, current, latest_time, import_type),
      Self::InsightTimer => Import::insight_timer(rdr, current, latest_time, import_type)?,
      Self::MindfulnessCoach => Import::mindfulness_coach(rdr, latest_time, import_type)?,
      Self::WakingUp => Import::waking_up(rdr, latest_time, import_type)?,
    };
    Ok(import)
  }
}

impl FinchTimerSession {
  pub fn to_csv(content: &Vec<u8>) -> Result<Vec<u8>> {
    let mut entries: Vec<FinchTimerSessionRecord> = vec![];
    let records: FinchTimerSession = serde_json::from_slice(content.as_slice())?;
    for record in records.data {
      entries.push(record);
    }
    let mut wtr = WriterBuilder::new().from_writer(vec![]);
    for entry in entries {
      wtr.serialize(entry)?;
    }
    let csv = wtr.into_inner()?;

    Ok(csv)
  }
}

impl FinchBreathingSession {
  pub fn to_csv(content: &Vec<u8>) -> Result<Vec<u8>> {
    let mut entries: Vec<FinchBreathingSessionRecord> = vec![];
    let records: FinchBreathingSession = serde_json::from_slice(content.as_slice())?;
    for record in records.data {
      entries.push(record);
    }
    let mut wtr = WriterBuilder::new().from_writer(vec![]);
    for entry in entries {
      wtr.serialize(entry)?;
    }
    let csv = wtr.into_inner()?;

    Ok(csv)
  }
}

impl SqlQueries {
  pub fn generate(guild_id: GuildId, user_id: UserId, user_data: &[BloomRecord]) -> Self {
    let mut insert_query =
    "INSERT INTO meditation (record_id, user_id, meditation_minutes, meditation_seconds, guild_id, occurred_at) VALUES"
      .to_owned();
    let mut delete_query = "DELETE FROM meditation WHERE record_id IN (".to_owned();
    for (i, record) in user_data.iter().enumerate() {
      let record_id = Ulid::new().to_string();
      insert_query.push_str(" ('");
      delete_query.push('\'');
      insert_query.push_str(&record_id);
      delete_query.push_str(&record_id);
      insert_query.push_str("', '");
      insert_query.push_str(&user_id.to_string());
      insert_query.push_str("', '");
      insert_query.push_str(&record.meditation_minutes.to_string());
      insert_query.push_str("', '");
      insert_query.push_str(&record.meditation_seconds.to_string());
      insert_query.push_str("', '");
      insert_query.push_str(&guild_id.to_string());
      insert_query.push_str("', '");
      insert_query.push_str(&record.occurred_at.to_rfc3339());
      insert_query.push_str("')");
      insert_query.push_str(if i + 1 < user_data.len() { "," } else { ";" });
      delete_query.push_str(if i + 1 < user_data.len() {
        "', "
      } else {
        "');"
      });
    }

    Self {
      insert: insert_query,
      delete: delete_query,
    }
  }
}

fn time_overlaps(
  existing_entry: &Meditation,
  import_start: DateTime<Utc>,
  import_minutes: i32,
) -> bool {
  existing_entry.occurred_at.date_naive() == import_start.date_naive()
    && !(((existing_entry.occurred_at + TimeDelta::minutes(existing_entry.minutes.into()))
      < import_start)
      || ((import_start + TimeDelta::minutes(import_minutes.into())) < existing_entry.occurred_at))
}
