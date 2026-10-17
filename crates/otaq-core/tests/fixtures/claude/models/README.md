# Claude Code's model list

The answer to the SDK `initialize` control request (`claude -p --input-format stream-json --output-format
stream-json --verbose` with `{"type":"control_request","request_id":"r1","request":{"subtype":"initialize"}}` on
stdin), read by the Claude Code adapter (`src/agent_tools/claude/models.rs`, AGT-02; decision 0088, testing).

**Written from the models report's shape, 2026-10-17** (`reports/Models across the four tools (2026-10-17).md`,
Claude Code row 2), not captured: the five entries and what they resolve to follow the report; the field names
(`value`, `resolvedModel`, `displayName`, `description`, `supportedEffortLevels`) and the `control_response`
envelope were checked in the SDK schema inside the installed Claude Code 2.1.302 binary (read with `strings`,
nothing run). The descriptions and the fields beside `models` are illustrative. The close-out (T-0346) replaces
this file with a captured answer; record the Claude Code version here when it does.

| File | Case |
|---|---|
| `initialize.json` | the five entries: `default` (the tool default), `opus`, `fable`, `sonnet`, `haiku` |
