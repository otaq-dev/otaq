-- no-transaction
-- T-0309 (ADP-04, SES-01; d0088): sessions are tool-neutral. `claude_session_id` becomes `tool_session_id` (the
-- tool's own conversation id), next to the agent tool the session ran on (`tool`: NULL for Shell, Run and Project
-- terminals, which run no agent tool) and the model passed at launch (`model`: NULL when none was). Every session
-- before this ran Claude Code. `project_agents` gets the same three columns; its tool id may now be empty for a
-- moment (a tool that reports its id after start), which SQLite can't change in place, so the table is rebuilt the
-- documented way, as 0026 did: foreign keys off (outside a transaction, so dropping the old table cascades nothing
-- into sessions, messages and human_questions), copy, swap, indexes back, foreign keys on.
PRAGMA foreign_keys = OFF;
BEGIN;
ALTER TABLE sessions RENAME COLUMN claude_session_id TO tool_session_id;
ALTER TABLE sessions ADD COLUMN tool TEXT;
ALTER TABLE sessions ADD COLUMN model TEXT;
UPDATE sessions SET tool = 'claude'
WHERE kind IN ('agent', 'answer', 'delivery', 'validator', 'project_agent');
DROP INDEX sessions_claude_session;
CREATE INDEX sessions_tool_session ON sessions (tool_session_id);

CREATE TABLE project_agents_new (
    id              TEXT    NOT NULL PRIMARY KEY,
    project_id      TEXT    NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name            TEXT,
    name_source     TEXT    CHECK (name_source IN ('agent', 'fallback', 'user')),
    tool            TEXT    NOT NULL,
    tool_session_id TEXT,
    model           TEXT,
    state           TEXT    NOT NULL CHECK (state IN ('running', 'ended', 'ended_by_restart')),
    first_request   TEXT,
    limit_stopped   INTEGER NOT NULL DEFAULT 0,
    created_at      BIGINT  NOT NULL,
    ended_at        BIGINT,
    closed_at       BIGINT,
    epic_id         TEXT    REFERENCES tickets (id) ON DELETE CASCADE
);
INSERT INTO project_agents_new (id, project_id, name, name_source, tool, tool_session_id, model, state,
                                first_request, limit_stopped, created_at, ended_at, closed_at, epic_id)
SELECT id, project_id, name, name_source, 'claude', claude_session_id, NULL, state, first_request, limit_stopped,
       created_at, ended_at, closed_at, epic_id
FROM project_agents;
DROP TABLE project_agents;
ALTER TABLE project_agents_new RENAME TO project_agents;
CREATE INDEX project_agents_project ON project_agents (project_id, created_at);
CREATE UNIQUE INDEX project_agents_epic ON project_agents (epic_id) WHERE epic_id IS NOT NULL;
COMMIT;
PRAGMA foreign_keys = ON;
