# GitHub Copilot hook payloads and `events.jsonl` samples

The Copilot adapter's tests (`src/agent_tools/copilot/`) and `otaq hook --tool copilot <event>`
(`crates/otaq-cli/tests/suite/copilot_hooks.rs`) read them (decision 0088, testing). Paths are rewritten to
`/wt/shop-web/SHOP-29` (the worktree) and `/Users/me/.copilot` (Copilot's home).

**Captured** on 2026-10-18 from Copilot CLI 1.0.98 loading `plugin-copilot/` with `--plugin-dir`, signed out, against a
local OpenAI-compatible stand-in server (Copilot's own custom-provider mode, `COPILOT_PROVIDER_BASE_URL`), with
`COPILOT_HOME` and `HOME` in a scratch folder:

| File | What |
|---|---|
| `hooks/Stop.json`, `hooks/UserPromptSubmit.json`, `hooks/SessionStart.json` | Claude names, snake_case payloads |
| `hooks/errorOccurred-rate_limit.json` | the last of six `errorOccurred` hooks on a 429 (one per retry, all `recoverable: true`) |
| `events/rate_limit.jsonl` | that run: `session.error` (`rate_limit`) is written about 15 ms **after** the last hook |
| `events/two_turns.jsonl` | a new conversation and its `--resume`: messages, `session.usage_record`, `session.resume` |
| `oneoff/answer.jsonl` | "Ask the agent" (`copilot -p … --output-format json`, reading tools only): the stand-in model reads `package.json` with `view`, then answers |
| `oneoff/no_provider.jsonl` | the same question with the model unreachable: `session.error`, then `result` with `exitCode` 1 |
| `acp/initialize.json`, `acp/session_new-signed_out.json` | `copilot --acp`: the `initialize` answer, and `session/new` signed out (`Authentication required`) |

**Written from the reports** (T-0369 replaces each with a capture from a real account):

| File | Written from |
|---|---|
| `hooks/Notification-permission_prompt.json` | Copilot real-account check, §5 (camelCase `sessionId`) |
| `hooks/Notification-elicitation_dialog.json`, `hooks/Notification-shell_completed.json` | hands-on check, row 3 |
| `hooks/permissionRequest.json` | hands-on check, row 3 |
| `hooks/errorOccurred-unrecoverable.json` | the captured `errorOccurred`, with `recoverable: false` |
| `events/free_auto.jsonl` | real-account check, §2 and §3 (Auto's choice, `quotaSnapshots`, `copilotUsage.totalNanoAiu`), with the `chat` window at 80% left |
| `events/swapped.jsonl` | models report, rows 1 and 4 (an unavailable `--model` starts on Auto) |
| `events/quota.jsonl`, `events/other_error.jsonl` | real-account check, §4 (`session.error` types; never triggered for real) |
| `acp/session_new-free.json` | models report, row 2, and real-account check, §2 (Free: `auto` listed twice) |
| `acp/session_new-paid.json` | the same shape with exact models (a paid plan's list was never seen) |
| `acp/session_new-offline.json` | models report, row 2 (offline) |
| `config/config.json` | Copilot 1.0.98's `config.json` as written in a scratch `COPILOT_HOME` (its comment lines, camelCase keys), with an account and a trusted folder added |
| `config/config-0.0.x.json` | the key names of a `config.json` written by Copilot 0.0.x (snake_case accounts, no comment lines) |
