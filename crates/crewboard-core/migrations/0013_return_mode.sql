-- Resume (REC-01, REC-03; d0018, d0045): an Interrupted or Resume failed ticket the user resumes goes through the start
-- sequence like a parked one, keeping its worktree. 'resume' continues the stored conversation (`claude --resume`);
-- 'scratch' is Start from scratch: a new conversation in the same worktree. Cleared once that start has run.
ALTER TABLE tickets ADD COLUMN return_mode TEXT CHECK (return_mode IN ('resume', 'scratch'));
