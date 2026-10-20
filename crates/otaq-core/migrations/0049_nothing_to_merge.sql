-- M32 The card of a ticket closed by `otaq stage done` (T-0411; AGM-10, Q276). Decisions 0108, 0079. Never edit once
-- committed.
--
-- A ticket an agent closed with `otaq stage done` (nothing to deliver) keeps the agent's reason here, so its card says
-- "Closed · nothing to merge" with it (History has it in the Closed event). Every close writes it: NULL for any other
-- close; the card reads it only while the ticket is Closed.
ALTER TABLE tickets ADD COLUMN nothing_to_merge TEXT;
