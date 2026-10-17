# OpenCode's one-off question

`opencode run --standalone --format json` output lines, read by "Ask the agent" (`src/agent_tools/opencode/oneoff.rs`,
RUN-02). OpenCode 2.0.17, a scratch home, folder paths replaced.

| File | Case | How it was made |
|---|---|---|
| `read.jsonl` | the agent reads `package.json`, then answers | captured against a local stand-in model server (a scripted OpenAI-compatible endpoint, no real model) with the question's read-only rules |
| `shell-denied.jsonl` | the model calls `shell` anyway: the rules refuse it, then it answers | the same |
| `model-unavailable.jsonl` | `-m opencode/foo-bar-9`: the error line, exit 1 | captured (OpenCode refuses before any call) |
