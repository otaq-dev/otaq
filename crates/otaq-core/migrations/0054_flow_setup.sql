-- M35 Asking each tool and Flow setup (T-0433; ASK-03, ASK-04). Decisions 0108, 0079. Never edit once committed.
--
-- Several proposals (ASK-03): while a project is in Flow setup it keeps one open proposal per flow agent, so
-- `flow_proposals.project_id` is no longer UNIQUE (outside Flow setup Otaq still keeps one per project, FLW-03).
-- `tool` is the agent tool of the Project agent that proposed it (`NULL` for the user's own); during Flow setup a
-- new proposal replaces only the open one with the same `tool`. SQLite can't drop a UNIQUE constraint, so the table
-- is rebuilt (nothing references it); its numbers go on from where they were, never reused.
CREATE TABLE flow_proposals_new (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       TEXT   NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    tool             TEXT,
    yaml             TEXT   NOT NULL,
    source_json      TEXT   NOT NULL,
    proposed_by_json TEXT   NOT NULL,
    created_at       BIGINT NOT NULL
);
INSERT INTO flow_proposals_new (id, project_id, tool, yaml, source_json, proposed_by_json, created_at)
SELECT id, project_id, NULL, yaml, source_json, proposed_by_json, created_at FROM flow_proposals;
-- The copy started the new table's counter at its highest row; it goes on from the old counter instead.
DELETE FROM sqlite_sequence WHERE name = 'flow_proposals_new';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'flow_proposals_new', seq FROM sqlite_sequence WHERE name = 'flow_proposals';
DROP TABLE flow_proposals;
DELETE FROM sqlite_sequence WHERE name = 'flow_proposals';
ALTER TABLE flow_proposals_new RENAME TO flow_proposals;
CREATE INDEX flow_proposals_project ON flow_proposals (project_id, tool);

-- Flow setup (ASK-04): set when the user adds the project, cleared once its flow is settled (Use this flow, a preset
-- applied, Use the default, Skip). While it is set the project shows no board and takes no tickets.
ALTER TABLE projects ADD COLUMN flow_setup_since BIGINT;
