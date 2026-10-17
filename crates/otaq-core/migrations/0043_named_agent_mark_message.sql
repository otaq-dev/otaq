-- M22 (T-0337; AGT-05, d0088): the tool's own message behind a named agent's mark ("There's an issue with the
-- selected model (claude-fable-5-1). …"), shown in quotes in the edit dialog. Set and cleared with the mark's other
-- columns. Never edit once committed.
ALTER TABLE named_agents ADD COLUMN mark_message TEXT;
