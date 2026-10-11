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
- **Appearance (0069, 0070):** one **theme** picked from the 15 bundled ones (the same picker as onboarding, brief 14);
  no Light / Dark / System mode and no "In use" tag. A **Theme
  credits** row opens a dialog: each source project, the themes taken from it, its copyright line, license and source link,
  then the full MIT text. Terminal font size.
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

## M12 additions (design update K; decisions 0063, 0064, 0068)
Each is a global default with a per-project override ("Use global" when empty):
- **Workflow skills** gain a fourth: **Validate** (default `/crewboard:validate`).
- **Validation**: on/off (on). Off: "Submit goes straight to Ready for Review. Auto-merge does nothing." (crewboard sample).
- **Validation rounds**: 1–5 (default 3).
- **Auto-merge**: **Off** (default) · **Epic children only** · **All tickets**, each with a one-line explanation ("Validated epic
  children are merged into their epic branch; you still review the epic" / "Validated tickets are merged into the base branch,
  locally, never pushed"). Disabled with a note while validation is off.
- **Project agent instructions**: text, for example "Use my /import-github skill. Split into tickets of about one day."

## About (design update O; decision 0071)
A last **About** section in global Settings: the Crewboard mark (placeholder until the logo is decided), name and tagline;
version and commit with **Copy version info**; "© 2026 The Crewboard Authors · MIT License" with **View license**; rows for
**Open-source licenses** (a dialog: filter, license groups that fold open, each crate's source link, Copy all notices) and
**Theme credits** (the 0069 dialog); the Anthropic trademark line. No website or repository links.
