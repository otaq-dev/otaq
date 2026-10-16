-- M19 Ticket environment (T-0304). Decisions 0085, 0086, 0087. Never edit once committed.
-- `build_state`: where the ticket's ignored build output is. `present` in the worktree; `freed` moved to warm stock
-- at Review entry; `fetching` while it is brought back (Send back, Fix, Run, Shell). `build_problem`: why the last
-- Free failed (Cleanup "could not free"). `resource_name`: the name Otaq gave at Create; Remove uses only this.
-- `schema_overlap`: JSON `[{other, path}]`, other Review tickets that add files under the same schema path (M19-e).
ALTER TABLE tickets ADD COLUMN build_state TEXT NOT NULL DEFAULT 'present'
    CHECK (build_state IN ('present', 'freed', 'fetching'));
ALTER TABLE tickets ADD COLUMN build_problem TEXT;
ALTER TABLE tickets ADD COLUMN resource_name TEXT;
ALTER TABLE tickets ADD COLUMN resource_state TEXT NOT NULL DEFAULT 'none'
    CHECK (resource_state IN ('none', 'created', 'failed', 'removed'));
ALTER TABLE tickets ADD COLUMN resource_problem TEXT;
ALTER TABLE tickets ADD COLUMN schema_overlap TEXT;
