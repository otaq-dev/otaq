-- Send back (REV-04; d0034, d0048).
-- sent_back_at: when the latest Send back returned the ticket to Dev; the card shows "Sent back · N comments ·
--               round K" (K = sent_back_round) until the ticket next reaches Ready for Review, which clears it.
-- resume_skill: the skill a ticket returning to Dev runs once its agent is live again (address-review after Send
--               back, restack after Send to agent to fix), when the click couldn't type it at once: the session
--               was gone (resumed with the skill) or the agent limit was full (Queued until a slot frees).
ALTER TABLE tickets ADD COLUMN sent_back_at BIGINT;
ALTER TABLE tickets ADD COLUMN resume_skill TEXT;

-- A new kind of automatic delivery: a skill a click asked for while the agent limit was full, typed once the
-- input line is empty. SQLite can't change a CHECK constraint, so the table is rebuilt with the same columns.
CREATE TABLE pending_deliveries_new (
    id         TEXT   NOT NULL PRIMARY KEY,
    session_id TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    kind       TEXT   NOT NULL CHECK (kind IN ('agent_message', 'ticket_change', 'continue_after_limit', 'skill')),
    text       TEXT   NOT NULL,
    created_at BIGINT NOT NULL
);
INSERT INTO pending_deliveries_new (id, session_id, kind, text, created_at)
    SELECT id, session_id, kind, text, created_at FROM pending_deliveries;
DROP TABLE pending_deliveries;
ALTER TABLE pending_deliveries_new RENAME TO pending_deliveries;
CREATE INDEX pending_deliveries_session ON pending_deliveries (session_id, created_at);
