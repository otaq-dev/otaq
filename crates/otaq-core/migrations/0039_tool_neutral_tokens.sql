-- T-0312 (USG-09; d0088): tokens are stored per session and day the same way for every agent tool: `input` (input
-- not read from a cache), `cached` (input read from a cache), `output`, and `reasoning` where the tool reports it
-- apart from its output (NULL otherwise). The dollar estimate's `cost_micro_usd` is gone (d0086). No rows are
-- copied: the usage reader reads the conversations again on its next tick.
DROP TABLE usage;
CREATE TABLE usage (
    session_id TEXT   NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    day        TEXT   NOT NULL,
    input      BIGINT NOT NULL DEFAULT 0,
    cached     BIGINT NOT NULL DEFAULT 0,
    output     BIGINT NOT NULL DEFAULT 0,
    reasoning  BIGINT,
    PRIMARY KEY (session_id, day)
);
CREATE INDEX usage_day ON usage (day);
