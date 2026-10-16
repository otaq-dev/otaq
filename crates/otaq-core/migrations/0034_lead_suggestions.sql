-- T-0280 (LEAD-08; d0083): an epic lead's suggestion to close or delete a child of its epic
-- (`otaq suggest close|delete <KEY> --stdin`). The lead never closes or deletes; the user confirms (the usual close or
-- delete) or dismisses it from the inbox. One open suggestion per child and kind. A suggestion whose child started
-- or closed meanwhile is left out of the inbox; deleting the child deletes its suggestions.
CREATE TABLE lead_suggestions (
    id           TEXT   NOT NULL PRIMARY KEY,
    epic_id      TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    child_id     TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    kind         TEXT   NOT NULL CHECK (kind IN ('close', 'delete')),
    reason       TEXT   NOT NULL,
    state        TEXT   NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'confirmed', 'dismissed')),
    suggested_by TEXT   NOT NULL,
    created_at   BIGINT NOT NULL,
    resolved_by  TEXT,
    resolved_at  BIGINT
);

CREATE UNIQUE INDEX lead_suggestions_one_open ON lead_suggestions (child_id, kind) WHERE state = 'open';
CREATE INDEX lead_suggestions_by_epic ON lead_suggestions (epic_id);
