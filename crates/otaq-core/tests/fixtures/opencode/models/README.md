# OpenCode's model list

`GET /api/model` and `GET /api/model/default` answers of a short-lived `opencode serve`, read by the OpenCode
adapter (`src/agent_tools/opencode/models.rs`, AGT-02; decision 0088, testing).

**Captured** with OpenCode 2.0.17 on 2026-10-18, its folders (`HOME`, `XDG_*`, `TMPDIR`) in a scratch folder, no
model call. Only the folder paths were replaced (`/Users/me/…`).

| File | Case |
|---|---|
| `free.json` | no provider signed in: OpenCode's 10 free models; `space-bunny-free` has no `family` |
| `signed-in.json` | a test Anthropic key stored (`opencode auth list` shows it): 30 models, Anthropic's and the free ones |
| `default.json` | `/api/model/default` with no provider: the model a session with none runs on |
