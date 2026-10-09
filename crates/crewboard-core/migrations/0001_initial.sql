-- Crewboard v0.1 initial schema (SDD §8, d0003, d0045).
--
-- Written to stay portable to Postgres (d0003):
--   * ids are TEXT (UUIDv7 strings made by the app), never rowid / AUTOINCREMENT;
--   * every column has an explicit type: TEXT, BIGINT, BOOLEAN, DOUBLE PRECISION;
--   * timestamps are BIGINT milliseconds since the Unix epoch (UTC), named *_at;
--   * JSON is stored as TEXT (Postgres could use JSONB later).
-- Derived values (inbox items, readiness, sort order, cleanup candidates) are not stored.
-- Never edit this file once committed: later changes go into new migrations.

-- A project is one git repository (d0024). Removing a project deletes its tickets, never the repo.
CREATE TABLE projects (
    id                 TEXT    NOT NULL PRIMARY KEY,
    key                TEXT    NOT NULL UNIQUE,        -- SHOP; never changes (d0029)
    name               TEXT    NOT NULL,
    repo_path          TEXT    NOT NULL UNIQUE,
    base_branch        TEXT    NOT NULL,
    next_ticket_number BIGINT  NOT NULL DEFAULT 1,     -- numbers are never reused (d0029)
    settings_json      TEXT    NOT NULL DEFAULT '{}',  -- per-project settings (setup/run cmd, Open URL, env files, ...)
    created_at         BIGINT  NOT NULL
);

-- Global settings as key/value pairs.
CREATE TABLE settings (
    key        TEXT   NOT NULL PRIMARY KEY,
    value_json TEXT   NOT NULL,
    updated_at BIGINT NOT NULL
);

