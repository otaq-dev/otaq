# 15: Settings

Read `00-README.md` and `01-design-system.md` first.

## Context
All settings live **in the app**. Nothing is written into user repos (0023). Defaults favor "the user is in
control" and "off unless enabled".

## Global settings
- **Agent limits** (0016): optional, **OFF by default**. When on: max agents working at once (suggested 5), and
  "pause new starts when N items wait in the inbox" (suggested 3).
- **Agent instructions** (0040): a global default text, for example "Ask before coding. Run tests before submitting."
- **Usage** (0020): show tokens (always on). **Dollar estimate** toggle, OFF by default, for API-key users.
- **Notifications:** OS notifications for blocking items (on by default).
- **Data:** the location of the database file, with a "Reveal in Finder/Explorer" action. A note that the user's
  normal machine backup covers it (0003).
- **Appearance:** light, dark or system (a proposal). Terminal font size.
- **Tools:** the same checklist as onboarding (brief 14), with Re-check.

## Per-project settings
- Name, **project key (read-only**, for example `SHOP`; 0029), repo path, base branch
- **Agent instructions** (0040): a per-project override of the global text, for example "Use my /tdd skill. Run `pnpm test`
  before submitting. Never touch migrations." Empty means the global default is used.
- **Workflow skills** (0050), also as a global default in global settings: **Commit**, **Push**, **Open PR**, each a skill name
  (default `/crewboard:commit`, `/crewboard:push`, `/crewboard:open-pr`, or the user's own such as `/my-pr`). Empty means
  the global default.
- **Let finished tickets answer questions** (0030): on by default. A Done ticket is resumed, answer-only, when a linked
  agent asks it something. Off: those questions go to the inbox.
- **Setup command** (runs in a new worktree, for example `pnpm install`)
- **Run command** (brief 13), with Ask Claude, for any stack (for example `pnpm dev`, `dotnet run`, `cargo run`)
- **Open URL** (optional, for example `http://localhost:$PORT` or `/swagger`). If empty, no Open button is shown
- **Env files to copy into worktrees** (for example `.env.local`)
- **PR provider (read-only, 0050):** detected from the remote, e.g. "GitHub (detected from origin) · gh: logged in", or
  "Not detected: use your own Open PR skill"
- Remove project (with a confirmation; explain what is deleted: the tickets in Crewboard, never the repo itself)

## States
- A fresh project (everything empty, "zero config works")
- A configured project
- An invalid value (the env file doesn't exist)
- The limits toggled on and off

## Out of scope
A per-project permission-mode override (deferred, 0016). No Claude version settings (0021).
