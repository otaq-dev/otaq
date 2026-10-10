-- The optional dollar estimate (USG-02; d0020): what a session's responses of that day would cost at the API's
-- prices, in millionths of a dollar, priced per response by its model when the counts are read
-- (`usage::pricing`). Rows from before this migration start at 0 and are filled by the next read.
ALTER TABLE usage ADD COLUMN cost_micro_usd BIGINT NOT NULL DEFAULT 0;
