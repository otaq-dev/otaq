-- "conflicts with `main`" (GIT-03, GIT-04, GIT-06; d0038, d0044, d0051).
-- conflicts_with:  the branch the ticket's branch conflicts with (the card's flag), set by Local merge's free rebase,
--                  the read-only test merge after a merge, or the rebase of a reopened ticket on its way to Dev.
-- conflict_files:  the conflicting files, as a JSON array of paths.
-- conflict_reason: 'merge' (Local merge or the test merge: Send to agent to fix) or 'reopen' (`crewboard prime`
--                  tells the agent to resolve it first). Cleared when the agent submits again, or when the next test
--                  merge is clean.
ALTER TABLE tickets ADD COLUMN conflicts_with TEXT;
ALTER TABLE tickets ADD COLUMN conflict_files TEXT;
ALTER TABLE tickets ADD COLUMN conflict_reason TEXT;
