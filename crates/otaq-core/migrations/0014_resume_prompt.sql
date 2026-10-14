-- Answering an Interrupted orange ticket from the inbox (ATT-08; d0045): one click resumes the conversation and
-- delivers the answer. The answer waits here until the start sequence resumes the session, which gets it as the
-- resumed conversation's first prompt (`claude --resume <id> "<answer>"`), so it arrives once the CLI is ready.
-- Cleared once that start has run.
ALTER TABLE tickets ADD COLUMN resume_prompt TEXT;
