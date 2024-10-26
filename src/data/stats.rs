pub struct Streak {
  pub current: i32,
  pub longest: i32,
}

pub struct UserStats {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: TimeframeStats,
  pub streak: Streak,
}

#[derive(Debug)]
pub struct LeaderboardUserStats {
  pub name: Option<String>,
  pub minutes: Option<i64>,
  pub sessions: Option<i64>,
  pub streak: Option<i32>,
  pub anonymous_tracking: Option<bool>,
  pub streaks_active: Option<bool>,
  pub streaks_private: Option<bool>,
}

pub struct GuildStats {
  pub all_minutes: i64,
  pub all_count: u64,
  pub timeframe_stats: TimeframeStats,
}

#[derive(Debug)]
pub struct TimeframeStats {
  pub sum: Option<i64>,
  pub count: Option<i64>,
}
