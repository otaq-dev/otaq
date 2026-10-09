# 17: Project terminal

Read `00-README.md` and `01-design-system.md` first.

## Context (0019, refined in the grilling session)
A **plain terminal in the project's own folder** (the user's repo checkout, e.g. `~/code/shop-web`), one per project. It
does **not** start Claude by itself. The user can:
- create and split tickets and epics with `crewboard create`, `create --parent SHOP-40`, `dep add` (full access)
- run any project command
- start `claude` themselves, for example to run their own planning skill or to make a change without a ticket
New tickets land in **Backlog**, marked "created by agent" when Claude made them. It survives closing the window.

## Design
1. **Opening the Project terminal** from the header (per project). Propose where it lives: a bottom drawer, a
   tab or a separate panel.
2. **The live session:** the user runs `claude` and types "Split the checkout feature into tickets". Claude creates SHOP-18–SHOP-21 with
   dependencies, and **the tickets appear in Backlog in real time** (a subtle highlight), with the Stack
   relationships visible.
3. **The review moment:** a summary of what the agent created ("4 tickets, 3 dependencies") with links. Undo
   per ticket (delete, with a confirmation).

## States
- Empty: a shell prompt in `~/code/shop-web`, with a hint ("run `claude` to plan with your skills, or `crewboard create`")
- Plain shell use (for example `git log`, `pnpm test`)
- Running and creating tickets
- Ended
- A cycle rejected by the tool (Claude tried to create a dependency loop, so an error appears in the session)
