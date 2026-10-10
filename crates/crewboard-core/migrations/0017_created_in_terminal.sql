-- T-0167 (TKT-08, RUN-06): a ticket created from a Project terminal remembers that session (by caller identity),
-- and the terminal's summary of created tickets remembers when the user last dismissed it.
ALTER TABLE tickets ADD COLUMN created_in_session TEXT;
ALTER TABLE sessions ADD COLUMN summary_dismissed_at BIGINT;
