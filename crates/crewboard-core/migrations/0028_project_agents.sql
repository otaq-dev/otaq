-- T-0199 (PAG-01, PAG-04, PAG-06; d0068): Project agents. One row per Project agent the user opened: its name and
-- where the name came from (NULL while it is "New agent"), the Claude conversation it continues on Resume, its state,
-- the first request it got (NULL: it never got one, so closing or a restart discards it), whether the plan limit
-- stopped it, and when the user closed its tab. Its sessions point at it.
CREATE TABLE project_agents (
    id                TEXT    NOT NULL PRIMARY KEY,
    project_id        TEXT    NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name              TEXT,
    name_source       TEXT    CHECK (name_source IN ('agent', 'fallback', 'user')),
    claude_session_id TEXT    NOT NULL,
    state             TEXT    NOT NULL CHECK (state IN ('running', 'ended', 'ended_by_restart')),
    first_request     TEXT,
    limit_stopped     INTEGER NOT NULL DEFAULT 0,
    created_at        BIGINT  NOT NULL,
    ended_at          BIGINT,
    closed_at         BIGINT
);
CREATE INDEX project_agents_project ON project_agents (project_id, created_at);
ALTER TABLE sessions ADD COLUMN project_agent_id TEXT REFERENCES project_agents (id) ON DELETE SET NULL;
