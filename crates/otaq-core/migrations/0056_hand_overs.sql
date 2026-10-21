-- M36 Hand-overs become comments (T-0443; CMT-05). Decisions 0109, 0079. Never edit once committed.
--
-- A stage's summary, a check's report and the user's Send back or drag-back note are ticket comments now
-- (`ticket_comments`, 0055). `stage_runs.comment_id` is the comment the run ended with (the agent's `stage next`, the
-- checker's verdict, the user's Send back note…), so History and the stage path still show what ended it.
-- `validations.reason` keeps Otaq's own words for a round that ended without a verdict ("The validator didn't start:
-- …"); a verdict's text is its comment.
ALTER TABLE stage_runs ADD COLUMN comment_id TEXT REFERENCES ticket_comments (id) ON DELETE SET NULL;
ALTER TABLE validations ADD COLUMN reason TEXT;

-- A round's end is its run's end; the report no longer copies into the run (its comment is linked instead).
DROP TRIGGER stage_runs_end_rounds;
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
                 END
    WHERE validation_id = NEW.id;
END;

-- The old hand-over stores go, with no copy (no users yet): the stage summary, the check report and the Send back
-- note are comments now.
ALTER TABLE stage_runs DROP COLUMN summary;
ALTER TABLE validations DROP COLUMN report;
ALTER TABLE reviews DROP COLUMN note;
