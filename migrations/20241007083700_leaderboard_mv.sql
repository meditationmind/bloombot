CREATE MATERIALIZED VIEW IF NOT EXISTS yearly_leaderboard AS
    SELECT
        COUNT(m.record_id) AS sessions,
        (SUM(m.meditation_minutes) + (SUM(m.meditation_seconds) / 60)) AS minutes,
        m.user_id AS name,
        m.guild_id AS guild,
        s.current_streak AS streak,
        t.anonymous_tracking AS anonymous_tracking,
        t.streaks_active AS streaks_active,
        t.streaks_private AS streaks_private
    FROM meditation m
    LEFT JOIN streak s ON m.user_id = s.user_id
    LEFT JOIN tracking_profile t ON m.user_id = t.user_id
    WHERE m.occurred_at >= date_trunc('year', now()) AND m.occurred_at <= now()
    GROUP BY name, guild, streak, anonymous_tracking, streaks_active, streaks_private;

CREATE MATERIALIZED VIEW IF NOT EXISTS monthly_leaderboard AS
    SELECT
        COUNT(m.record_id) AS sessions,
        (SUM(m.meditation_minutes) + (SUM(m.meditation_seconds) / 60)) AS minutes,
        m.user_id AS name,
        m.guild_id AS guild,
        s.current_streak AS streak,
        t.anonymous_tracking AS anonymous_tracking,
        t.streaks_active AS streaks_active,
        t.streaks_private AS streaks_private
    FROM meditation m
    LEFT JOIN streak s ON m.user_id = s.user_id
    LEFT JOIN tracking_profile t ON m.user_id = t.user_id
    WHERE m.occurred_at >= date_trunc('month', now()) AND m.occurred_at <= now()
    GROUP BY name, guild, streak, anonymous_tracking, streaks_active, streaks_private;

CREATE MATERIALIZED VIEW IF NOT EXISTS weekly_leaderboard AS
    SELECT
        COUNT(m.record_id) AS sessions,
        (SUM(m.meditation_minutes) + (SUM(m.meditation_seconds) / 60)) AS minutes,
        m.user_id AS name,
        m.guild_id AS guild,
        s.current_streak AS streak,
        t.anonymous_tracking AS anonymous_tracking,
        t.streaks_active AS streaks_active,
        t.streaks_private AS streaks_private
    FROM meditation m
    LEFT JOIN streak s ON m.user_id = s.user_id
    LEFT JOIN tracking_profile t ON m.user_id = t.user_id
    WHERE m.occurred_at >= date_trunc('week', now()) AND m.occurred_at <= now()
    GROUP BY name, guild, streak, anonymous_tracking, streaks_active, streaks_private;

CREATE MATERIALIZED VIEW IF NOT EXISTS daily_leaderboard AS
    SELECT
        COUNT(m.record_id) AS sessions,
        (SUM(m.meditation_minutes) + (SUM(m.meditation_seconds) / 60)) AS minutes,
        m.user_id AS name,
        m.guild_id AS guild,
        s.current_streak AS streak,
        t.anonymous_tracking AS anonymous_tracking,
        t.streaks_active AS streaks_active,
        t.streaks_private AS streaks_private
    FROM meditation m
    LEFT JOIN streak s ON m.user_id = s.user_id
    LEFT JOIN tracking_profile t ON m.user_id = t.user_id
    WHERE m.occurred_at >= date_trunc('day', now()) AND m.occurred_at <= now()
    GROUP BY name, guild, streak, anonymous_tracking, streaks_active, streaks_private;

CREATE UNIQUE INDEX ON yearly_leaderboard (name);
CREATE UNIQUE INDEX ON monthly_leaderboard (name);
CREATE UNIQUE INDEX ON weekly_leaderboard (name);
CREATE UNIQUE INDEX ON daily_leaderboard (name);

CREATE INDEX ON yearly_leaderboard (minutes);
CREATE INDEX ON yearly_leaderboard (sessions);
CREATE INDEX ON yearly_leaderboard (streak);

CREATE INDEX ON monthly_leaderboard (minutes);
CREATE INDEX ON monthly_leaderboard (sessions);
CREATE INDEX ON monthly_leaderboard (streak);

CREATE INDEX ON weekly_leaderboard (minutes);
CREATE INDEX ON weekly_leaderboard (sessions);
CREATE INDEX ON weekly_leaderboard (streak);

CREATE INDEX ON daily_leaderboard (minutes);
CREATE INDEX ON daily_leaderboard (sessions);
CREATE INDEX ON daily_leaderboard (streak);
