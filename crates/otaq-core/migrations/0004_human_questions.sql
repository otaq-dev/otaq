-- Questions an agent asks the user with `otaq ask human` (ATT-01, ATT-12; d0061): the text and its optional
-- 2–4 answer options with the recommended one (1-based). The inbox, card and panel show them (M7); the answer is
-- kept once given (`otaq questions`, M10).
CREATE TABLE human_questions (
    id           TEXT   NOT NULL PRIMARY KEY,
    ticket_id    TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    question     TEXT   NOT NULL,
    options_json TEXT   NOT NULL DEFAULT '[]',
    recommended  BIGINT,
    asked_at     BIGINT NOT NULL,
    answer       TEXT,
    answered_at  BIGINT
);
CREATE INDEX human_questions_ticket ON human_questions (ticket_id, asked_at);
