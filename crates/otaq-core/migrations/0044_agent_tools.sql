-- M23 More agent tools (T-0347; AGT-08, SCH-04, §7.2). Decisions 0086, 0088. Never edit once committed.
-- No rows are copied (0088: no backward compatibility). Hidden tools and "Use fallback agents automatically" are
-- `settings` keys (`agent_tools.hidden`, `agents.auto_fallback`); the named agents' `fallback_id`, the mark and the
-- sessions' `model` / `model_ran` exist since M22.

-- A Developer row moved to another named agent because its tool was at its limit or not available, by the fallback
-- or by Move waiting to… (`tool_limit`, `tool_unavailable`), or moved by the user (`user_move`): where it came from
-- (the agent's id and its name then), why, the tool that was at its limit or unavailable, and when. All NULL = not
-- moved. A later change of the row clears them.
ALTER TABLE ticket_agent_rows ADD COLUMN moved_from_agent_id TEXT;
ALTER TABLE ticket_agent_rows ADD COLUMN moved_from_name TEXT;
ALTER TABLE ticket_agent_rows ADD COLUMN moved_reason TEXT
    CHECK (moved_reason IN ('tool_limit', 'tool_unavailable', 'user_move'));
ALTER TABLE ticket_agent_rows ADD COLUMN moved_tool TEXT;
ALTER TABLE ticket_agent_rows ADD COLUMN moved_at BIGINT;

-- An agent that would start but is held (SCH-04 per tool, §7.2): a Developer start, a Validator round, an epic lead,
-- or a Project agent's first message, because its tool is at its limit, not available, or its model is no longer
-- in the tool's list. One row per held role of a ticket (`ticket_id`), or of a project for its Project agent
-- (`project_id`, `ticket_id` NULL). `tool`, `model` and `named_agent_id` are what it would have started on;
-- `fallback_agent_id` the fallback that could take it (NULL when there is none or it is held too), and
-- `fallback_held` whether the fallback is held as well ("both at their limits"). The row goes when the hold lifts.
CREATE TABLE agent_holds (
    id                TEXT    NOT NULL PRIMARY KEY,
    ticket_id         TEXT    REFERENCES tickets (id) ON DELETE CASCADE,
    project_id        TEXT    REFERENCES projects (id) ON DELETE CASCADE,
    role              TEXT    NOT NULL CHECK (role IN ('developer', 'validator', 'lead', 'project_agent')),
    reason            TEXT    NOT NULL CHECK (reason IN ('tool_limit', 'tool_unavailable', 'model_unavailable')),
    tool              TEXT    NOT NULL,
    model             TEXT,
    named_agent_id    TEXT    REFERENCES named_agents (id) ON DELETE SET NULL,
    fallback_agent_id TEXT    REFERENCES named_agents (id) ON DELETE SET NULL,
    fallback_held     BOOLEAN NOT NULL DEFAULT FALSE,
    since             BIGINT  NOT NULL,
    CHECK ((ticket_id IS NULL) <> (project_id IS NULL))
);

CREATE UNIQUE INDEX agent_holds_ticket_role ON agent_holds (ticket_id, role) WHERE ticket_id IS NOT NULL;
CREATE UNIQUE INDEX agent_holds_project_role ON agent_holds (project_id, role) WHERE project_id IS NOT NULL;
CREATE INDEX agent_holds_tool ON agent_holds (tool, reason);
