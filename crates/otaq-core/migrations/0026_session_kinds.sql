-- no-transaction
-- T-0183 (VAL-01; d0063, d0068): sessions.kind gains 'validator' (a ticket's validator) and 'project_agent' (a
-- Project agent, used from T-0199). SQLite can't change a CHECK constraint, so the table is rebuilt the
-- documented way: foreign keys off (outside a transaction, so dropping the old table cascades nothing into usage
-- and pending_deliveries), copy, swap, indexes back, foreign keys on.
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE sessions_new (
    id                   TEXT   NOT NULL PRIMARY KEY,
    project_id           TEXT   NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    ticket_id            TEXT   REFERENCES tickets (id) ON DELETE CASCADE,  -- NULL for Project terminals and agents
    kind                 TEXT   NOT NULL CHECK (kind IN ('agent', 'answer', 'delivery', 'shell', 'project', 'run',
                                                         'validator', 'project_agent')),
    pty_id               TEXT,
    claude_session_id    TEXT,
    state                TEXT   NOT NULL,
    started_at           BIGINT NOT NULL,
    ended_at             BIGINT,
    end_reason           TEXT,
    summary_dismissed_at BIGINT
);
INSERT INTO sessions_new (id, project_id, ticket_id, kind, pty_id, claude_session_id, state, started_at, ended_at,
                          end_reason, summary_dismissed_at)
SELECT id, project_id, ticket_id, kind, pty_id, claude_session_id, state, started_at, ended_at, end_reason,
       summary_dismissed_at
FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;
CREATE INDEX sessions_ticket ON sessions (ticket_id, started_at);
CREATE INDEX sessions_project ON sessions (project_id);
CREATE INDEX sessions_claude_session ON sessions (claude_session_id);
COMMIT;
PRAGMA foreign_keys = ON;
