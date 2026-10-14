-- Reopen (CLS-03, GIT-06; d0044, Q5): a Closed ticket reopened to Backlog that has been to Dev. Its next start reuses
-- its old branch, first rebased onto the current base (the checkpoint rule), then starts a NEW agent session (not a
-- resume). Cleared once that start has run.
ALTER TABLE tickets ADD COLUMN reopened BOOLEAN NOT NULL DEFAULT FALSE;
