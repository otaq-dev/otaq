-- The squash commit a Local merge made for this ticket (GIT-03; d0059): an epic's diff is grouped by child with
-- these commits (REV-01, EPC-05). NULL for tickets that were never merged locally.
ALTER TABLE tickets ADD COLUMN merge_commit_sha TEXT;
