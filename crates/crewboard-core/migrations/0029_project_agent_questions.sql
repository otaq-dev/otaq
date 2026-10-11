-- no-transaction
-- T-0200 (PAG-02, MSG-01; d0068, d0030): a Project agent asks too. Its `ask <KEY>` questions have no asking ticket,
-- so messages.from_ticket_id may be empty and from_agent_id names the Project agent instead; its `ask human`
-- questions belong to the Project agent, so human_questions.ticket_id may be empty and project_agent_id names it.
-- Exactly one of each pair is set. SQLite can't relax NOT NULL, so both tables are rebuilt the documented way:
-- foreign keys off (outside a transaction), copy, swap, indexes back, foreign keys on.
PRAGMA foreign_keys = OFF;
BEGIN;
CREATE TABLE messages_new (
    id                  TEXT   NOT NULL PRIMARY KEY,
    from_ticket_id      TEXT   REFERENCES tickets (id) ON DELETE CASCADE,
    from_agent_id       TEXT   REFERENCES project_agents (id) ON DELETE CASCADE,
    to_ticket_id        TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    question            TEXT   NOT NULL,
    answer              TEXT,
    delivery            TEXT   CHECK (delivery IN ('live', 'resumed', 'inbox')),
    asked_at            BIGINT NOT NULL,
    delivered_at        BIGINT,
    answered_at         BIGINT,
    answer_delivered_at BIGINT,
    CHECK ((from_ticket_id IS NULL) <> (from_agent_id IS NULL))
);
INSERT INTO messages_new (id, from_ticket_id, to_ticket_id, question, answer, delivery, asked_at, delivered_at,
                          answered_at, answer_delivered_at)
SELECT id, from_ticket_id, to_ticket_id, question, answer, delivery, asked_at, delivered_at, answered_at,
       answer_delivered_at
FROM messages;
DROP TABLE messages;
ALTER TABLE messages_new RENAME TO messages;
CREATE INDEX messages_from ON messages (from_ticket_id);
CREATE INDEX messages_from_agent ON messages (from_agent_id);
CREATE INDEX messages_to ON messages (to_ticket_id);

CREATE TABLE human_questions_new (
    id               TEXT   NOT NULL PRIMARY KEY,
    ticket_id        TEXT   REFERENCES tickets (id) ON DELETE CASCADE,
    project_agent_id TEXT   REFERENCES project_agents (id) ON DELETE CASCADE,
    question         TEXT   NOT NULL,
    options_json     TEXT   NOT NULL DEFAULT '[]',
    recommended      BIGINT,
    asked_at         BIGINT NOT NULL,
    answer           TEXT,
    answered_at      BIGINT,
    CHECK ((ticket_id IS NULL) <> (project_agent_id IS NULL))
);
INSERT INTO human_questions_new (id, ticket_id, question, options_json, recommended, asked_at, answer, answered_at)
SELECT id, ticket_id, question, options_json, recommended, asked_at, answer, answered_at FROM human_questions;
DROP TABLE human_questions;
ALTER TABLE human_questions_new RENAME TO human_questions;
CREATE INDEX human_questions_ticket ON human_questions (ticket_id, asked_at);
CREATE INDEX human_questions_agent ON human_questions (project_agent_id, asked_at);
COMMIT;
PRAGMA foreign_keys = ON;
