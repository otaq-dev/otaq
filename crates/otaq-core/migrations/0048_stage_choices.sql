-- M32 Stage choices (T-0407; AGM-08, AGM-11). Decisions 0108, 0087, 0079. Never edit once committed.
--
-- A ticket's choice for one stage that has `options` in its flow (AGM-08): `choice` is `usual` (the stage's own agent,
-- "required" for a skippable stage), `agent` (one of the stage's other agents, `agent_id`) or `skip`. No row = the
-- stage's usual agent. Who set it: `user` (also an agent's `--by-user`, d0087), `project_agent`, `epic_lead` or
-- `agent` (a ticket's own agent), with the setter's name and an agent's reason. A row whose stage the flow no longer
-- has, or whose agent is no longer one of the stage's options, is ignored where it is read. Deleting the agent drops
-- the choice: the stage runs its usual agent again.
CREATE TABLE ticket_stage_choices (
    ticket_id TEXT   NOT NULL REFERENCES tickets (id) ON DELETE CASCADE,
    stage_id  TEXT   NOT NULL,
    choice    TEXT   NOT NULL CHECK (choice IN ('usual', 'agent', 'skip')),
    agent_id  TEXT   REFERENCES agents (id) ON DELETE CASCADE,
    set_by    TEXT   NOT NULL CHECK (set_by IN ('user', 'project_agent', 'epic_lead', 'agent')),
    by_name   TEXT,
    reason    TEXT,
    set_at    BIGINT NOT NULL,
    PRIMARY KEY (ticket_id, stage_id),
    CHECK ((choice = 'agent') = (agent_id IS NOT NULL))
);
