-- When the ticket entered its current status (SDD §12.2; d0016): the age of a needs-you item ("Start
-- interrupted · 20 min", "parent SHOP-26 was closed without merging · 4 min"), oldest first inside an inbox group.
-- Set by every statement that changes `state` (kept when the state stays the same); rows from before this
-- migration take their last update time.
ALTER TABLE tickets ADD COLUMN state_since BIGINT;
UPDATE tickets SET state_since = updated_at;
