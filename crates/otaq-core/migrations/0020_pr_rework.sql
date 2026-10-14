-- T-0150 (REV-14; d0041): a ticket with an open PR dragged back from Approved to Ready for Review is "back for
-- rework" until it is submitted again; then Approve offers Push to PR. The card shows which.
ALTER TABLE tickets ADD COLUMN pr_rework BOOLEAN NOT NULL DEFAULT FALSE;
