DROP INDEX IF EXISTS term_aliases_idx1;

CREATE OR REPLACE FUNCTION f_textarr2text(text[]) 
  RETURNS text LANGUAGE sql IMMUTABLE AS $$SELECT array_to_string($1, ',')$$;

CREATE INDEX ON term USING GIN (f_textarr2text(aliases) gin_trgm_ops);