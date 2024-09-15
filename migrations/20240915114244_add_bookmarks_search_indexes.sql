ALTER TABLE IF EXISTS bookmarks
  ADD COLUMN IF NOT EXISTS desc_tsv tsvector
    GENERATED ALWAYS AS (to_tsvector('english', user_desc)) STORED;

CREATE INDEX ON bookmarks USING GIN (desc_tsv);
