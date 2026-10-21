-- M33 Agent checks in any flow (T-0417; FLW-13, Q259). Decisions 0108, 0063, 0079. Never edit once committed.
--
-- Each agent check counts its own fails in a row (d0108 "After a Fail" 2): one row per ticket and agent check stage,
-- written when the check counts a round, the user allows another round, or the user moves the card past it.
-- `fails` the fails in a row (a pass ends them); `extra` the rounds Give it another round added to the stage's
-- rounds; `passed_round` the round the check last passed in ("Validated · round 2"), NULL unless its last verdict
-- was a pass; `moved_past` how the user moved the card past it in this walk (`failed`: Move on anyway after its last
-- round, "Failed Security 3 times · moved by you"; `skipped`: Skip this check, "Skipped Security · by you"), which
-- forces a click-mode Delivery; `updated_at` when the row last changed (the card's round marker names the check that
-- counted last). A human Fail or a drag back deletes a ticket's rows (every counter at 0); entering a check again
-- clears its `moved_past`.
CREATE TABLE check_counters (
    ticket_id    TEXT    NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    stage_id     TEXT    NOT NULL,
    fails        INTEGER NOT NULL DEFAULT 0,
    extra        INTEGER NOT NULL DEFAULT 0,
    passed_round INTEGER,
    moved_past   TEXT CHECK (moved_past IN ('failed', 'skipped')),
    updated_at   BIGINT  NOT NULL,
    PRIMARY KEY (ticket_id, stage_id)
);

-- The one counter per ticket goes, unconverted (d0108 changelog 2026-10-20: no released users).
ALTER TABLE tickets DROP COLUMN validation_round;
ALTER TABLE tickets DROP COLUMN validation_rounds_extra;
