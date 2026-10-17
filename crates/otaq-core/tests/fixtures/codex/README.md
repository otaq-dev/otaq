# Codex payloads

Read by the Codex adapter's tests (`src/agent_tools/codex/`) and by `otaq hook --tool codex <event>`
(`crates/otaq-cli/tests/suite/hook.rs`), decision 0088 (testing).

**Captured** with Codex CLI 0.157.1 on 2026-10-17 (the hands-on check of Codex, Copilot and OpenCode, and the models
check), from a throwaway `CODEX_HOME`. The `app_server/` answers are from 2026-10-18 (T-0351), with `CODEX_HOME` and `HOME` in a
scratch folder (paths shortened to `/tmp/probe/…`). Paths are shortened to `/Users/me/…`; the account ids, the instructions and the
conversation's own messages (`response_item`, `turn_context`, `world_state`, `item_completed`) are left out of the
session files, which the adapter doesn't read.

| File | What |
|---|---|
| `hooks/<event>.json` | the hook payloads of a TUI session (`SessionStart-startup` / `-resume`, `UserPromptSubmit`, `PermissionRequest` for `curl`, `Interrupt` after Esc, `Stop`, `SessionEnd`, `PreToolUse`, which Otaq doesn't map) |
| `session_file/turns.jsonl` | a TUI session of several turns: `token_count` with `total_token_usage` and the Plus account's one weekly `rate_limits.primary`, one turn aborted by Esc |
| `session_file/model_refused.jsonl` | the TUI's first turn on a model the ChatGPT account can't use (`task_complete` with the HTTP 400 in `error.message`) |
| `session_file/failed_signed_out.jsonl` | a failed turn while signed out (401; captured by T-0349 with `codex exec`) |
| `app_server/model_list-page{1,2,3}.json` | `codex app-server` `model/list {includeHidden: false, limit: 3}` answers, followed by `nextCursor` (T-0351; signed out, so Codex's bundled list) |
| `app_server/hooks_list-untrusted.json` | `hooks/list` right after `codex plugin add otaq@otaq-local`: Otaq's five plugin hooks, untrusted, with their keys and hashes (T-0351) |
| `app_server/hooks_list-mixed.json` | `hooks/list` with the user's own `hooks.json` hook (untrusted), four of Otaq's trusted and one `modified` (its recorded hash changed) (T-0351) |
| `app_server/config_batch_write.json` | the `config/batchWrite` answer after trusting Otaq's five hooks (T-0351) |

**Written, not captured** (no limit could be triggered): `session_file/limit.jsonl`, from the hands-on report row 7
(`rate_limits` in `token_count`, `usage_limit_exceeded` in the binary, probably in `task_complete.error.codex_error_info`)
and Codex's limit message. T-0369 replaces it with a capture.
