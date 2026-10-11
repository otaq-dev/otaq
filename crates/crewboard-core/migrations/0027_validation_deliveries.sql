-- T-0184 (VAL-04, VAL-08; d0063, d0048): a new kind of automatic delivery, the validator's findings (or
-- "Validation was cancelled…") for the agent. SQLite can't change a CHECK constraint, so the table is rebuilt with
-- the same columns (nothing references it).
CREATE TABLE pending_deliveries_new (
    id           TEXT   NOT NULL PRIMARY KEY,
    session_id   TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    kind         TEXT   NOT NULL CHECK (kind IN ('agent_message', 'ticket_change', 'continue_after_limit', 'skill',
                                                 'validation')),
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
