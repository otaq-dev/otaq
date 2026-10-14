-- T-0151 (MSG-01, MSG-02; d0030, d0048): agent-to-agent questions are typed as automatic deliveries. A delivery that
-- carries a question or its answer names the message, so the message knows when it reached the other agent.
-- message_part: 'question' (typed into the target's session) or 'answer' (typed into the asker's).
ALTER TABLE pending_deliveries ADD COLUMN message_id TEXT;
ALTER TABLE pending_deliveries ADD COLUMN message_part TEXT;
-- When the answer reached the asker: "asked SHOP-40.1 · waiting" lasts until then (MSG-06).
ALTER TABLE messages ADD COLUMN answer_delivered_at BIGINT;
