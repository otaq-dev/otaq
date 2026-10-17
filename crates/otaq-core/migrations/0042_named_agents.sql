-- M22 Named agents (T-0334; AGT-01, AGT-04, ADP-04). Decisions 0086, 0088. Never edit once committed.
-- No rows are copied from anything older (0088: "No backward compatibility … There is no upgrade path"): the starter
-- entry and the role defaults are made by the service when the table is empty.

-- The named agents (AGT-01): a stable id that tickets, defaults and sessions point at, so a rename changes nothing.
-- `name` is unique ignoring case (the CLI picks agents by name; the service also compares beyond ASCII).
-- `tool` is the stored `agent_tools::Tool` (`claude`). `model` is the text passed to the tool as it is: NULL = the
-- tool's default (no model option), else a follow choice (`opus`) or an exact id, always one picked from the
-- tool's own list. `fallback_id`: M23 (AGT-08); nothing in M22 reads or writes it. The mark (AGT-05, T-0337):
-- `mark_text` the line shown ("Claude Code couldn't run `claude-fable-5-1` on SHOP-48"), `mark_model` the model
-- refused, `mark_ticket` the ticket key it happened on, `marked_at`; all NULL = no mark.
CREATE TABLE named_agents (
    id          TEXT   NOT NULL PRIMARY KEY,
    name        TEXT   NOT NULL,
    tool        TEXT   NOT NULL,
    model       TEXT,
    use_for     TEXT   NOT NULL DEFAULT '',
    fallback_id TEXT   REFERENCES named_agents (id) ON DELETE SET NULL,
    mark_text   TEXT,
    mark_model  TEXT,
    mark_ticket TEXT,
    marked_at   BIGINT,
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL
);
CREATE UNIQUE INDEX named_agents_name ON named_agents (lower(name));
CREATE INDEX named_agents_tool ON named_agents (tool, created_at);

-- A ticket's Developer and Validator rows and an epic's Epic lead row (AGT-04): one per (ticket, role); no row, or a
-- row whose agent was deleted (SET NULL: only Done and Closed tickets can be left so), is empty = "use the default".
-- `set_by` `user` ("set by you") or `agent` with `picked_by` (the agent's name at pick time, "Wishlist lead") and its
-- `reason` ("config only"). Which role fits which kind (no `lead` on a ticket, only `lead` on an epic) is the
-- service's rule.
CREATE TABLE ticket_agent_rows (
    ticket_id      TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    role           TEXT   NOT NULL CHECK (role IN ('developer', 'validator', 'lead')),
    named_agent_id TEXT   REFERENCES named_agents (id) ON DELETE SET NULL,
    set_by         TEXT   NOT NULL CHECK (set_by IN ('user', 'agent')),
    picked_by      TEXT,
    reason         TEXT,
    set_at         BIGINT NOT NULL,
    PRIMARY KEY (ticket_id, role)
);
CREATE INDEX ticket_agent_rows_agent ON ticket_agent_rows (named_agent_id);

-- The named agent a session ran on (ADP-04, AGT-05; filled by T-0336): no foreign key, so a session outlives its
-- agent; `named_agent_name` is the name at start ("Light (deleted)", "Light (started on claude-haiku-5-5)"),
-- `model_ran` the exact model the tool reported. `model` (M20) stays the model as passed. The same three on
-- `project_agents` (Project agents and epic leads).
ALTER TABLE sessions ADD COLUMN named_agent_id TEXT;
ALTER TABLE sessions ADD COLUMN named_agent_name TEXT;
ALTER TABLE sessions ADD COLUMN model_ran TEXT;
ALTER TABLE project_agents ADD COLUMN named_agent_id TEXT;
ALTER TABLE project_agents ADD COLUMN named_agent_name TEXT;
ALTER TABLE project_agents ADD COLUMN model_ran TEXT;
