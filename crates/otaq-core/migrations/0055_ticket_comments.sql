-- M36 Ticket comments and reports (T-0442; CMT-01, CMT-03, CMT-04). Decisions 0109, 0079. Never edit once committed.
--
-- A comment (CMT-01) belongs to one ticket. `n` is its number on the ticket (1, 2, 3…), never reused: a deleted
-- comment keeps its row with `deleted_at` set (History shows what it said, CMT-14), and no list or `prime` shows it.
-- The author is `user` (`author_name` is the OS user), `agent` (a stage's agent: `author_id` is the flow agent's id,
-- `author_name` its name, `author_tool` the agent tool, `stage_id` and `round` the stage and check round it wrote
-- from) or `lead` (an epic lead: `author_id` is its epic's id, one lead per epic whichever session runs it). A report
-- (CMT-03) is a file in the data folder: `report_file` is its path relative to the data folder
-- (`reports/<repo>/<KEY>/<comment id>.txt`), kept as written so a later key change (Move to epic) still finds it, with
-- its size in bytes and its line count. The rows go with their ticket; the files are removed by Delete (d0060).
CREATE TABLE ticket_comments (
    id           TEXT   NOT NULL PRIMARY KEY,
    ticket_id    TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    n            BIGINT NOT NULL,
    author_kind  TEXT   NOT NULL CHECK (author_kind IN ('user', 'agent', 'lead')),
    author_id    TEXT,
    author_name  TEXT,
    author_tool  TEXT,
    stage_id     TEXT,
    round        BIGINT,
    body         TEXT   NOT NULL,
    report_file  TEXT,
    report_bytes BIGINT,
    report_lines BIGINT,
    posted_at    BIGINT NOT NULL,
    edited_at    BIGINT,
    deleted_at   BIGINT,
    UNIQUE (ticket_id, n)
);

-- For the hand-overs and the read gate (T-0443, T-0444; CMT-05, CMT-08), added here so M36 has one migration:
-- `handed_over_at` is when Otaq started or resumed the run with a new comment, `primed_at` when the run's session last
-- ran `prime`. Nothing reads them yet.
ALTER TABLE stage_runs ADD COLUMN handed_over_at BIGINT;
ALTER TABLE stage_runs ADD COLUMN primed_at BIGINT;
