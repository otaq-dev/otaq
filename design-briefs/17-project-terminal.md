# 17: Project terminals

Read `00-README.md` and `01-design-system.md` first. Project agents (the Claude sessions that create tickets) are in brief 18;
both live in the same place in the app.

## Context (0019, refined by 0068)
A **plain terminal in the project's own folder** (the user's repo checkout, e.g. `~/code/shop-web`). It does **not** start
Claude by itself. **Several can be open per project** ("Terminal 1", "Terminal 2"), owned by the background service, so they
survive closing the window. The user can:
- create and split tickets and epics with `crewboard create`, `create --parent SHOP-40`, `dep add` (full access)
- run any project command
- start `claude` themselves, for example to make a change without a ticket

For creating tickets with Claude's help, the user opens a **Project agent** instead (brief 18).

## Design
1. **Opening Project terminals** from the header (per project), ⇧⌘T / Ctrl+Shift+T. Propose where they live together with
   Project agents (brief 18): for example a bottom drawer or a panel with a tab per terminal and per agent, and a "+" menu
   with **New terminal** / **New Project agent**.
2. **Several terminals:** switching between "Terminal 1" and "Terminal 2", closing one (ends its shell), and which one ⇧⌘T
   focuses (the last used, or a new one if none is open).
3. Tickets created from a terminal with `crewboard create` appear in Backlog in real time (a subtle highlight).

## States
- Empty: a shell prompt in `~/code/shop-web`, with a hint ("`crewboard create` makes tickets; open a Project agent to let
  Claude help")
- Plain shell use (for example `git log`, `pnpm test`)
- Two terminals open, one focused
- Ended (the shell exited)
- After a restart: terminals are gone (nothing to recover)
- A cycle rejected by the tool (`crewboard dep add` would create a loop, so an error appears in the shell)

## Decided after design update K (2026-10-11)
- The header button is named **"Terminals & agents"** (tooltip ⇧⌘T); ⇧⌘A opens a new Project agent (0058).
