-- M33 Proposals and the quiet-board wait (T-0413; FLW-03, FLW-05, FLW-06). Decisions 0108, 0079. Never edit once
-- committed.
--
-- A project's open proposal (FLW-03, FLW-04): at most one per project, a new one replaces it (a new number). `id`
-- is the number the preview and `otaq flow propose` show ("Proposal 3"), never reused. `yaml` is the flow document
-- as proposed, its stage ids already matched to the flow it changes; `source_json` where it came from (a document,
-- a preset, another project, a previous version, the shipped agents file) and `proposed_by_json` who proposed it
-- (the user, or a Project agent's session, which gets the answer).
CREATE TABLE flow_proposals (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id       TEXT   NOT NULL UNIQUE REFERENCES projects (id) ON DELETE CASCADE,
    yaml             TEXT   NOT NULL,
    source_json      TEXT   NOT NULL,
    proposed_by_json TEXT   NOT NULL,
    created_at       BIGINT NOT NULL
);

-- A project's waiting change (FLW-06): the flow as it will be once the board is quiet (`yaml`), who applied it, the
-- preset it came from, and what it changes in a few words for the banner (`changes_json`, a list: "QA after
-- Security", "Review can be skipped"). At most one per project: a second apply merges into it. While it is here, no
-- ticket of the project starts from Ready for Dev.
CREATE TABLE flow_waiting (
    project_id   TEXT   PRIMARY KEY REFERENCES projects (id) ON DELETE CASCADE,
    yaml         TEXT   NOT NULL,
    applied_by   TEXT   NOT NULL CHECK (applied_by IN ('user', 'project_agent')),
    agent_name   TEXT,
    preset       TEXT,
    changes_json TEXT   NOT NULL,
    since        BIGINT NOT NULL
);
