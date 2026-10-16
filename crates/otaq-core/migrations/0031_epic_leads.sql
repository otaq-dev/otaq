-- T-0278 (LEAD-01, LEAD-12; d0083): an epic's lead is a Project agent row with its epic set. One lead per epic:
-- the epic is unique while set. Deleting the epic deletes its lead's row (its sessions stay, as for any Project
-- agent). Existing Project agents have no epic.
ALTER TABLE project_agents ADD COLUMN epic_id TEXT REFERENCES tickets (id) ON DELETE CASCADE;
CREATE UNIQUE INDEX project_agents_epic ON project_agents (epic_id) WHERE epic_id IS NOT NULL;
