-- T-0285 (LEAD-07, LEAD-04; d0083, d0063, d0064, d0042): a stuck epic child goes to its live lead once. The row is
-- the child's one lead attempt: it exists from the child's first stop that woke the lead, and never again for that
-- child (the primary key). While `state` is `held` the stop waits for the lead (no inbox item); `noted` once the
-- lead's note answered it (`note_id`); `handed_back` once it went to the user without one (the lead's turn ended, the
-- lead ended, or the service restarted). `detail` is what the stop needs to be shown later: the last validation
-- report, the merge target, or the agent's last line. Deleting the child or the epic deletes the row.
CREATE TABLE lead_stuck (
    child_id    TEXT    NOT NULL PRIMARY KEY REFERENCES tickets (id) ON DELETE CASCADE,
    epic_id     TEXT    NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    -- The lead's Project agent row (no reference: the row outlives a lead row).
    lead_id     TEXT    NOT NULL,
    stop        TEXT    NOT NULL CHECK (stop IN ('validation_failed', 'keeps_conflicting', 'idle')),
    detail      TEXT,
    -- Validation failed: the rounds it failed (the setting plus the extra rounds allowed).
    rounds      INTEGER,
    state       TEXT    NOT NULL CHECK (state IN ('held', 'noted', 'handed_back')),
    held_at     BIGINT  NOT NULL,
    resolved_at BIGINT,
    note_id     TEXT    REFERENCES lead_notes (id) ON DELETE SET NULL,
    CHECK ((state = 'held') = (resolved_at IS NULL))
);

CREATE INDEX lead_stuck_by_epic ON lead_stuck (epic_id);
