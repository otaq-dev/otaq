# 0040. "Agent instructions": the user's own rules for agents, per project, in the app

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009, 0018, 0023, 0027, 0032, 0036

## Context
0018, 0027 and 0036 say workflow policy (tests, retries, how to investigate) lives in the user's own skills. But Crewboard
always starts its own `/crewboard:work-ticket`, and nothing defined where the user's rules go. Editing the shipped skill
would be overwritten on update and couldn't differ per project. (Raised by the product owner, review 2.)

## Decision
- An **"Agent instructions"** plain-text field, with a **global default** (Settings) and a **per-project override**
  (project settings). It is stored in the app (0023) and never written into the repo.
- **`crewboard prime`** prints these instructions. Every Crewboard skill (`work-ticket`, `address-review`, `restack`)
  tells the agent to **follow them**, so first attempts, rework and restacks all use the same rules.
- The instructions can name the user's own skills, for example: "Use my /tdd skill. Ask before coding. Run `pnpm test` before
  submitting." The user's skills in `~/.claude/skills` are available in every agent session anyway.
- **Crewboard keeps its own steps** (get context, submit for review). The user's text can't break the path to Review.
- **Not adopted for now:** replacing the start command per project. It can be revisited if instructions prove too weak.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: refined by 0050: Workflow skills (Commit, Push, Open PR) use the same global + per-project settings pattern; branch naming rules can live in Agent instructions.
