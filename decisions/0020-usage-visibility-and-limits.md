# 0020. Usage visibility and plan-limit handling

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0007, 0011, 0012, 0016, 0018

## Context
5–10 agents can use up a subscription's usage limit quickly. When the limit is hit, every agent stops
at once. If the factory doesn't know why, it keeps starting queued tickets and the inbox floods with
duplicate items.

## Decision
- **Read-only usage tracking:** the factory reads the session log files that Claude Code already writes,
  and never modifies or wraps the CLI (0018). It shows **tokens used**, not what remains of the plan.
  - Tokens per card, and today's total in the board header.
  - **Dollar cost estimate: an optional setting, off by default** (meaningful only for pay-per-token/API-key
    users, 0012).
- **Plan limit reached:**
  - **One banner** ("Usage limit reached, resets at 3pm") instead of one inbox item per agent.
  - **No new tickets start** while the limit is in effect (the scheduler holds, 0007).
  - After the reset, the user clicks **Resume all**. Nothing restarts on its own (0018).

**How the limit is detected (grilling session, refines the above):** Claude Code's log files are documented as internal and
may change, so they are not used for limits. Checked against the Claude Code docs on 2026-10-09:
1. **"It happened":** the Crewboard plugin ships a **`StopFailure` hook** (it fires when a turn ends on an API error; the
   normal `Stop` hook doesn't fire then). When the error is `rate_limit` and the last message is the limit text ("You've hit
   your session limit · resets 3:45pm" / "…weekly limit…"), the agent shows **"Waiting for limit reset"** and the banner
   appears (0046). The user's Claude settings are not touched.
2. **"When it resets":** agents launch with Crewboard's own **transparent status-line command** (via `--settings`). It records
   `rate_limits.five_hour` / `rate_limits.seven_day` (`used_percentage`, `resets_at`), then runs the **user's own**
   status-line command and prints its output unchanged, so the user's status line looks the same (0012). Fallback: the reset
   time from the on-screen limit text. If both fail, the banner says "resets at an unknown time".
3. **Plan meter (refines "not what remains of the plan"):** the board header shows **"5h · 82% · week · 40%"** from the same
   data, so the limit is visible before agents stop. Pro/Max only; hidden for API-key users, who get automatic retries
   from Claude Code instead.
4. **Safety net:** if detection misses (the CLI changed), any Idle inbox item has a **"This is the usage limit"** button. One
   click turns the Idle items into the single banner.

## Alternatives considered
- No visibility (the user checks `/usage`): rejected, because the "8 agents stop at once" case stays unexplained.
- Per-ticket token budgets: rejected, because they change CLI behavior. They belong in the user's own skill or hook if wanted.

## Consequences
- The log file format and the wording of the limit message must be verified during the agent-signals
  milestone. Because this is read-only, a format change only breaks the display.

## Changelog
- 2026-10-09: created
- 2026-10-09: detection via the plugin's StopFailure hook, reset time via a transparent status-line command, a plan meter in the header, and a manual safety net (grilling session, user decision).
- 2026-10-09: enterprise-plan limit banners (Request more / Retry) are not in v0.1; an enterprise limit shows the "reset time unknown" banner and Resume all serves as retry (ticket-writing review Q7, user decision).
