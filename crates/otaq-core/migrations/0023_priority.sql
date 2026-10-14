-- T-0177 (TKT-09; d0065): priorities 1 Urgent · 2 High · 3 Normal · 4 Low. Tickets and epics start at 3 (Normal);
-- an epic child keeps NULL and follows its epic's priority until it gets its own.
ALTER TABLE tickets ADD COLUMN priority BIGINT CHECK (priority BETWEEN 1 AND 4);
UPDATE tickets SET priority = 3 WHERE epic_id IS NULL;
