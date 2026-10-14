# 0061. Questions to the user can carry 2–4 answer options

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0016, 0031, 0032, 0037, 0042, 0048

## Context
Ticket-writing review, Q8. The inbox design (frame 10) shows agent questions with numbered options, one marked
"Recommended" and preselected. `otaq ask human` only accepted free text. The user rejected free-text-only for v0.1.

## Decision
- **Asking:** `otaq ask human --option "<label>" … [--recommended N] --stdin`, for example
  `--option "Postmark" --option "SES" --option "Resend" --recommended 1`, with the question text on stdin (0031).
  **2–4 options**, short labels; `--recommended` is optional. Without options, the question is free text as before.
- **Free text is always possible:** the UI adds **"Other…"**, so the user is never forced into the agent's list.
- **Answering** (inbox, card, panel): options are numbered; the recommended one is marked and preselected; **1–4** picks,
  **Enter** sends, **Other…** lets the user type. The answer is typed into the agent's session under the usual rules (0048),
  for example *"Answer to your question: Postmark (option 1)."*, and is readable with `otaq questions`.
- **Skills:** `work-ticket` and `address-review` teach the agent to offer 2–4 options with a recommendation when there are clear
  alternatives, and to keep open questions free text.
- **Scope:** only questions to the user (`ask human`). Agent-to-agent questions (`ask <KEY>`) stay free text.

## Changelog
- 2026-10-09: created (ticket-writing review Q8, user decision)
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
