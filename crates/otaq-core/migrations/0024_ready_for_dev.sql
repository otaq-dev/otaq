-- T-0178 (SCH-01, TKT-06, TKT-09; d0066, d0065): Queued becomes its own column, Ready for Dev. Queued tickets
-- move there; ready_since keeps when a queued ticket became ready to start (queued while ready, or its last parent
-- merged), the last key of the start order (pinned → priority → ready the longest). The scheduler fills it in.
UPDATE tickets SET board_column = 'ready_for_dev'
WHERE state IN ('queued', 'waiting_for_slot', 'held', 'parent_closed', 'start_interrupted');
ALTER TABLE tickets ADD COLUMN ready_since BIGINT;
