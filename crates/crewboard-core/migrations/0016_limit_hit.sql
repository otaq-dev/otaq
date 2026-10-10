-- The plan limit being hit (USG-03, USG-04; d0020, d0046), carried in every plan_limits row while it lasts:
-- limit_kind:          'session', 'weekly' or 'other' (a model's own limit, a spend limit); NULL when no limit is active
-- limit_text:          Claude Code's message ("You've hit your session limit · resets 3:45pm")
-- limit_hit_at:        when it was first hit
-- limit_resets_at:     when it resets; NULL = "resets at an unknown time"
-- limit_reset_source:  'status_line' (rate_limits.*.resets_at) or 'limit_text' (parsed from the message)
-- limit_reset_seen_at: when the reset was observed (bookkeeping only: nothing resumes; the banner stays until
--                      the user's Resume all)
ALTER TABLE plan_limits ADD COLUMN limit_kind TEXT;
ALTER TABLE plan_limits ADD COLUMN limit_text TEXT;
ALTER TABLE plan_limits ADD COLUMN limit_hit_at BIGINT;
ALTER TABLE plan_limits ADD COLUMN limit_resets_at BIGINT;
ALTER TABLE plan_limits ADD COLUMN limit_reset_source TEXT;
ALTER TABLE plan_limits ADD COLUMN limit_reset_seen_at BIGINT;
