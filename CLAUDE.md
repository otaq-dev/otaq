# Otaq

Otaq (called "Crewboard" until `decisions/0076`, and "Software Factory"/"factory" before `decisions/0028`) is a Rust kanban desktop app (Tauri UI since `decisions/0074`; built on GPUI before that) that drives Claude Code agents: drag a ticket to Dev, and an agent works
on it in its own worktree, then moves it to Ready for Review. See `DESIGN.md` for the overview.

## Decisions are the source of truth

- All product and architecture decisions are recorded in `decisions/`. Read `decisions/README.md` first.
- **If anything the user asks for conflicts with an accepted decision, point out the conflict
  explicitly** (quote the decision number and its text) and confirm with the user before acting. Once it
  is confirmed, update the decision log following the rules in `decisions/README.md`
  (refinement → edit the file and add a changelog line; reversal → write a new decision and mark the old
  one superseded).
- Log every new decision in `decisions/` as it is made, and keep `DESIGN.md` consistent with it.
- `design-briefs/` contains the UI design requests derived from the decisions. When a decision changes,
  update the affected briefs too.

## Working style

- Discuss and decide **one topic at a time**. Don't bundle several open decisions into one message.
