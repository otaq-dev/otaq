-- M32 Agents and models (T-0405; AGM-01, AGM-02, AGM-03, AGM-05). Decisions 0108, 0086, 0088, 0040, 0068, 0092.
-- Never edit once committed.
--
-- Every session Otaq starts runs an agent: a name, its instructions, its skills and its own model. The tables come
-- empty: the move-over (`agents::move_over`, run by the store right after the migrations, once, in one transaction)
-- fills them from the shipped `otaq-agents.yaml` and the default flow's `agents:`, carries M31's named agents, role
-- defaults, instructions and imported workflow skills over (AGM-02), records what it dropped and then drops
-- `named_agents` and `ticket_agent_rows`. The tables are left here because only the move-over can read them.

-- An agent (AGM-01): a permanent id (sessions, holds and the flow's names point at it, so a rename changes nothing);
-- a name, unique ignoring case (the service also compares beyond ASCII and refuses `skip` and `_…`); its Markdown
-- instructions; its model: `tool` NULL = `model: default` (the account's default model, resolved at session start),
-- else the tool and `model` as passed to it (NULL = the tool's default, else a follow choice or an exact id from the
-- tool's list); `effort` the tool's own effort / reasoning level, only for a tool with a documented one; `role`
-- `project-agent` or `epic-lead`, each held by exactly one agent; where it came from (`agents_file`, `default_preset`,
-- `you`, `flow` with the project's key in `came_from_project`); the amber mark of M22 (AGT-05).
CREATE TABLE agents (
    id                TEXT   NOT NULL PRIMARY KEY,
    name              TEXT   NOT NULL,
    instructions      TEXT   NOT NULL DEFAULT '',
    tool              TEXT,
    model             TEXT,
    effort            TEXT,
    role              TEXT   CHECK (role IN ('project-agent', 'epic-lead')),
    came_from         TEXT   NOT NULL CHECK (came_from IN ('agents_file', 'default_preset', 'you', 'flow')),
    came_from_project TEXT,
    mark_text         TEXT,
    mark_model        TEXT,
    mark_ticket       TEXT,
    mark_message      TEXT,
    marked_at         BIGINT,
    created_at        BIGINT NOT NULL,
    updated_at        BIGINT NOT NULL,
    CHECK (tool IS NOT NULL OR (model IS NULL AND effort IS NULL))
);
CREATE UNIQUE INDEX agents_name ON agents (lower(name));
CREATE UNIQUE INDEX agents_role ON agents (role) WHERE role IS NOT NULL;

-- An agent's skills, in order (AGM-01): a shipped skill by its name (`work-ticket`), or an imported folder with
-- Otaq's own copy in `$OTAQ_HOME/skills/<imported_id>/` and what the import remembers (WSK-02…WSK-04: the `name:` of
-- its SKILL.md, the original folder, when, the copy's fingerprint, the warnings as a JSON list). `stands_for`: the
-- shipped skill an imported one replaces (`commit`, set by the move-over for M31's workflow skills), NULL otherwise.
CREATE TABLE agent_skills (
    agent_id      TEXT    NOT NULL REFERENCES agents (id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    shipped       TEXT,
    imported_id   TEXT,
    name          TEXT,
    original      TEXT,
    imported_at   BIGINT,
    fingerprint   TEXT,
    warnings_json TEXT    NOT NULL DEFAULT '[]',
    stands_for    TEXT,
    PRIMARY KEY (agent_id, position),
    CHECK ((shipped IS NULL) <> (imported_id IS NULL))
);
CREATE INDEX agent_skills_imported ON agent_skills (imported_id);

-- What a session ran on (AGM-05, ADP-04): the agent's id (no foreign key: a session outlives its agent), its name at
-- start, the exact model the tool reported (M22) and the effort passed. `fallback_from` / `fallback_reason`: the
-- session runs on its tool's fallback, by the user's click, because that tool was at its limit (`tool_limit`), not
-- available (`tool_unavailable`) or no longer listed the model (`model_unavailable`) (AGM-03: "on Codex · Claude
-- Code limit"). The same agent columns on `project_agents` (Project agents and epic leads).
DROP TRIGGER stage_runs_follow_tickets;
DROP TRIGGER stage_runs_take_sessions;
DROP TRIGGER stage_runs_take_session_models;
DROP TRIGGER stage_runs_take_round_sessions;
ALTER TABLE sessions RENAME COLUMN named_agent_id TO agent_id;
ALTER TABLE sessions RENAME COLUMN named_agent_name TO agent_name;
ALTER TABLE sessions ADD COLUMN effort TEXT;
ALTER TABLE sessions ADD COLUMN fallback_from TEXT;
ALTER TABLE sessions ADD COLUMN fallback_reason TEXT
    CHECK (fallback_reason IN ('tool_limit', 'tool_unavailable', 'model_unavailable'));
ALTER TABLE project_agents RENAME COLUMN named_agent_id TO agent_id;
ALTER TABLE project_agents RENAME COLUMN named_agent_name TO agent_name;
ALTER TABLE project_agents ADD COLUMN effort TEXT;

-- A stage run keeps the agent's id and the effort too (STG-05), copied from its session as the name and model are.
ALTER TABLE stage_runs ADD COLUMN agent_id TEXT;
ALTER TABLE stage_runs ADD COLUMN effort TEXT;
UPDATE stage_runs
SET agent_id = (SELECT s.agent_id FROM sessions s WHERE s.id = stage_runs.session_id)
WHERE session_id IS NOT NULL;

-- 0046's triggers, with the renamed columns and the agent's id and effort.
CREATE TRIGGER stage_runs_follow_tickets AFTER UPDATE OF stage_id, state ON tickets
WHEN NEW.stage_id IS NOT OLD.stage_id
  OR (NEW.stage_id IS NOT NULL
      AND OLD.state IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted')
      AND NEW.state NOT IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted'))
BEGIN
    UPDATE stage_runs
    SET ended_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
        result = COALESCE(result, CASE WHEN NEW.state = 'merged' OR NEW.stage_id IS NOT NULL THEN 'done'
                                       ELSE 'cancelled' END)
    WHERE ticket_id = NEW.id AND ended_at IS NULL;
    INSERT INTO stage_runs (id, ticket_id, stage_id, kind, session_id, agent_id, agent, model, effort, started_at)
    SELECT lower(hex(randomblob(16))), NEW.id, NEW.stage_id, k.kind, s.id, s.agent_id, s.agent_name,
           COALESCE(s.model_ran, s.model), s.effort, CAST(unixepoch('subsec') * 1000 AS INTEGER)
    FROM (SELECT CASE
              WHEN NEW.state IN ('validating', 'validator_needs_you', 'validation_failed', 'validation_no_result')
                OR NEW.interrupted_from IN ('validating', 'validator_needs_you') THEN 'check'
              WHEN NEW.state = 'ready_for_review' THEN 'you'
              WHEN NEW.state IN ('to_deliver', 'pr_open') THEN 'delivery'
              ELSE 'agent'
          END AS kind) k
    LEFT JOIN sessions s ON s.id = (
        SELECT id FROM sessions
        WHERE ticket_id = NEW.id AND ended_at IS NULL
          AND kind = CASE k.kind WHEN 'agent' THEN 'agent' WHEN 'check' THEN 'validator' ELSE 'delivery' END
        ORDER BY started_at DESC LIMIT 1)
    WHERE NEW.stage_id IS NOT NULL
      AND NEW.state NOT IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted');
END;

CREATE TRIGGER stage_runs_take_sessions AFTER INSERT ON sessions
WHEN NEW.ticket_id IS NOT NULL AND NEW.kind IN ('agent', 'validator', 'delivery')
BEGIN
    UPDATE stage_runs
    SET session_id = NEW.id, agent_id = NEW.agent_id, agent = NEW.agent_name,
        model = COALESCE(NEW.model_ran, NEW.model), effort = NEW.effort
    WHERE ticket_id = NEW.ticket_id AND ended_at IS NULL
      AND kind = CASE NEW.kind WHEN 'agent' THEN 'agent' WHEN 'validator' THEN 'check' ELSE 'delivery' END;
END;

CREATE TRIGGER stage_runs_take_session_models AFTER UPDATE OF model_ran, agent_name ON sessions
BEGIN
    UPDATE stage_runs
    SET model = COALESCE(NEW.model_ran, NEW.model, model), agent = COALESCE(NEW.agent_name, agent),
        agent_id = COALESCE(NEW.agent_id, agent_id)
    WHERE session_id = NEW.id;
END;

CREATE TRIGGER stage_runs_take_round_sessions AFTER UPDATE OF session_id ON validations
WHEN NEW.session_id IS NOT NULL
BEGIN
    UPDATE stage_runs
    SET session_id = NEW.session_id,
        agent_id = (SELECT agent_id FROM sessions WHERE id = NEW.session_id),
        agent = (SELECT agent_name FROM sessions WHERE id = NEW.session_id),
        model = (SELECT COALESCE(model_ran, model) FROM sessions WHERE id = NEW.session_id),
        effort = (SELECT effort FROM sessions WHERE id = NEW.session_id)
    WHERE validation_id = NEW.id;
END;

-- The holds (SCH-04 `[M23]`, AGM-03), rekeyed to the agent and the tool: `agent_id` the agent it would have started
-- on (the named agents go); `fallback_tool` the tool whose fallback could take it now (the tool's fallback per
-- AGM-03; NULL without one or while it can't run: `fallback_held`); `fallback_asked` the user's Use fallback (a held
-- start then launches on the fallback; nothing switches without it). `role` keeps which session it holds
-- (`developer` an Agent stage's, `validator` an agent check's round, `lead`, `project_agent`). The holds of M31 are
-- kept; the next lift checks each one again on its agent.
CREATE TABLE agent_holds_new (
    id             TEXT    NOT NULL PRIMARY KEY,
    ticket_id      TEXT    REFERENCES tickets (id) ON DELETE CASCADE,
    project_id     TEXT    REFERENCES projects (id) ON DELETE CASCADE,
    role           TEXT    NOT NULL CHECK (role IN ('developer', 'validator', 'lead', 'project_agent')),
    reason         TEXT    NOT NULL CHECK (reason IN ('tool_limit', 'tool_unavailable', 'model_unavailable')),
    tool           TEXT    NOT NULL,
    model          TEXT,
    agent_id       TEXT    REFERENCES agents (id) ON DELETE SET NULL,
    fallback_tool  TEXT,
    fallback_held  BOOLEAN NOT NULL DEFAULT FALSE,
    fallback_asked BOOLEAN NOT NULL DEFAULT FALSE,
    since          BIGINT  NOT NULL,
    CHECK ((ticket_id IS NULL) <> (project_id IS NULL))
);
INSERT INTO agent_holds_new (id, ticket_id, project_id, role, reason, tool, model, since)
SELECT id, ticket_id, project_id, role, reason, tool, model, since FROM agent_holds;
DROP TABLE agent_holds;
ALTER TABLE agent_holds_new RENAME TO agent_holds;
CREATE UNIQUE INDEX agent_holds_ticket_role ON agent_holds (ticket_id, role) WHERE ticket_id IS NOT NULL;
CREATE UNIQUE INDEX agent_holds_project_role ON agent_holds (project_id, role) WHERE project_id IS NOT NULL;
CREATE INDEX agent_holds_tool ON agent_holds (tool, reason);
