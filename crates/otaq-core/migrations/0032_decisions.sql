-- T-0276 (DEC-01, DEC-02; d0084): recorded decisions. An agent that meets a product question it can go on without
-- builds one option and records the choice on its ticket (`otaq decision record --stdin`); nothing is written into
-- the user's repo (d0023). Each row is numbered per ticket from 1 (`ordinal`): its ID is `<KEY>#<ordinal>`
-- (SHOP-40.1#1). `other_options` is a JSON list of at least one option.
-- The lead's mark (T-0280; DEC-04, d0083): a one-line note that a written source contradicts it, with the source
-- (PAY-123); empty until marked. The outcome (T-0277; DEC-05, DEC-06): open until the review keeps or changes it.
CREATE TABLE decisions (
    id            TEXT   NOT NULL PRIMARY KEY,
    ticket_id     TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    ordinal       BIGINT NOT NULL,
    question      TEXT   NOT NULL,
    built         TEXT   NOT NULL,
    other_options TEXT   NOT NULL DEFAULT '[]',
    why           TEXT   NOT NULL,
    recorded_by   TEXT   NOT NULL,
    recorded_at   BIGINT NOT NULL,
    mark_text     TEXT,
    mark_source   TEXT,
    marked_by     TEXT,
    marked_at     BIGINT,
    outcome       TEXT   NOT NULL DEFAULT 'open'
        CHECK (outcome IN ('open', 'kept', 'changed_sent_back', 'changed_fix_child')),
    outcome_text  TEXT,
    fix_child_id  TEXT   REFERENCES tickets (id) ON DELETE SET NULL,
    outcome_at    BIGINT,
    UNIQUE (ticket_id, ordinal)
);
