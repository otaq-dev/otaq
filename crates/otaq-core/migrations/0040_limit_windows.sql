-- T-0313 (USG-09, USG-03, USG-04; d0088): usage limits per agent tool. The fixed five-hour and weekly slots of
-- the old plan table become a list of windows per tool, and the active limit is stored with its tool. No rows are
-- copied (d0088: no backward compatibility): the next status-line update fills the windows.
DROP TABLE plan_limits;

-- A tool's current windows: the set from its latest report, one row per tool and key. A report replaces the set.
-- key:            the tool's own key (Claude Code: its status line's window names)
-- length_minutes: 300, 10080; NULL for a window without a fixed length
-- used_percent:   0-100
-- resets_at:      Unix milliseconds; NULL when the tool doesn't say
-- credits_*:      where the tool counts credits; NULL otherwise
-- observed_at:    when this set was reported
CREATE TABLE limit_windows (
    tool           TEXT             NOT NULL,
    key            TEXT             NOT NULL,
    length_minutes INTEGER,
    used_percent   DOUBLE PRECISION NOT NULL,
    resets_at      BIGINT,
    credits_used   DOUBLE PRECISION,
    credits_total  DOUBLE PRECISION,
    observed_at    BIGINT           NOT NULL,
    PRIMARY KEY (tool, key)
);

-- The usage limit that was hit, one row per tool, while its banner is up (until the user's Resume all, d0020).
-- kind:          'session', 'weekly' or 'other' (a model's own limit, a spend limit)
-- text:          the tool's message ("You've hit your session limit · resets 3:45pm")
-- hit_at:        when it was first hit
-- resets_at:     when it resets; NULL = "resets at an unknown time"
-- reset_source:  'status_line' (the matching window's reset) or 'limit_text' (the limit message)
-- reset_seen_at: when the reset was observed (bookkeeping only: nothing resumes)
CREATE TABLE active_limits (
    tool          TEXT   NOT NULL PRIMARY KEY,
    kind          TEXT   NOT NULL,
    text          TEXT   NOT NULL,
    hit_at        BIGINT NOT NULL,
    resets_at     BIGINT,
    reset_source  TEXT,
    reset_seen_at BIGINT
);
