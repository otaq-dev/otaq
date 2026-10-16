-- T-0284 (LEAD-06; d0083): an epic child's `otaq ask human` goes to its epic's live lead first. The question is
-- stored as before, and these columns say it went to the lead and where it stands:
--   lead_id       the lead's Project agent row, kept after the question left it (the Messages log names it);
--   lead_epic_id  the epic it was routed through, so the epic's Messages list it even if the child moves;
--   lead_state    with_lead (the lead hasn't acted), answered (the lead's `otaq answer`; `answer` holds its text),
--                 passed_on (`otaq ask human --pass-on`: an "Asks you" item with the lead's recommendation), or
--                 handed_back (the lead's turn ended without either, or it ended: a plain "Asks you");
--   lead_reason   the lead's reason when it passed the question on;
--   lead_at       when the question left the lead.
-- A question that never went to a lead keeps them all empty.
ALTER TABLE human_questions ADD COLUMN lead_id TEXT REFERENCES project_agents (id) ON DELETE SET NULL;
ALTER TABLE human_questions ADD COLUMN lead_epic_id TEXT REFERENCES tickets (id) ON DELETE SET NULL;
ALTER TABLE human_questions ADD COLUMN lead_state TEXT
    CHECK (lead_state IN ('with_lead', 'answered', 'passed_on', 'handed_back'));
ALTER TABLE human_questions ADD COLUMN lead_reason TEXT;
ALTER TABLE human_questions ADD COLUMN lead_at BIGINT;
CREATE INDEX human_questions_lead_epic ON human_questions (lead_epic_id) WHERE lead_epic_id IS NOT NULL;
