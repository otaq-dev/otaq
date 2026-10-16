-- T-0281 (LEAD-10, CLI-08; d0083, d0048): an epic lead's notes to its running children (`otaq note <KEY> --stdin`).
-- A note is unread until the child's agent runs `otaq notes` or `otaq prime`, or the service types it in when the
-- child goes idle; `read_how` says which. `with_stop` and `view` are for a note to a stuck child (T-0285): it goes
-- with the stop's own action, and the lead's view of the cause travels with it. Deleting the child or the epic
-- deletes its notes.
CREATE TABLE lead_notes (
    id         TEXT   NOT NULL PRIMARY KEY,
    epic_id    TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    child_id   TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    -- The lead's Project agent row (no reference: the note outlives a lead row).
    lead_id    TEXT   NOT NULL,
    written_by TEXT   NOT NULL,
    text       TEXT   NOT NULL,
    view       TEXT,
    created_at BIGINT NOT NULL,
    read_at    BIGINT,
    read_how   TEXT   CHECK (read_how IN ('notes', 'prime', 'typed', 'with_stop')),
    CHECK ((read_at IS NULL) = (read_how IS NULL))
);

CREATE INDEX lead_notes_by_child ON lead_notes (child_id, created_at);
CREATE INDEX lead_notes_by_epic ON lead_notes (epic_id);

-- The note typed in at idle is an automatic delivery of its own kind. SQLite can't change a CHECK constraint, so the
-- table is rebuilt with the same columns (nothing references it), as in 0033.
CREATE TABLE pending_deliveries_new (
    id           TEXT   NOT NULL PRIMARY KEY,
    session_id   TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    kind         TEXT   NOT NULL CHECK (kind IN ('agent_message', 'ticket_change', 'continue_after_limit', 'skill',
                                                 'validation', 'lead_wake', 'lead_note')),
    text         TEXT   NOT NULL,
    created_at   BIGINT NOT NULL,
    message_id   TEXT,
    message_part TEXT
);
INSERT INTO pending_deliveries_new (id, session_id, kind, text, created_at, message_id, message_part)
    SELECT id, session_id, kind, text, created_at, message_id, message_part FROM pending_deliveries;
DROP TABLE pending_deliveries;
ALTER TABLE pending_deliveries_new RENAME TO pending_deliveries;
CREATE INDEX pending_deliveries_session ON pending_deliveries (session_id, created_at);
