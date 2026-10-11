# 0042. Waiting signals: one orange family (Asks you / Needs you / Idle), cleared by the user's input

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009, 0016 (refined), 0027, 0037

## Context
0037 keeps tickets that need the user in Dev with an orange highlight, but nothing turned it off. A third signal, "idle"
(the agent stopped without submitting or asking), was not orange and not in the inbox, so plain-text questions like "Should
I also update the tests?" were easy to miss. (Raised by the product owner, review 2.)

## Decision
- **One orange family with three labels**, all sorted to the **top of Dev** and listed in **inbox group 1** (0016):
  - **Asks you:** the agent ran `crewboard ask human`.
  - **Needs you:** the agent is waiting on a permission prompt (Notification hook).
  - **Idle:** the agent ended its turn without submitting or asking (Stop hook).
- **Background work is not waiting (grilling session, question 20).** Checked against the Claude Code docs on 2026-10-09:
  the `Stop` hook reports `background_tasks` (shells, subagents, monitors, workflows, teammates) and `session_crons`
  (scheduled wakeups).
  1. When `Stop` fires and either list isn't empty, the ticket shows a neutral **"Working in background"** status: not orange,
     no inbox item, no notification. The card says what it's waiting for ("background: test run, 1 helper agent · 6 min", or
     "wakes up at 4:10pm"). It still counts as working for the optional agent limit (0016).
  2. **Idle** only when `Stop` fires with both lists empty and no submit or ask.
  3. **Needs you** only for real requests: `Notification` types `permission_prompt`, `elicitation_dialog`,
     `elicitation_url_dialog` and `agent_needs_input` (including prompts raised inside a helper agent). `idle_prompt`,
     `auth_success` and the other types are ignored.
  4. Back to working when the next turn starts (`UserPromptSubmit`) or on any hook or `crewboard` activity.
  5. The "last signal" line (0027) counts background activity, so a long test run doesn't look stale.
- **Clearing:** the highlight clears on the **user's input**:
  - pressing Enter in that ticket's terminal (the background service already relays keystrokes, 0022)
  - answering from the inbox
  - or the agent running any `crewboard` command again

  No extra Claude Code hooks are needed.
- **While you're looking (grilling session):** while a ticket's terminal has focus (in the app, or attached with
  `crewboard attach`), that ticket raises **no orange highlight, no inbox item and no OS notification**, for all three
  signals. If you leave the terminal (focus moves elsewhere, the panel closes, you detach) while the agent still waits, it
  turns orange at that moment and follows the normal rules. So chatting with an agent doesn't produce false alerts.
- **Optional agent limit (refines 0016):** when the limit is on, **orange (waiting) tickets count** toward
  `max_concurrency`, along with actively working ones. This is the user's choice. In effect, unanswered questions hold slots, so
  new tickets don't start while agents wait on the user. An agent the user unblocks still always resumes.
- Order inside Dev → Agent Working: **pinned (0047)** → orange (inbox order) → Interrupted / Start interrupted /
  Parent closed → working → Paused. Queued has its own section.

## Changelog
- 2026-10-09: created (product owner review 2, user decision)
- 2026-10-09: Restack conflict removed from the needs-you order; it is a working status now (0005).
- 2026-10-09: Paused no longer occurs in v0.1 (0049).
- 2026-10-09: no alerts for the ticket whose terminal has focus; it turns orange when you leave while it still waits (grilling session, user decision).
- 2026-10-09: background work (Stop's background_tasks / session_crons) shows a neutral "Working in background" status; Needs you only for real request notification types (grilling session, user decision).
- 2026-10-11: the Stop hook doesn't mark an agent Idle while its ticket validates ("Validating…" is neutral); the validator's permission prompts are "Needs you · validator" (0063).
