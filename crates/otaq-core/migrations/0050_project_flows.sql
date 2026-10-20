-- M33 A project's flow, stored (T-0412; FLW-01). Decisions 0108, 0004, 0063, 0064, 0079. Never edit once committed.
--
-- Each project's applied flow and every flow it applied before (FLW-01): one row per version, numbered from 1 per
-- project; `yaml` the flow document as applied (STG-01's format: its agents and its stages); who applied it
-- (`shipped` for the version a project gets when it is added, `user`, `project_agent` with the agent's name in
-- `agent_name`); `preset` the shipped preset file it came from (`otaq-default.yaml`), NULL otherwise; `current` on
-- exactly one row per project, the flow the board runs. Rows are never changed once written except `current`.
CREATE TABLE project_flows (
    project_id TEXT    NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    version    INTEGER NOT NULL,
    yaml       TEXT    NOT NULL,
    applied_by TEXT    NOT NULL CHECK (applied_by IN ('shipped', 'user', 'project_agent')),
    agent_name TEXT,
    preset     TEXT,
    applied_at BIGINT  NOT NULL,
    current    BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (project_id, version)
);
CREATE UNIQUE INDEX project_flows_current ON project_flows (project_id) WHERE current;

-- Every project stored today starts on the shipped default flow as it is now (Validate in its own column).
INSERT INTO project_flows (project_id, version, yaml, applied_by, preset, applied_at, current)
SELECT id, 1, 'agents:
  - name: Developer
    skills: [work-ticket, commit]
    model: default
  - name: Validator
    skills: [validate]
    model: default
flow:
  - id: dev
    name: Dev
    kind: agent
    column: own
    agent: Developer
    skill: work-ticket
  - id: validate
    name: Validate
    kind: check
    by: agent
    column: own
    agent: Validator
    skill: validate
    rounds: 3
  - id: review
    name: Review
    kind: check
    by: you
    column: own
  - id: deliver
    name: Done
    kind: delivery
    column: with-previous
    agent: Developer
    mode: click
    delivers: either
', 'shipped', 'otaq-default.yaml', CAST(unixepoch('subsec') * 1000 AS INTEGER), TRUE
FROM projects;

-- No migration from M32 (d0108 changelog 2026-10-20, no released users): the Validation, Validation rounds and
-- Auto-merge settings go unconverted, global and per project; so do the move-over's record and the names it bound
-- the default flow's agents by, the `stands_for` link of imported skills (Q296), and M31's named-agent tables the
-- move-over used to drop.
DELETE FROM settings
WHERE key IN ('validation', 'validation_rounds', 'auto_merge', 'agents.moved_over', 'agents.flow_names');
UPDATE projects
SET settings_json = json_remove(settings_json, '$.validation', '$.validation_rounds', '$.auto_merge')
WHERE json_valid(settings_json);
ALTER TABLE agent_skills DROP COLUMN stands_for;
DROP TABLE IF EXISTS ticket_agent_rows;
DROP TABLE IF EXISTS named_agents;
