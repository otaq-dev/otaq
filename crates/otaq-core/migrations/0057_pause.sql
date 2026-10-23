-- no-transaction
-- M38 Pause (T-0455; PAU-01, PAU-03). Decisions 0112, 0079. Never edit once committed.
--
-- A project is paused while `paused_since` is set (the time the user paused it, Unix ms); `NULL` means it runs. The
-- service stores it so quitting the app, restarting the service or rebooting keeps the pause (d0112: a pause never
-- lifts on its own). Only the user's Resume clears it; a project added later starts unpaused.
--
-- A Project agent or an epic lead the pause ended is `paused` ("Paused · Resume", the same conversation, like
-- `ended_by_restart`). SQLite can't change the state's CHECK in place, so `project_agents` is rebuilt the documented
-- way, as 0038 did: foreign keys off (outside a transaction, so dropping the old table cascades nothing into
-- sessions, messages and human_questions), copy, swap, indexes back, foreign keys on.
PRAGMA foreign_keys = OFF;
BEGIN;
ALTER TABLE projects ADD COLUMN paused_since BIGINT;

CREATE TABLE project_agents_new (
    id              TEXT    NOT NULL PRIMARY KEY,
    project_id      TEXT    NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name            TEXT,
    name_source     TEXT    CHECK (name_source IN ('agent', 'fallback', 'user')),
    tool            TEXT    NOT NULL,
    tool_session_id TEXT,
    model           TEXT,
    state           TEXT    NOT NULL CHECK (state IN ('running', 'ended', 'ended_by_restart', 'paused')),
    first_request   TEXT,
    limit_stopped   INTEGER NOT NULL DEFAULT 0,
    created_at      BIGINT  NOT NULL,
    ended_at        BIGINT,
    closed_at       BIGINT,
    epic_id         TEXT    REFERENCES tickets (id) ON DELETE CASCADE,
    agent_id        TEXT,
    agent_name      TEXT,
    model_ran       TEXT,
    effort          TEXT
);
INSERT INTO project_agents_new (id, project_id, name, name_source, tool, tool_session_id, model, state,
                                first_request, limit_stopped, created_at, ended_at, closed_at, epic_id, agent_id,
                                agent_name, model_ran, effort)
SELECT id, project_id, name, name_source, tool, tool_session_id, model, state, first_request, limit_stopped,
       created_at, ended_at, closed_at, epic_id, agent_id, agent_name, model_ran, effort
FROM project_agents;
DROP TABLE project_agents;
ALTER TABLE project_agents_new RENAME TO project_agents;
CREATE INDEX project_agents_project ON project_agents (project_id, created_at);
CREATE UNIQUE INDEX project_agents_epic ON project_agents (epic_id) WHERE epic_id IS NOT NULL;
COMMIT;
PRAGMA foreign_keys = ON;
