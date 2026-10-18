-- M26 A note per answer option (T-0374; ATT-17). Decisions 0061, 0083. Never edit once committed.
-- Each option's note (its pros and cons), as a JSON list the same length as `options_json`, `null` for an option
-- without one. NULL = no notes (every question asked before M26 reads that way). Stored whole: never cut.
ALTER TABLE human_questions ADD COLUMN option_notes_json TEXT;
