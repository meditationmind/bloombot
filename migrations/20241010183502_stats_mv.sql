CREATE MATERIALIZED VIEW IF NOT EXISTS yearly_data AS
	SELECT
		floor(
			extract(epoch from ((date_trunc('year', now()) + interval '1 year') - interval '1 second') - occurred_at) /
			extract(epoch from (((date_trunc('year', occurred_at) + interval '1 year') - interval '1 second') - (date_trunc('year', occurred_at))))
		)::float AS times_ago,
		meditation_minutes,
		meditation_seconds,
		guild_id,
		user_id
	FROM meditation;

CREATE MATERIALIZED VIEW IF NOT EXISTS monthly_data AS
	SELECT
		floor(
			extract(epoch from ((date_trunc('month', now()) + interval '1 month') - interval '1 second') - occurred_at) /
			extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at))))
		)::float AS times_ago,
		meditation_minutes,
		meditation_seconds,
		guild_id,
		user_id
	FROM meditation;

CREATE MATERIALIZED VIEW IF NOT EXISTS weekly_data AS
	SELECT
		floor(
			extract(epoch from ((date_trunc('week', now()) + interval '1 week') - interval '1 second') - occurred_at) /
			extract(epoch from (((date_trunc('month', occurred_at) + interval '1 month') - interval '1 second') - (date_trunc('month', occurred_at))))
		)::float AS times_ago,
		meditation_minutes,
		meditation_seconds,
		guild_id,
		user_id
	FROM meditation;
