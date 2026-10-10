-- When the ticket last entered Dev · Queued by the user's drag (SCH-01; d0007): ready tickets start oldest
-- first by this time. Moves between Queued's own sub-states (waiting for a slot, parent closed) and Retry start
-- keep it. Tickets already in Dev take the time they entered their current status.
ALTER TABLE tickets ADD COLUMN queued_at BIGINT;
UPDATE tickets SET queued_at = state_since WHERE board_column = 'dev';
