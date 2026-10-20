-- M33 Delivery modes, PR comments and conflicts (T-0419; FLW-15, FLW-16, FLW-17). Decisions 0108, 0079. Never edit
-- once committed.
--
-- `delivered_without_checks`: an automatic Delivery delivered the ticket with no check passed before it (d0108 rule 8:
-- the card says "Delivered without checks"). `pr_flows.automatic`: the PR flow is an automatic Delivery's, with no
-- dialog: Otaq confirms the skill's proposal itself, and the card is not waiting for the user. The ticket's
-- `auto_merge_conflicts` and `auto_merge_stop` keep their names; they now count and stop a Delivery's conflicts in
-- either mode (the Auto-merge setting went in 0050).
ALTER TABLE tickets ADD COLUMN delivered_without_checks BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE pr_flows ADD COLUMN automatic BOOLEAN NOT NULL DEFAULT FALSE;
