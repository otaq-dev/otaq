-- T-0145 (REV-10, REV-06; d0043, d0050): after a push (Open PR or Push to PR) Crewboard compares the pushed branch's
-- content with what the user reviewed. pushed_differs holds the difference while they differ: JSON
-- {"reviewed": "<sha>", "pushed": "<sha>", "files": ["…"]}; NULL when the pushed content is what was reviewed.
ALTER TABLE tickets ADD COLUMN pushed_differs TEXT;
