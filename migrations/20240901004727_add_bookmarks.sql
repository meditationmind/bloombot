CREATE TABLE IF NOT EXISTS bookmarks (
  record_id          TEXT PRIMARY KEY,
  user_id            TEXT NOT NULL,
  guild_id           TEXT NOT NULL,
  message_link       TEXT NOT NULL,
  user_desc          TEXT,
  occurred_at        TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL
);