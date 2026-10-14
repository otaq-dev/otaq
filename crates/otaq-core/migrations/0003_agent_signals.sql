-- Agent signals (SDD §11.4, §12.1; d0042, d0027).
-- signal_text:  the needs-you or background detail: the question (Asks you), the prompt (Needs you), the agent's last
--               line (Idle), or "background: test run, 1 helper agent" (Working in background).
-- signal_since: when the current signal began (the card's "· 6 min").
-- wakeup_at:    the next scheduled wakeup while Working in background ("wakes up at 4:10pm").
-- last_signal_at: the latest hook event or `otaq` command of the ticket's agent ("active 40 s ago").
ALTER TABLE tickets ADD COLUMN signal_text TEXT;
ALTER TABLE tickets ADD COLUMN signal_since BIGINT;
ALTER TABLE tickets ADD COLUMN wakeup_at BIGINT;
ALTER TABLE tickets ADD COLUMN last_signal_at BIGINT;
