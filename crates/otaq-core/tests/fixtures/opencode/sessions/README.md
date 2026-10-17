# OpenCode's sessions

Read by the OpenCode adapter's resume check, Resume failed and "Before the restart"
(`src/agent_tools/opencode/sessions.rs`, REC-02, REC-03). OpenCode 2.0.17, a scratch home, no model call, folder
paths replaced (`/Users/me/…`).

| File | Case | How it was made |
|---|---|---|
| `session-get.json` | `opencode api --standalone GET /api/session/<id>` for a session it has | captured |
| `session-get-not-found.txt` | the same for an unknown id (exit 1) | captured |
| `tui-session-not-found.bin` | the terminal output of `opencode --standalone --session <unknown id>` (no prompt) | captured |
| `export.json` | `opencode session export --standalone <id>`: a user message, the agent's text and tools, a command the user ran, OpenCode's own records | **written** from the export schema in OpenCode's API description, imported with `opencode session import` and exported back; T-0369 replaces it with a real session's export |
