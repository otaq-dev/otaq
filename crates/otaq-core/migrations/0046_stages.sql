-- M31 Stages replace the fixed columns and states (T-0400; STG-04, STG-05). Decisions 0108, 0079. Never edit once
-- committed.
--
-- Where a ticket is (STG-04): `stage_id` is the id of the flow stage it is in, or of the Agent stage it waits
-- before; NULL in Backlog, Ready for Dev (waiting before the first Agent stage) and Done. Every project runs the
-- default flow in M31 (`dev` → `validate` → `review` → `deliver`), so each of today's states moves to its stage:
-- Dev's to `dev`, the validation states (and an Interrupted or limit-held validation) to `validate`, Ready for
-- Review to `review`, PR open to `deliver`. The stored states keep their names. `epic_in_progress` was never
-- written; any such row becomes `backlog`. `board_column` goes: nothing reads or writes a column any more (the
-- board computes its columns from the flow, STG-08). `ALTER TABLE … DROP COLUMN` rebuilds the table inside SQLite
-- (no index, view, trigger or constraint names the column), so the 0026 rebuild by hand isn't needed.
ALTER TABLE tickets ADD COLUMN stage_id TEXT;
UPDATE tickets SET state = 'backlog' WHERE state = 'epic_in_progress';
UPDATE tickets SET stage_id = CASE
    WHEN state IN ('validating', 'validator_needs_you', 'validation_failed', 'validation_no_result') THEN 'validate'
    WHEN state IN ('interrupted', 'waiting_for_limit_reset')
         AND interrupted_from IN ('validating', 'validator_needs_you') THEN 'validate'
    WHEN state IN ('working', 'working_in_background', 'asks_you', 'needs_you', 'idle', 'waiting_for_limit_reset',
                   'interrupted', 'resume_failed', 'merge_conflict_fixing', 'auto_merge_stopped') THEN 'dev'
    WHEN state = 'ready_for_review' THEN 'review'
    WHEN state IN ('to_deliver', 'pr_open') THEN 'deliver'
    ELSE NULL
END;
ALTER TABLE tickets DROP COLUMN board_column;

-- Stage runs (STG-05): one row each time a ticket runs a stage: the stage, its kind when it ran (`agent`, `check`
-- for an agent check, `you` for a human check, `delivery`), the session and the agent and exact model it ran on,
-- an agent check's round, how it ended and a summary. A run the ticket skipped has `result` 'skipped' and the same
-- start and end. `result` NULL while it runs; a run that ended without one of the others is 'no_result'.
-- `validation_id` links an agent check's run to its round in `validations`, which keeps the report and notes.
CREATE TABLE stage_runs (
    id            TEXT   NOT NULL PRIMARY KEY,
    ticket_id     TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    stage_id      TEXT   NOT NULL,
    kind          TEXT   NOT NULL CHECK (kind IN ('agent', 'check', 'you', 'delivery')),
    session_id    TEXT,
    agent         TEXT,
    model         TEXT,
    round         BIGINT,
    result        TEXT   CHECK (result IN ('passed', 'failed', 'done', 'skipped', 'cancelled', 'no_result')),
    summary       TEXT,
    validation_id TEXT   REFERENCES validations (id) ON DELETE SET NULL,
    started_at    BIGINT NOT NULL,
    ended_at      BIGINT
);
CREATE INDEX stage_runs_ticket ON stage_runs (ticket_id, started_at);
CREATE INDEX stage_runs_session ON stage_runs (session_id);

-- Today's validation rounds become runs of `validate` (STG-05), linked to their rows.
INSERT INTO stage_runs (id, ticket_id, stage_id, kind, session_id, agent, model, round, result, validation_id,
                        started_at, ended_at)
SELECT v.id, v.ticket_id, 'validate', 'check', v.session_id, s.named_agent_name, COALESCE(s.model_ran, s.model),
       v.round,
       CASE
           WHEN v.outcome IS NULL THEN NULL
           WHEN v.outcome = 'pass' THEN 'passed'
           WHEN v.outcome = 'fail' THEN 'failed'
           WHEN v.outcome = 'skipped' THEN 'skipped'
           WHEN v.outcome IN ('cancelled', 'stopped', 'interrupted') THEN 'cancelled'
           ELSE 'no_result'
       END,
       v.id, v.started_at, v.ended_at
FROM validations v LEFT JOIN sessions s ON s.id = v.session_id;

-- Tickets in a stage today have a run of it that is still open (their earlier stages left no record).
INSERT INTO stage_runs (id, ticket_id, stage_id, kind, session_id, agent, model, started_at)
SELECT lower(hex(randomblob(16))), t.id, t.stage_id,
       CASE t.stage_id WHEN 'dev' THEN 'agent' WHEN 'validate' THEN 'check' WHEN 'review' THEN 'you'
                       ELSE 'delivery' END,
       s.id, s.named_agent_name, COALESCE(s.model_ran, s.model), COALESCE(t.state_since, t.updated_at)
FROM tickets t
LEFT JOIN sessions s ON s.id = (
    SELECT id FROM sessions WHERE ticket_id = t.id AND kind = 'agent' AND t.stage_id = 'dev'
    ORDER BY started_at DESC LIMIT 1)
WHERE t.stage_id IS NOT NULL
  AND NOT EXISTS (SELECT 1 FROM stage_runs r WHERE r.ticket_id = t.id AND r.ended_at IS NULL);

