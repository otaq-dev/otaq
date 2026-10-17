# Claude Code status-line payloads

What Claude Code passes on stdin to the status-line command Otaq runs agents with (`otaq statusline`, USG-04). The
Claude Code adapter's tests (`src/agent_tools/claude/limits.rs`) read them (decision 0088, testing).

**Shaped from the status-line docs, 2026-10-10** (code.claude.com/docs/en/statusline, "Available data"; Claude Code
2.1.296), not captured from a real session yet: the fields, their nesting and the example values follow the docs'
full JSON example. `rate_limits.*.resets_at` is in Unix epoch seconds. When a captured payload replaces these files,
record the Claude Code version here.

| File | Case |
|---|---|
| `with_rate_limits.json` | a Pro or Max account after the first response: both windows (5-hour and weekly) |
| `without_rate_limits.json` | an API-key user, or before the first response: no `rate_limits` |
