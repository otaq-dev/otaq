# 05: Ticket side panel (what happens when you click a card)

Read `00-README.md` and `01-design-system.md` first. Tab contents get their own briefs: Terminal (06),
Diff (07), Ticket editor (08), Run and Shell (13). Here you design the **panel frame,
header, tabs, actions and the Stack section**.

## Context
Clicking a card opens a panel showing everything about that ticket (0008, 0022). Tabs: **Terminal · Diff ·
Run · Shell · Ticket · History** (0035, 0036). The board stays visible next to it.

## Design
1. **Panel frame:** opening and closing, resizing, and an optional expand-to-full-window mode. Keyboard: open,
   close, and next/previous ticket.
2. **Panel header:**
   - Number, title, status, project, and branch name (copyable)
   - Base: "on `main`" (no stacking in v0.1, 0049)
   - Tokens used, and last signal
3. **Context-sensitive action bar.** Show the correct buttons for **each state**:
   | Ticket state | Primary actions |
   |---|---|
   | Backlog | Start (to Dev), Edit |
   | Agent Working | Open terminal, Run |
   | Needs you / Asks you | Answer in terminal (primary) |
   | Interrupted | **Resume** |
   | Start interrupted | **Retry start** |
   | Ready for Review | **Approve ▾ (Local merge / Open PR)**, **Send back** |
   | Approved, PR open | Open PR in browser, **Mark merged**, **Mark closed** |
   | Done · Merged | View diff (read-only); worktree removed (or "kept: uncommitted changes" flag) |
   | Done · Closed, not merged | **Reopen** (to Backlog), View diff |
   | Any state | **Close ticket** in the "…" menu, with a confirmation that mentions stopping a running agent (0033) |

   Destructive or costly actions get a confirmation (for example Mark closed). Otaq has no Failed column and no Reset/retry-limit flow (0018).
4. **Tabs**, with indicators: Terminal (live dot), Diff (+412 −38),
   Run (running :4019), Shell, History (number of sessions).
5. **Stack section** (0006), always visible in the panel (propose where: sidebar, header area or a
   collapsible section). An indented list of parents and children with statuses. Click to jump.
   ```
   SHOP-17 Add Stripe client ........ Done
    └ SHOP-18 Checkout API .......... Ready for Review
       └ SHOP-20 Checkout page ...... Queued (waits for SHOP-18, SHOP-19 to merge)
   SHOP-19 Price table .............. Agent Working
       └ SHOP-20
   ```
6. **Messages section** (0030): questions and answers between this ticket's agent and directly linked
   tickets' agents, for example SHOP-20 → SHOP-18: "What does POST /checkout return on a declined card?".
   Show asked, delivered, answered and "sent to your inbox (target has no live session)". Read-only for the user,
   except answering a question that was routed to the inbox.
7. **History tab** (renamed from Runs, so it doesn't clash with Run): a history of sessions for this ticket (started, interrupted, resumed, sent back),
   with time, duration and tokens. Simple list.

## States
- The panel for SHOP-18 (review), SHOP-20 (queued, waiting for merges), SHOP-22 (needs you), SHOP-25 (interrupted), SHOP-26 (PR open), and SHOP-17 (done)
- A ticket with no dependencies (empty Stack section)
- A long title and long branch names

## M12 additions (design update K; decisions 0063–0065)
- **Action bar, new rows:**

  | Ticket state | Primary actions |
  |---|---|
  | Validating… (SHOP-29) | Open terminal (Validator side); "…" menu: **Stop validation**, **Skip to Review** (Submit for review and Validate and submit hidden) |
  | Validation failed 3 times (SHOP-32) | **Move to Review anyway**, **Give it another round**, **Answer the agent** (a note field); the last report visible |
  | Validation ended without a result (SHOP-34) | **Retry validation**, **Move to Review anyway** |
  | Needs you · validator (SHOP-33) | Answer in the terminal (Validator side) |
  | Keeps conflicting / base diverged / checkout dirty (Auto-merge) | **Retry merge**, **Move to Review**, with the message |
  | Agent Working, not mid-turn | "…" menu: **Submit for review** (skips validation) and **Validate and submit** |
- **Close** and **Park** confirmations while validating: "SHOP-29's agent and validator will be stopped."
- **Header:** priority (editable from the panel), and tokens split "agent 61k · validation 23k (2 rounds)".
- **History tab:** one entry per validation round (round, trigger, outcome: passed / failed / cancelled / no result / discarded /
  interrupted / skipped / stopped, duration, tokens) that opens that round's report; "Merged automatically" entries.
- After the last failed round, **Move to Review anyway** is the main button; Give it another round and Answer the agent are
  secondary (no Enter shortcut for another round).

## Update P additions (decision 0078)
- The Ticket tab's read mode renders the description with the same renderer as the editor (brief 08, update P): links open
  in the system browser, task boxes are read-only, and code blocks show "Copy" on hover.
- **Copy as Markdown** and **Expand** (full-window view): icon buttons at the right end of the "What and why" label row, in
  every ticket state. Copy copies the stored Markdown and shows "Copied" for 1.5 s.

## Update Q (0080)
The epic panel opens from a track (double-click or ↵) as well as from the epic card. See `CHANGES-2026-10-15-q.md`.
