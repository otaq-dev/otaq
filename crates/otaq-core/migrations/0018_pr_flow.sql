-- T-0142, T-0143 (REV-10, REV-14, GIT-07; d0050): the Open PR / Push to PR flow of a delivery unit, from the user's
-- click until the skill reports the PR (or the user cancels). One row per ticket at most.
-- kind:           'open' (Open PR) or 'push' (Push to PR #N).
-- state:          'preparing' (the skill runs, no proposal yet), 'proposed' (the dialog shows the proposal),
--                 'pushing' (the user confirmed; Crewboard applied the edits and told the skill to push).
-- proposal_json:  the skill's proposal (`crewboard pr propose --stdin`), checked.
-- confirmed_json: what the user confirmed (branch, messages, title, description), shown by `crewboard prime`.
-- reported:       what the skill reported instead of a PR (a failed push, a missing login).
CREATE TABLE pr_flows (
    ticket_id      TEXT   NOT NULL PRIMARY KEY REFERENCES tickets (id) ON DELETE CASCADE,
    kind           TEXT   NOT NULL CHECK (kind IN ('open', 'push')),
    state          TEXT   NOT NULL CHECK (state IN ('preparing', 'proposed', 'pushing')),
    proposal_json  TEXT,
    confirmed_json TEXT,
    reported       TEXT,
    started_at     BIGINT NOT NULL,
    updated_at     BIGINT NOT NULL
);

-- GIT-07: the branch was pushed (Open PR), so `crewboard branch rename` refuses from now on. (`branch_locked` already
-- means "the base branch is locked because the ticket started", d0051.)
ALTER TABLE tickets ADD COLUMN branch_pushed BOOLEAN NOT NULL DEFAULT FALSE;