-- Tickets and epics. Keys: SHOP-18 is (number 18, child_number 0); the epic child SHOP-40.1 is
-- (number 40, child_number 1, epic_id = SHOP-40's id).
CREATE TABLE tickets (
    id                TEXT    NOT NULL PRIMARY KEY,
    project_id        TEXT    NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    number            BIGINT  NOT NULL,
    epic_id           TEXT    REFERENCES tickets (id),
    child_number      BIGINT  NOT NULL DEFAULT 0,       -- 0 = not an epic child
    next_child_number BIGINT  NOT NULL DEFAULT 1,       -- epics only; child numbers are never reused (d0039)
    kind              TEXT    NOT NULL CHECK (kind IN ('ticket', 'epic')),
    title             TEXT    NOT NULL,
    body              TEXT    NOT NULL DEFAULT '',      -- "What and why" (Markdown)
    external_id       TEXT,                             -- PAY-123; epics and standalone tickets (d0059)
    board_column      TEXT    NOT NULL,                 -- "column" is an SQL keyword
    state             TEXT    NOT NULL,
    outcome           TEXT    CHECK (outcome IN ('merged', 'closed')),
    pinned_at         BIGINT,
    parked            BOOLEAN NOT NULL DEFAULT FALSE,
    base_branch       TEXT,                             -- resolved base (d0051)
    branch_name       TEXT,
    branch_locked     BOOLEAN NOT NULL DEFAULT FALSE,
    base_sha          TEXT,
    review_head_sha   TEXT,
    review_entered_at BIGINT,
    worktree_path     TEXT,
    agent_port        BIGINT,
    run_port          BIGINT,
    pr_url            TEXT,
    pr_number         BIGINT,
    sent_back_round   BIGINT  NOT NULL DEFAULT 0,
    created_by        TEXT    NOT NULL,                 -- actor: the user or an agent (d0032 --actor)
    updated_by        TEXT,
    started_at        BIGINT,
    created_at        BIGINT  NOT NULL,
    updated_at        BIGINT  NOT NULL,
    UNIQUE (project_id, number, child_number)
);
CREATE INDEX tickets_epic ON tickets (epic_id);

-- "Done when": an ordered checklist.
CREATE TABLE ticket_done_when (
    ticket_id TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    position  BIGINT NOT NULL,
    text      TEXT   NOT NULL,
    added_by  TEXT   NOT NULL,
    PRIMARY KEY (ticket_id, position)
);

-- Dependencies: ticket_id depends on depends_on_id (a DAG; cycles are refused by the app).
CREATE TABLE ticket_deps (
    ticket_id     TEXT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    depends_on_id TEXT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    PRIMARY KEY (ticket_id, depends_on_id),
    CHECK (ticket_id <> depends_on_id)
);
CREATE INDEX ticket_deps_parent ON ticket_deps (depends_on_id);

-- Old keys that keep resolving after "Move to epic" (SHOP-51 → SHOP-50.2, d0044).
CREATE TABLE ticket_key_aliases (
    old_key   TEXT NOT NULL PRIMARY KEY,
    ticket_id TEXT NOT NULL REFERENCES tickets (id) ON DELETE CASCADE
);
CREATE INDEX ticket_key_aliases_ticket ON ticket_key_aliases (ticket_id);

-- One row per session ever run. Every Claude session ID is kept (d0045).
CREATE TABLE sessions (
    id                TEXT   NOT NULL PRIMARY KEY,
    project_id        TEXT   NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    ticket_id         TEXT   REFERENCES tickets (id) ON DELETE CASCADE,  -- NULL for the Project terminal
    kind              TEXT   NOT NULL CHECK (kind IN ('agent', 'answer', 'delivery', 'shell', 'project', 'run')),
    pty_id            TEXT,
    claude_session_id TEXT,
    state             TEXT   NOT NULL,
    started_at        BIGINT NOT NULL,
    ended_at          BIGINT,
    end_reason        TEXT
);
CREATE INDEX sessions_ticket ON sessions (ticket_id, started_at);
CREATE INDEX sessions_project ON sessions (project_id);
CREATE INDEX sessions_claude_session ON sessions (claude_session_id);

-- Audit log: History tab and the "last signal" on cards (d0027).
CREATE TABLE events (
    id           TEXT   NOT NULL PRIMARY KEY,
    ticket_id    TEXT   REFERENCES tickets (id) ON DELETE CASCADE,
    kind         TEXT   NOT NULL,
    actor        TEXT   NOT NULL,
    payload_json TEXT   NOT NULL DEFAULT '{}',
    at           BIGINT NOT NULL
);
CREATE INDEX events_ticket ON events (ticket_id, at);

-- Line comments made during review; sent with Send back.
CREATE TABLE review_comments (
    id         TEXT   NOT NULL PRIMARY KEY,
    ticket_id  TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    round      BIGINT NOT NULL,
    file       TEXT   NOT NULL,
    line       BIGINT NOT NULL,
    side       TEXT   NOT NULL,
    body       TEXT   NOT NULL,
    created_at BIGINT NOT NULL,
    sent_at    BIGINT
);
CREATE INDEX review_comments_ticket ON review_comments (ticket_id, round);

-- Review actions.
CREATE TABLE reviews (
    id        TEXT   NOT NULL PRIMARY KEY,
    ticket_id TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    round     BIGINT NOT NULL,
    action    TEXT   NOT NULL CHECK (action IN ('send_back', 'approve_local', 'open_pr', 'push')),
    note      TEXT,
    at        BIGINT NOT NULL
);
CREATE INDEX reviews_ticket ON reviews (ticket_id, round);

-- Agent-to-agent questions (d0030).
CREATE TABLE messages (
    id             TEXT   NOT NULL PRIMARY KEY,
    from_ticket_id TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    to_ticket_id   TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    question       TEXT   NOT NULL,
    answer         TEXT,
    delivery       TEXT   CHECK (delivery IN ('live', 'resumed', 'inbox')),
    asked_at       BIGINT NOT NULL,
    delivered_at   BIGINT,
    answered_at    BIGINT
);
CREATE INDEX messages_from ON messages (from_ticket_id);
CREATE INDEX messages_to ON messages (to_ticket_id);

-- Text waiting to be typed into a session once its input line is empty (d0048).
CREATE TABLE pending_deliveries (
    id         TEXT   NOT NULL PRIMARY KEY,
    session_id TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    kind       TEXT   NOT NULL CHECK (kind IN ('agent_message', 'ticket_change', 'continue_after_limit')),
    text       TEXT   NOT NULL,
    created_at BIGINT NOT NULL
);
CREATE INDEX pending_deliveries_session ON pending_deliveries (session_id, created_at);

-- Token usage per session and day (d0020). day is YYYY-MM-DD in local time.
CREATE TABLE usage (
    session_id         TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    day                TEXT   NOT NULL,
    tokens_in          BIGINT NOT NULL DEFAULT 0,
    tokens_out         BIGINT NOT NULL DEFAULT 0,
    cache_read_tokens  BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (session_id, day)
);
CREATE INDEX usage_day ON usage (day);

-- Plan-limit observations (d0020); the newest row is the current state.
CREATE TABLE plan_limits (
    id                     TEXT             NOT NULL PRIMARY KEY,
    five_hour_used_percent DOUBLE PRECISION,
    five_hour_resets_at    BIGINT,
    seven_day_used_percent DOUBLE PRECISION,
    seven_day_resets_at    BIGINT,
    limit_active           BOOLEAN          NOT NULL DEFAULT FALSE,
    observed_at            BIGINT           NOT NULL
);
CREATE INDEX plan_limits_observed ON plan_limits (observed_at);
