-- T-0182 (VAL-01, VAL-03, VAL-06, VAL-07, AMG-02…AMG-04; d0063, d0064): validation rounds and Auto-merge.
-- One row per validation round: who started it, the validator's session, how it ended and its report (findings
-- and notes). Only pass and fail count as rounds (tickets.validation_round).
CREATE TABLE validations (
    id         TEXT   NOT NULL PRIMARY KEY,
    ticket_id  TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    round      BIGINT NOT NULL,
    trigger    TEXT   NOT NULL,                 -- agent / user
    session_id TEXT,                            -- the validator's session (sessions.id)
    started_at BIGINT NOT NULL,
    ended_at   BIGINT,
    outcome    TEXT,                            -- pass / fail / cancelled / no_result / discarded / interrupted / skipped / stopped
    report     TEXT,
    notes_json TEXT,
    head_sha   TEXT                             -- the worktree's head when the validator started (VAL-02)
);
CREATE INDEX validations_ticket ON validations (ticket_id, started_at);

-- How the ticket reached Review: validated / skipped / submitted_by_user / failed_moved (NULL before M12).
ALTER TABLE tickets ADD COLUMN review_entry TEXT;
-- Rounds counted since the last reset (Send back, Answer the agent, an Auto-merge conflict).
ALTER TABLE tickets ADD COLUMN validation_round BIGINT NOT NULL DEFAULT 0;
-- Extra rounds the user allowed with "Give it another round" (implementation detail, d0063).
ALTER TABLE tickets ADD COLUMN validation_rounds_extra BIGINT NOT NULL DEFAULT 0;
-- Auto-merge conflicts in a row; a successful merge resets it (AMG-03).
ALTER TABLE tickets ADD COLUMN auto_merge_conflicts BIGINT NOT NULL DEFAULT 0;
-- Why Auto-merge stopped: keeps_conflicting / base_diverged / checkout_dirty (AMG-03, AMG-04).
ALTER TABLE tickets ADD COLUMN auto_merge_stop TEXT;
ALTER TABLE tickets ADD COLUMN merged_automatically BOOLEAN NOT NULL DEFAULT FALSE;
