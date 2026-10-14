# 0011. Agents inherit the user's Claude Code permission mode

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0008, 0009, 0012

## Context
Users configure Claude Code differently (default prompts, accept-edits, auto mode, allowlists). The tool
may be published as open source, so it shouldn't impose one security posture.

## Decision
The factory does **not** pass a permission mode or flags that override permissions when it launches
`claude`. Each session uses whatever the user's own Claude Code settings resolve to
(user, project and local settings). If that is auto mode, the agent runs in auto mode; if it is
something else, the agent uses that.

**One narrow exception (grilling session):** Otaq launches `claude` with `--allowedTools "Bash(otaq:*)"`.
It pre-approves only Otaq's own commands. Otherwise, under a prompting mode, every ticket would ask about
`otaq prime` again (Claude saves "don't ask again" in the worktree's local settings, so it doesn't carry over to the
next ticket). The permission mode, the user's allowlist and every other prompt stay exactly as the user set them.

## Consequences
- With a prompting mode, agents will stop on permission prompts. The `Notification` hook (0009) shows
  the "needs you" badge so the user notices.
- Not decided: an optional per-repo override in the app's per-repo settings (0023), which may be added later in line
  with 0012.

## Changelog
- 2026-10-08: created
- 2026-10-09: added a first-run warning for prompting modes (warning only). The per-repo override stays deferred (0016).
- 2026-10-09: launch pre-approves only `Bash(otaq:*)` with `--allowedTools`; the mode is still never changed (grilling session, user decision).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
