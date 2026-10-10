-- Done cleanup (REC-05; d0018, d0033): a finished ticket's worktree is removed unless it has uncommitted changes;
-- then it is kept and flagged here for the Cleanup view (T-0116), which lets the user delete it.
ALTER TABLE tickets ADD COLUMN worktree_kept BOOLEAN NOT NULL DEFAULT FALSE;
