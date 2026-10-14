-- Why a Queued ticket is held or its start stopped halfway (SES-01, SES-02; d0018, d0059): the step and a
-- message for people, e.g. ('setup', 'setup command failed (exit 1): …') for Start interrupted, or
-- ('sync_base', '`develop` and `origin/develop` have diverged. Push or pull, then retry') for Held.
-- Cleared when the start succeeds or is retried.
ALTER TABLE tickets ADD COLUMN start_problem_step TEXT;
ALTER TABLE tickets ADD COLUMN start_problem_message TEXT;
