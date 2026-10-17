# Claude Code hook payloads

One file per row of SDD §11.4, named `<event>-<case>.json` (`hook_event_name` is the event). The Claude Code
adapter's tests (`src/agent_tools/claude/hooks.rs`) and `otaq hook --tool claude <event>`
(`crates/otaq-cli/tests/hook.rs`) read them (decision 0088, testing).

**Shaped from the hooks reference, 2026-10-10** (code.claude.com/docs/en/hooks; Claude Code 2.1.296), not captured
from a real session yet. The captured payloads replace these files as a small follow-up; record the Claude Code
version here when they do. The limit and failure payloads (`StopFailure-*`) can't be triggered at will and stay
real-shaped: their texts follow the error reference and the messages Claude Code shows.

| File | Row of §11.4 |
|---|---|
| `Notification-permission_prompt.json` | a permission prompt → Needs you |
| `Notification-elicitation_dialog.json` | an elicitation → Needs you |
| `Notification-helper_agent.json` | a permission prompt inside a helper agent → Needs you (bot icon) |
| `Notification-idle_prompt.json`, `Notification-auth_success.json` | other types → ignored |
| `Stop-idle.json` | nothing pending → Idle, or "turn ended" after a submit or an ask |
| `Stop-background.json` | a background shell and a helper agent (and a finished task, left out) → Working in background |
| `Stop-cron.json` | a scheduled wakeup → Working in background |
| `StopFailure-session_limit.json`, `StopFailure-weekly_limit.json` | the usage limit → Waiting for limit reset |
| `StopFailure-capacity.json` | a 429 that isn't the plan's limit → a logged failure |
| `StopFailure-other.json` | another failure → a logged failure |
| `UserPromptSubmit.json` | a new turn → Working |
