-- T-0262 (EPC-11, EPC-13; d0080): queueing an epic. On an epic, its queue mode: none (never queued, or taken
-- back), whole (the user queued the whole epic: new children join the queue) or single (the user queued one child
-- row: new children wait as "not queued"), and when it was last set. On a child, who added it to the queue by
-- joining a whole-queued epic (an actor: the user, or the ticket whose agent ran `otaq create --parent`); it
-- clears when the child starts or is taken back. Existing epics start as none.
ALTER TABLE tickets ADD COLUMN epic_queue_mode TEXT NOT NULL DEFAULT 'none'
    CHECK (epic_queue_mode IN ('none', 'whole', 'single'));
ALTER TABLE tickets ADD COLUMN epic_queue_mode_at BIGINT;
ALTER TABLE tickets ADD COLUMN joined_queue_by TEXT;
