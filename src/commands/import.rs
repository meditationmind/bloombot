use crate::commands::{commit_and_say, MessageType};
use crate::config::{
  BloomBotEmbed, StreakRoles, TimeSumRoles, CHANNELS, EMOJI, MEDITATION_MIND, ROLES,
};
use crate::database::{DatabaseHandler, MeditationData, TrackingProfile};
use crate::Context;
use anyhow::{Context as AnyhowContext, Result};
use chrono::{TimeDelta, Utc};
use log::{error, info};
use poise::serenity_prelude::{
  self as serenity, builder::*, ChannelId, Mentionable, RoleId, UserId,
};
use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::io::AsyncWriteExt;
use ulid::Ulid;

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct WakingUpRecord {
  #[serde(rename = "Finished On")]
  date: String,
  #[serde(rename = "Title")]
  title: String,
  #[serde(rename = "Duration")]
  duration: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MindfulnessCoachRecord {
  #[serde(rename = "Date")]
  date: String,
  #[serde(rename = "Duration")]
  duration: String,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Deserialize)]
struct AppleHealthRecord {
  #[serde(rename = "App Name")]
  app_name: String,
  #[serde(rename = "Start Time")]
  occurred_at: chrono::DateTime<Utc>,
  #[serde(rename = "Minutes")]
  meditation_minutes: i32,
  #[serde(rename = "Seconds")]
  meditation_seconds: i32,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FinchTimerSession {
  data: Vec<FinchTimerSessionRecord>,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FinchBreathingSessionRecord {
  breathing_type: String,
  duration: i32,
  start_time: String,
  completed_time: String,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct FinchBreathingSession {
  data: Vec<FinchBreathingSessionRecord>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct BloomRecord {
  occurred_at: chrono::DateTime<Utc>,
  meditation_minutes: i32,
  meditation_seconds: i32,
}

#[derive(poise::ChoiceParameter)]
pub enum ImportSource {
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
  Unknown,
}

#[derive(poise::ChoiceParameter)]
pub enum ImportType {
  #[name = "new entries"]
  NewEntries,
  #[name = "all entries"]
  AllEntries,
}

fn autodetect_source(rdr: &mut csv::Reader<&[u8]>) -> Result<ImportSource> {
  let headers = rdr.headers()?;
  if headers == vec!["App Name", "Start Time", "Minutes", "Seconds"] {
    return Ok(ImportSource::AppleHealth);
  }
  if headers == vec!["Started At", "Duration", "Preset", "Activity"] {
    return Ok(ImportSource::InsightTimer);
  }
  if headers == vec!["Date", "Exercise", "Duration", "Comments"] {
    return Ok(ImportSource::MindfulnessCoach);
  }
  if headers == vec!["Finished On", "Title", "Duration"] {
    return Ok(ImportSource::WakingUp);
  }
  if headers
    == vec![
      "timerTypeIndex",
      "selectedDurationSeconds",
      "startTime",
      "completedTime",
    ]
  {
    return Ok(ImportSource::FinchMeditation);
  }
  if headers == vec!["breathing_type", "duration", "start_time", "completed_time"] {
    return Ok(ImportSource::FinchBreathing);
  }
  info!("Unrecognized headers: {:?}", headers);
  Ok(ImportSource::Unknown)
}

fn process_finch_timer(content: &Vec<u8>) -> Result<Vec<u8>> {
  let mut entries: Vec<FinchTimerSessionRecord> = vec![];
  let records: FinchTimerSession = serde_json::from_slice(content.as_slice())?;
  for record in records.data {
    entries.push(record);
  }
  let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
  for entry in entries {
    wtr.serialize(entry)?;
  }
  let csv = wtr.into_inner()?;

  Ok(csv)
}

fn process_finch_breathing(content: &Vec<u8>) -> Result<Vec<u8>> {
  let mut entries: Vec<FinchBreathingSessionRecord> = vec![];
  let records: FinchBreathingSession = serde_json::from_slice(content.as_slice())?;
  for record in records.data {
    entries.push(record);
  }
  let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
  for entry in entries {
    wtr.serialize(entry)?;
  }
  let csv = wtr.into_inner()?;

  Ok(csv)
}

async fn update_time_roles(
  ctx: Context<'_>,
  member: &serenity::Member,
  sum: i64,
  privacy: bool,
) -> Result<()> {
  let current_time_roles = TimeSumRoles::get_users_current_roles(&member.roles);
  let updated_time_role = TimeSumRoles::from_sum(sum);

  if let Some(updated_time_role) = updated_time_role {
    if !current_time_roles.contains(&updated_time_role.to_role_id()) {
      for role in current_time_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");
            ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_time_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");
          ctx.send(CreateReply::default()
            .content(format!("{} An error occured while updating your time roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true)).await?;

          return Ok(());
        }
      }

      let congrats = if ctx.guild_id().is_some() {
        format!(
          ":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the <@&{}> role!",
          member.mention(),
          updated_time_role.to_role_id()
        )
      } else {
        format!(
          ":tada: Congrats to {}, your hard work is paying off! Your total meditation minutes have given you the @{} role!",
          member.mention(),
          updated_time_role.to_role_icon()
        )
      };

      if privacy {
        ctx
          .send(
            CreateReply::default()
              .content(congrats)
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(privacy),
          )
          .await?;
      } else {
        ChannelId::new(CHANNELS.tracking)
          .send_message(
            &ctx,
            CreateMessage::new()
              .content(congrats)
              .allowed_mentions(serenity::CreateAllowedMentions::new()),
          )
          .await?;
      }
    }
  }

  Ok(())
}

async fn update_streak_roles(
  ctx: Context<'_>,
  member: &serenity::Member,
  streak: i32,
  privacy: bool,
) -> Result<()> {
  let current_streak_roles = StreakRoles::get_users_current_roles(&member.roles);
  #[allow(clippy::cast_sign_loss)]
  let updated_streak_role = StreakRoles::from_streak(streak as u64);

  if let Some(updated_streak_role) = updated_streak_role {
    if !current_streak_roles.contains(&updated_streak_role.to_role_id()) {
      for role in current_streak_roles {
        match member.remove_role(ctx, role).await {
          Ok(()) => {}
          Err(err) => {
            error!("Error removing role: {err}");

            ctx.send(CreateReply::default()
                .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
                .allowed_mentions(serenity::CreateAllowedMentions::new())
                .ephemeral(true)).await?;

            return Ok(());
          }
        }
      }

      match member.add_role(ctx, updated_streak_role.to_role_id()).await {
        Ok(()) => {}
        Err(err) => {
          error!("Error adding role: {err}");

          ctx.send(CreateReply::default()
              .content(format!("{} An error occured while updating your streak roles. Your entry has been saved, but your roles have not been updated. Please contact a moderator.", EMOJI.mminfo))
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(true)).await?;

          return Ok(());
        }
      }

      let congrats = if ctx.guild_id().is_some() {
        format!(
          ":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the <@&{}> role!",
          member.mention(),
          streak,
          updated_streak_role.to_role_id()
        )
      } else {
        format!(
          ":tada: Congrats to {}, your hard work is paying off! Your current streak is {}, giving you the @{} role!",
          member.mention(),
          streak,
          updated_streak_role.to_role_icon()
        )
      };

      if privacy {
        ctx
          .send(
            CreateReply::default()
              .content(congrats)
              .allowed_mentions(serenity::CreateAllowedMentions::new())
              .ephemeral(privacy),
          )
          .await?;
      } else {
        ChannelId::new(CHANNELS.tracking)
          .send_message(
            &ctx,
            CreateMessage::new()
              .content(congrats)
              .allowed_mentions(serenity::CreateAllowedMentions::new()),
          )
          .await?;
      }
    }
  }

  Ok(())
}

/// Import meditation entries from an app
///
/// Imports meditation entries from a CSV or JSON file uploaded by the user.
///
/// Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser).
#[poise::command(slash_command, category = "Meditation Tracking")]
pub async fn import(
  ctx: Context<'_>,
  #[description = "The message with the CSV/JSON file"] message: serenity::Message,
  #[description = "The type of import (Defaults to new entries)"]
  #[rename = "type"]
  import_type: Option<ImportType>,
  #[description = "The user to import for (staff only)"] user: Option<serenity::User>,
) -> Result<()> {
  ctx.defer_ephemeral().await?;

  if message.attachments.is_empty() {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} No attachment found.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  let data = ctx.data();
  let dm = ctx.guild_id().is_none();
  let guild_id = ctx.guild_id().unwrap_or(MEDITATION_MIND);

  let staff = match ctx.author_member().await {
    Some(member) => member.roles.contains(&RoleId::from(ROLES.staff)),
    None => false,
  };

  let user_id = match user {
    Some(user) => {
      if staff {
        user.id
      } else {
        message.author.id
      }
    }
    None => message.author.id,
  };

  if message.author.id != ctx.author().id && !staff {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} You cannot import files uploaded by other users.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  let attachment = message
    .attachments
    .first()
    .with_context(|| "Failed to assign attachment")?;

  if attachment.size > 262144 && !staff {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} File exceeds size limit. Please contact staff for assistance with importing large files.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  let mut transaction = data.db.start_transaction_with_retry(5).await?;

  let tracking_profile =
    match DatabaseHandler::get_tracking_profile(&mut transaction, &guild_id, &user_id).await? {
      Some(tracking_profile) => tracking_profile,
      None => TrackingProfile {
        ..Default::default()
      },
    };

  let privacy = tracking_profile.anonymous_tracking;

  let import_type = import_type.unwrap_or(ImportType::NewEntries);

  let latest_meditation = match import_type {
    ImportType::NewEntries => {
      DatabaseHandler::get_latest_meditation_entry(&mut transaction, &guild_id, &user_id).await?
    }
    ImportType::AllEntries => None,
  };

  let import_type = if latest_meditation.is_none() {
    ImportType::AllEntries
  } else {
    import_type
  };

  let content = match attachment.download().await {
    Ok(content) => {
      if attachment.filename == *"TimerSession.json" {
        process_finch_timer(&content).unwrap_or(content)
      } else if attachment.filename == *"BreathingSession.json" {
        process_finch_breathing(&content).unwrap_or(content)
      } else {
        content
      }
    }
    Err(why) => {
      info!("Error downloading attachment for import: {:?}", why);
      ctx
        .send(
          CreateReply::default()
            .content(format!("{} Unable to download attachment.", EMOJI.mminfo))
            .ephemeral(true),
        )
        .await?;

      return Ok(());
    }
  };

  let mut user_data: Vec<BloomRecord> = vec![];
  let mut total_minutes = 0;
  let mut total_seconds = 0;
  let mut import_source = String::new();
  let latest_meditation_time = match &latest_meditation {
    Some(entry) => entry.occurred_at,
    None => chrono::DateTime::UNIX_EPOCH,
  };
  let new_entries_only = match import_type {
    ImportType::AllEntries => false,
    ImportType::NewEntries => true,
  };
  let current_data = if new_entries_only {
    vec![latest_meditation.unwrap_or(MeditationData {
      id: String::new(),
      user_id: UserId::default(),
      meditation_minutes: 0,
      meditation_seconds: 0,
      occurred_at: chrono::DateTime::UNIX_EPOCH,
    })]
  } else {
    DatabaseHandler::get_user_meditation_entries(&mut transaction, &guild_id, &user_id).await?
  };

  let mut rdr = csv::ReaderBuilder::new().from_reader(content.as_slice());

  match autodetect_source(&mut rdr) {
    Ok(ImportSource::AppleHealth) => {
      let mut sources: Vec<String> = vec![];
      'result: for result in rdr.deserialize() {
        let row: AppleHealthRecord = result?;
        if !sources.contains(&row.app_name) {
          sources.push(row.app_name);
        }
        let datetime_utc = row.occurred_at;
        if new_entries_only && datetime_utc.le(&latest_meditation_time) {
          continue;
        }
        let minutes = row.meditation_minutes;
        for entry in &current_data {
          if entry.occurred_at.date_naive() == datetime_utc.date_naive()
            && !(((entry.occurred_at + TimeDelta::minutes(entry.meditation_minutes.into()))
              < datetime_utc)
              || ((datetime_utc + TimeDelta::minutes(minutes.into())) < entry.occurred_at))
          {
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
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::FinchBreathing) => {
      'result: for result in rdr.deserialize::<FinchBreathingSessionRecord>().flatten() {
        if !result.completed_time.is_empty() {
          if let Ok(valid_starttime) =
            chrono::NaiveDateTime::parse_from_str(&result.start_time, "%a, %d %b %Y %H:%M:%S")
          {
            let datetime_utc = valid_starttime.and_utc()
              - chrono::Duration::minutes(i64::from(tracking_profile.utc_offset));
            if new_entries_only && datetime_utc.le(&latest_meditation_time) {
              continue;
            }
            let minutes = result.duration / 60;
            if minutes < 1 {
              continue;
            }
            for entry in &current_data {
              if entry.occurred_at.date_naive() == datetime_utc.date_naive()
                && !(((entry.occurred_at + TimeDelta::minutes(entry.meditation_minutes.into()))
                  < datetime_utc)
                  || ((datetime_utc + TimeDelta::minutes(minutes.into())) < entry.occurred_at))
              {
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
      }
      import_source.push_str("Finch Breathing Sessions");
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::FinchMeditation) => {
      'result: for result in rdr.deserialize::<FinchTimerSessionRecord>().flatten() {
        if result.timer_type == 0 {
          if let Ok(valid_starttime) =
            chrono::NaiveDateTime::parse_from_str(&result.start_time, "%a, %d %b %Y %H:%M:%S")
          {
            let datetime_utc = valid_starttime.and_utc()
              - chrono::Duration::minutes(i64::from(tracking_profile.utc_offset));
            if new_entries_only && datetime_utc.le(&latest_meditation_time) {
              continue;
            }
            #[allow(clippy::cast_possible_truncation)]
            let (minutes, seconds) = if let Ok(valid_endtime) =
              chrono::NaiveDateTime::parse_from_str(&result.completed_time, "%a, %d %b %Y %H:%M:%S")
            {
              let num_seconds = (valid_endtime - valid_starttime).num_seconds() as i32;
              (num_seconds / 60, num_seconds % 60)
            } else {
              (result.selected_duration / 60, result.selected_duration % 60)
            };
            if minutes < 1 {
              continue;
            }
            for entry in &current_data {
              if entry.occurred_at.date_naive() == datetime_utc.date_naive()
                && !(((entry.occurred_at + TimeDelta::minutes(entry.meditation_minutes.into()))
                  < datetime_utc)
                  || ((datetime_utc + TimeDelta::minutes(minutes.into())) < entry.occurred_at))
              {
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
      }
      import_source.push_str("Finch Meditation Sessions");
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::InsightTimer) => {
      'result: for result in rdr.deserialize::<InsightTimerRecord>().flatten() {
        if result.activity == "PracticeType.Meditation"
        || result.activity == "Meditation" {
          if let Ok(valid_datetime) =
            chrono::NaiveDateTime::parse_from_str(&result.start_time, "%m/%d/%Y %H:%M:%S")
          {
            let datetime_utc = valid_datetime.and_utc();
            if new_entries_only && datetime_utc.le(&latest_meditation_time) {
              continue;
            }
            let (minutes, seconds) = {
              let h_m_s: Vec<&str> = result.duration.split(':').collect();
              let hours = <i32 as std::str::FromStr>::from_str(h_m_s[0])?;
              let minutes = <i32 as std::str::FromStr>::from_str(h_m_s[1])?;
              let seconds = <i32 as std::str::FromStr>::from_str(h_m_s[2])?;
              ((hours * 60) + minutes, seconds)
            };
            for entry in &current_data {
              if entry.occurred_at.date_naive() == datetime_utc.date_naive()
                && !(((entry.occurred_at + TimeDelta::minutes(entry.meditation_minutes.into()))
                  < datetime_utc)
                  || ((datetime_utc + TimeDelta::minutes(minutes.into())) < entry.occurred_at))
              {
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
      }
      import_source.push_str("Insight Timer");
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::MindfulnessCoach) => {
      for result in rdr.deserialize() {
        let row: MindfulnessCoachRecord = result?;
        if let Ok(valid_datetime) = chrono::NaiveDateTime::parse_from_str(
          format!("{} 00:00:00", &row.date).as_str(),
          "%Y-%m-%d %H:%M:%S",
        ) {
          let datetime_utc = valid_datetime.and_utc();
          if new_entries_only && datetime_utc.le(&latest_meditation_time) {
            continue;
          }
          if let Some(duration) = row.duration.split_whitespace().next() {
            let minutes = <i32 as std::str::FromStr>::from_str(duration)?;
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
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::WakingUp) => {
      for result in rdr.deserialize() {
        let row: WakingUpRecord = result?;
        if let Ok(valid_datetime) = chrono::NaiveDateTime::parse_from_str(
          format!("{} 00:00:00", &row.date).as_str(),
          "%m/%d/%Y %H:%M:%S",
        ) {
          let datetime_utc = valid_datetime.and_utc();
          if new_entries_only && datetime_utc.le(&latest_meditation_time) {
            continue;
          }
          let minutes = <i32 as std::str::FromStr>::from_str(&row.duration)? / 60;
          let seconds = <i32 as std::str::FromStr>::from_str(&row.duration)? % 60;
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
      if !dm {
        message.delete(ctx).await?;
      }
    }
    Ok(ImportSource::Unknown) => {
      ctx
        .send(
          CreateReply::default()
            .content(format!("{} **Unrecognized file format.**\n-# Please use an unaltered data export. Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser). If you would like support for another format, please contact staff.", EMOJI.mminfo))
            .ephemeral(true),
        )
        .await?;

      if message.author.id == ctx.author().id
        && message.channel_id == ChannelId::new(CHANNELS.tracking)
      {
        message.delete(ctx).await?;
      }

      return Ok(());
    }
    Err(e) => {
      info!("Failed to autodetect CSV source: {}", e);
      ctx
        .send(
          CreateReply::default()
            .content(format!("{} **Unrecognized file format.**\n-# Please use an unaltered data export. Supported sources include Insight Timer, VA Mindfulness Coach, Waking Up, Finch Breathing and Meditation Sessions, and Apple Health (requires pre-processing with Bloom Parser). If you would like support for another format, please contact staff.", EMOJI.mminfo))
            .ephemeral(true),
        )
        .await?;

      if message.author.id == ctx.author().id
        && message.channel_id == ChannelId::new(CHANNELS.tracking)
      {
        message.delete(ctx).await?;
      }

      return Ok(());
    }
  }

  drop(content);
  drop(current_data);

  if user_data.is_empty() {
    ctx
      .send(
        CreateReply::default()
          .content(format!("{} No qualifying entries found.", EMOJI.mminfo))
          .ephemeral(true),
      )
      .await?;

    return Ok(());
  }

  let mut sql_query =
    "INSERT INTO meditation (record_id, user_id, meditation_minutes, meditation_seconds, guild_id, occurred_at) VALUES"
      .to_owned();
  let mut reversal_query = "DELETE FROM meditation WHERE record_id IN (".to_owned();
  for (i, record) in user_data.iter().enumerate() {
    let record_id = Ulid::new().to_string();
    sql_query.push_str(" ('");
    reversal_query.push('\'');
    sql_query.push_str(&record_id);
    reversal_query.push_str(&record_id);
    sql_query.push_str("', '");
    sql_query.push_str(&user_id.to_string());
    sql_query.push_str("', '");
    sql_query.push_str(&record.meditation_minutes.to_string());
    sql_query.push_str("', '");
    sql_query.push_str(&record.meditation_seconds.to_string());
    sql_query.push_str("', '");
    sql_query.push_str(&guild_id.to_string());
    sql_query.push_str("', '");
    sql_query.push_str(&record.occurred_at.to_rfc3339());
    sql_query.push_str("')");
    sql_query.push_str(if i + 1 < user_data.len() { "," } else { ";" });
    reversal_query.push_str(if i + 1 < user_data.len() {
      "', "
    } else {
      "');"
    });
  }

  drop(user_data);

  let result = DatabaseHandler::add_meditation_entry_batch(&mut transaction, &sql_query).await?;
  if result < 1 {
    ctx
      .send(
        CreateReply::default()
          .content(format!(
            "{} No entries added. Please try again or contact staff for assistance.",
            EMOJI.mminfo
          ))
          .ephemeral(true),
      )
      .await?;
  }

  let random_quote = DatabaseHandler::get_random_quote(&mut transaction, &guild_id).await?;
  let user_sum =
    DatabaseHandler::get_user_meditation_sum(&mut transaction, &guild_id, &user_id).await?;

  let response = match random_quote {
    Some(quote) => {
      // Strip non-alphanumeric characters from the quote
      let quote = quote
        .quote
        .chars()
        //.filter(|c| c.is_alphanumeric() || c.is_whitespace() || c.is_ascii_punctuation() || matches!(c, '’' | '‘' | '“' | '”' | '—' | '…' | 'ā'))
        .filter(|c| !matches!(c, '*'))
        .map(|c| {
          if c.is_ascii_punctuation() {
            if matches!(c, '_' | '~') {
              c.to_string()
            } else {
              format!("\\{c}")
            }
          } else {
            c.to_string()
          }
        })
        .collect::<String>();

      if privacy {
        format!(
          "Someone just added **{total_minutes} minutes** to their meditation time! :tada:\n*{quote}*"
        )
      } else {
        format!("<@{user_id}> added **{total_minutes} minutes** to their meditation time! Their total meditation time is now {user_sum} minutes :tada:\n*{quote}*")
      }
    }
    None => {
      if privacy {
        format!("Someone just added **{total_minutes} minutes** to their meditation time! :tada:")
      } else {
        format!("<@{user_id}> added **{total_minutes} minutes** to their meditation time! Their total meditation time is now {user_sum} minutes :tada:")
      }
    }
  };

  let user_streak = if tracking_profile.streaks_active {
    let streak = DatabaseHandler::get_streak(&mut transaction, &guild_id, &user_id).await?;
    streak.current
  } else {
    0
  };

  let guild_time_in_hours = {
    let guild_count =
      DatabaseHandler::get_guild_meditation_count(&mut transaction, &guild_id).await?;
    if guild_count % 10 == 0 {
      let guild_sum =
        DatabaseHandler::get_guild_meditation_sum(&mut transaction, &guild_id).await?;
      (guild_sum / 60).to_string()
    } else {
      "skip".to_owned()
    }
  };

  let h = (total_minutes + (total_seconds / 60)) / 60;
  let m = (total_minutes + (total_seconds / 60)) % 60;
  let s = total_seconds % 60;

  let success_response = format!(
    "{} Successfully added a total of {}h {}m {}s from {} {} imported from {}.",
    EMOJI.mmcheck,
    h,
    m,
    s,
    result,
    if result == 1 { "entry" } else { "entries" },
    import_source,
  );
  commit_and_say(
    ctx,
    transaction,
    MessageType::TextOnly(success_response),
    true,
  )
  .await?;

  ChannelId::new(CHANNELS.tracking)
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(response)
        .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;

  if guild_time_in_hours != "skip" {
    ChannelId::new(CHANNELS.tracking)
    .send_message(
      &ctx,
      CreateMessage::new()
        .content(format!("Awesome sauce! This server has collectively generated {guild_time_in_hours} hours of realmbreaking meditation!"))
        .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;
  }

  let member = guild_id.member(ctx, user_id).await?;
  update_time_roles(ctx, &member, user_sum, privacy).await?;
  if tracking_profile.streaks_active {
    update_streak_roles(ctx, &member, user_streak, privacy).await?;
  }

  let filename = format!("import_{}_{}.txt", user_id, Ulid::new().to_string());
  let mut file = tokio::fs::File::create(&filename).await?;
  file.write_all(reversal_query.as_bytes()).await?;
  file.flush().await?;
  let f1 = tokio::fs::File::open(&filename).await?;
  let return_file = [CreateAttachment::file(&f1, &filename).await?];

  let log_embed = BloomBotEmbed::new()
    .title("Meditation Tracking Data Import")
    .description(format!(
      "**User**: <@{user_id}>\n**Entries Added**: {result}\n**Total Time**: {total_minutes} minutes\n**Source**: {import_source}"
    ))
    .footer(
      CreateEmbedFooter::new(format!(
        "Added by {} ({})",
        ctx.author().name,
        ctx.author().id
      ))
      .icon_url(ctx.author().avatar_url().unwrap_or_default()),
    )
    .clone();
  let log_channel = serenity::ChannelId::new(CHANNELS.bloomlogs);
  log_channel
    .send_files(&ctx, return_file, CreateMessage::new().embed(log_embed))
    .await?;

  if let Err(e) = tokio::fs::remove_file(filename).await {
    return Err(anyhow::anyhow!("Error removing file: {:?}", e));
  }

  Ok(())
}
