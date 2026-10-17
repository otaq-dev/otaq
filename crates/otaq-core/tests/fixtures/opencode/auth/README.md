# `opencode auth list --standalone --format json`

Read by the OpenCode adapter's signed-in check (`src/agent_tools/opencode/detect.rs`, ADP-10). **Captured** with
OpenCode 2.0.17 on 2026-10-18 in a scratch home, no model call. The plain form prints "No authenticated
integrations" when there are none.

| File | Case |
|---|---|
| `list-none.json` | nothing signed in: Connected all the same, "free models only" (Q-K) |
| `list-two.json` | a stored test key (Anthropic) and an environment variable (`OPENAI_API_KEY`) |
