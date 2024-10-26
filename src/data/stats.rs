pub struct Streak {
  pub current: i32,
  pub longest: i32,
}

pub struct User {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: Timeframe,
  pub streak: Streak,
}

#[derive(Debug)]
pub struct LeaderboardUser {
  pub name: Option<String>,
  pub minutes: Option<i64>,
  pub sessions: Option<i64>,
  pub streak: Option<i32>,
  pub anonymous_tracking: Option<bool>,
  pub streaks_active: Option<bool>,
  pub streaks_private: Option<bool>,
}

pub struct Guild {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: Timeframe,
}

#[derive(Debug)]
pub struct Timeframe {
  pub sum: Option<i64>,
  pub count: Option<i64>,
}
