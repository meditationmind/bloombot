ALTER TABLE IF EXISTS quote
  ADD COLUMN IF NOT EXISTS quote_tsv tsvector
    GENERATED ALWAYS AS (to_tsvector('english', author || ' ' || quote)) STORED;

CREATE INDEX ON quote USING GIN (quote_tsv);
