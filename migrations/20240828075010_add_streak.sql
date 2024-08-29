-- Add migration script here
CREATE TABLE IF NOT EXISTS streak (
  record_id          TEXT PRIMARY KEY,
  user_id            TEXT NOT NULL,
  guild_id           TEXT NOT NULL,
  current_streak     INTEGER DEFAULT 0 NOT NULL,
  longest_streak     INTEGER DEFAULT 0 NOT NULL
);