-- The runs follow the tickets, whichever code moves them (implementation choice, T-0400): a ticket whose stage
-- changes ends its open run ('done' when it was merged or moved on to another stage, 'cancelled' when it went back
-- to Backlog or to waiting, or was closed; a move that knows better sets the result first) and opens a run of the stage it entered, with the
-- live session of that kind. Waiting before a stage isn't a run.
CREATE TRIGGER stage_runs_follow_tickets AFTER UPDATE OF stage_id, state ON tickets
WHEN NEW.stage_id IS NOT OLD.stage_id
  OR (NEW.stage_id IS NOT NULL
      AND OLD.state IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted')
      AND NEW.state NOT IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted'))
BEGIN
    UPDATE stage_runs
    SET ended_at = CAST(unixepoch('subsec') * 1000 AS INTEGER),
        result = COALESCE(result, CASE WHEN NEW.state = 'merged' OR NEW.stage_id IS NOT NULL THEN 'done'
                                       ELSE 'cancelled' END)
    WHERE ticket_id = NEW.id AND ended_at IS NULL;
    INSERT INTO stage_runs (id, ticket_id, stage_id, kind, session_id, agent, model, started_at)
    SELECT lower(hex(randomblob(16))), NEW.id, NEW.stage_id, k.kind, s.id, s.named_agent_name,
           COALESCE(s.model_ran, s.model), CAST(unixepoch('subsec') * 1000 AS INTEGER)
    FROM (SELECT CASE
              WHEN NEW.state IN ('validating', 'validator_needs_you', 'validation_failed', 'validation_no_result')
                OR NEW.interrupted_from IN ('validating', 'validator_needs_you') THEN 'check'
              WHEN NEW.state = 'ready_for_review' THEN 'you'
              WHEN NEW.state IN ('to_deliver', 'pr_open') THEN 'delivery'
              ELSE 'agent'
          END AS kind) k
    LEFT JOIN sessions s ON s.id = (
        SELECT id FROM sessions
        WHERE ticket_id = NEW.id AND ended_at IS NULL
          AND kind = CASE k.kind WHEN 'agent' THEN 'agent' WHEN 'check' THEN 'validator' ELSE 'delivery' END
        ORDER BY started_at DESC LIMIT 1)
    WHERE NEW.stage_id IS NOT NULL
      AND NEW.state NOT IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted');
END;

-- A session started for the stage a ticket runs: its run records it (the latest, after a Resume), with its agent
-- and model; the exact model arrives later from the tool (ADP-04).
CREATE TRIGGER stage_runs_take_sessions AFTER INSERT ON sessions
WHEN NEW.ticket_id IS NOT NULL AND NEW.kind IN ('agent', 'validator', 'delivery')
BEGIN
    UPDATE stage_runs
    SET session_id = NEW.id, agent = NEW.named_agent_name, model = COALESCE(NEW.model_ran, NEW.model)
    WHERE ticket_id = NEW.ticket_id AND ended_at IS NULL
      AND kind = CASE NEW.kind WHEN 'agent' THEN 'agent' WHEN 'validator' THEN 'check' ELSE 'delivery' END;
END;

CREATE TRIGGER stage_runs_take_session_models AFTER UPDATE OF model_ran, named_agent_name ON sessions
BEGIN
    UPDATE stage_runs
    SET model = COALESCE(NEW.model_ran, NEW.model, model), agent = COALESCE(NEW.named_agent_name, agent)
    WHERE session_id = NEW.id;
END;

-- An agent check's round is a run (STG-05): the first round of a visit takes the run the ticket opened by entering
-- the stage; a later round in the same visit (Retry validation) is a new run. The round's end is the run's end.
CREATE TRIGGER stage_runs_take_rounds AFTER INSERT ON validations
BEGIN
    UPDATE stage_runs SET round = NEW.round, validation_id = NEW.id, started_at = NEW.started_at
    WHERE id = (SELECT id FROM stage_runs
                WHERE ticket_id = NEW.ticket_id AND ended_at IS NULL AND kind = 'check' AND validation_id IS NULL
                ORDER BY started_at DESC LIMIT 1);
    INSERT INTO stage_runs (id, ticket_id, stage_id, kind, round, validation_id, started_at)
    SELECT lower(hex(randomblob(16))), NEW.ticket_id, t.stage_id, 'check', NEW.round, NEW.id, NEW.started_at
    FROM tickets t
    WHERE t.id = NEW.ticket_id AND t.stage_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM stage_runs WHERE validation_id = NEW.id);
END;

CREATE TRIGGER stage_runs_take_round_sessions AFTER UPDATE OF session_id ON validations
WHEN NEW.session_id IS NOT NULL
BEGIN
    UPDATE stage_runs
    SET session_id = NEW.session_id,
        agent = (SELECT named_agent_name FROM sessions WHERE id = NEW.session_id),
        model = (SELECT COALESCE(model_ran, model) FROM sessions WHERE id = NEW.session_id)
    WHERE validation_id = NEW.id;
END;

CREATE TRIGGER stage_runs_end_rounds AFTER UPDATE OF ended_at ON validations
WHEN NEW.ended_at IS NOT NULL
BEGIN
    UPDATE stage_runs
    SET ended_at = NEW.ended_at,
        result = CASE NEW.outcome
                     WHEN 'pass' THEN 'passed'
                     WHEN 'fail' THEN 'failed'
                     WHEN 'skipped' THEN 'skipped'
                     WHEN 'cancelled' THEN 'cancelled'
                     WHEN 'stopped' THEN 'cancelled'
                     WHEN 'interrupted' THEN 'cancelled'
                     ELSE 'no_result'
                 END,
        summary = COALESCE(summary, NEW.report)
    WHERE validation_id = NEW.id;
END;
