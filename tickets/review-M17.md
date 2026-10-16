# M17 review: Epic lead and recorded decisions

M17 gives every epic a **lead** (decision 0083) and makes agents **record** the product choices they make instead of
blocking on them (decision 0084).

- **The lead** is a read-only `claude` session named after its epic ("Wishlist lead"). It starts from the epic card's
  **Open lead** or when the epic is dragged to be queued. It plans the children with `otaq create --parent`,
  `dep add` and `update`. Otaq wakes it when a child merges or is closed, when a child asks something, and when a child
  gets stuck for the first time. It answers a child's question or passes it on to you with a recommended option. It
  sends one note to a stuck child before the stop reaches you, leaves notes for running children (read at their next
  commit), suggests closing or deleting children that haven't started, and marks a recorded decision that a written
  source contradicts.
- **Where the lead shows:** a Lead Status on the epic card and the track label, a Lead tab in the epic panel ("Changes
  by the lead", its terminal, "Before the restart"), Messages on the epic, its own inbox items, and the children's card
  states ("Asked lead", "via Wishlist lead", "Updated by lead", unread notes, "same file as …").
- **Recorded decisions:** an agent that meets a product question with no clear answer runs
  `otaq decision record` and keeps going. The decisions are listed in the review. **Approve** keeps them
  ("Approve · keeps N decisions"). **Change…** sends the ticket back with the new choice, or, for a merged epic
  child, adds a fix child.

It was built by parallel teammates in worktrees with fast-forward landings (decision 0079) on 2026-10-16.

## 1. What was built

Oldest first, all on `main` (`git log --oneline b397a32..main`):

| Commit | What |
|---|---|
| 4addc32 | M17 planning: epic lead (0083) and recorded decisions (0084) |
| ed0a1bb | 0083 refined: suggestions stay calm; Dismiss doesn't wake the lead |
| 3734ea5 | M17 tickets: T-0276–T-0293, and coverage rows |
| 6691d89 | **T-0282** Core: children changing the same file (`epic_lead/overlap.rs`, `Flags.same_file`) |
| 8d11da1 | **T-0278** Core: the lead session: storage (migration 0031), caller, Open lead, Resume, status |
| 17311b9 | **T-0276** Core: recorded decisions (migration 0032), `otaq decision record`, `show` / `prime` listing them |
| dd6b62e | **T-0277** Core: Approve keeps the decisions; Change… sends back or adds a fix child (Q61) |
| 7a112ad | **T-0279** Core: the lead's lifecycle, the merge / close wake-up, handovers (migration 0033; Q62) |
| 36a6879 | **T-0280** Core: what the lead may change, `otaq suggest`, `otaq decision mark` (migration 0034) |
| d25d294 | **T-0287** Decisions in the review, and "Approve · keeps N decisions" (Q63) |
| 4e88c44 | **T-0289** The Lead Status on the epic card, the track label and the reboot summary |
| f200807 | **T-0284** Core: a child's question goes to the lead first (migration 0035) |
| a3de219 | **T-0281** Core: notes to running children, `otaq notes`, the submit refusal (migration 0036) |
| 7a76473 | **T-0288** Change… and "Add a fix child", Decisions in the Ticket tab, History, the card count (Q64) |
| 0b86ea4 | **T-0290** The epic panel: Ticket · Lead · History, the Lead tab, Messages on the epic (Q65) |
| 3d15efa | **T-0290 fix** a passed-on or handed-back question shows once in Messages |
| f144444 | **T-0285** Core: a stuck child goes to the lead once (migration 0037) |
| 2aa0ab7 | **T-0283** Core: `otaq prime --epic` (Q66) |
| 6eaf9da | **T-0286** Plugin: `/otaq:epic-lead`, recording decisions in `work-ticket`, notes before a commit |
| 99461bb | **T-0291** The children's cards: lead states, notes, same file, the flag order (Q67) |
| 958c189 | **T-0292** The inbox: passed-on questions, stuck children, suggestions, the lead's own questions (Q68) |
| b50341c | **T-0277 fix** every transaction begins IMMEDIATE (`Store::begin_write`), so a busy database waits instead of "database is locked" (the Windows failure, §6) |
| 6b126b9 | **T-0285 fix** the stand-in lead's `PATH` leaves out cargo's build folders, as an agent shell's does (Windows, §6) |
| 43aa1df | **T-0284 fix** the test harness types an agent shell a short line that sources the command, and types it again if it never started (Git for Windows' `sh` drops typed-ahead input under parallel load; §6) |
| baac410 | **T-0293 (Windows fix)** the terminal typing test types `stty size` again when the resize repaint loses a key (§6) |
| (this branch) | **T-0293** This review, the sweep for what 0083 / 0084 replaced, the Windows check |

## 2. How to try it

Node 24 comes from nvm on the Mac; nothing is installed globally. The app starts the service from the `otaq` binary
next to it, so build the workspace once first. Keep `OTAQ_HOME` short (macOS's 104-byte socket path limit).

```sh
export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"
cargo build --workspace                       # the otaq CLI / service the app starts
cd crates/otaq-desktop
npm ci
rm -rf /tmp/m17 && OTAQ_HOME=/tmp/m17 npm run tauri dev
```

- **The example project** (`tickets/m17-tickets.md` § "The example"). In onboarding, add a small temporary repository
  as the project `shop-web` with the key `SHOP` (`git init /tmp/shop-web && git -C /tmp/shop-web commit --allow-empty
  -m init`). Then in Settings → shop-web set **Auto-merge "Epic children only"** and keep validation on. Create the
  tickets with the CLI (`O="env OTAQ_HOME=/tmp/m17 ../../target/debug/otaq -P SHOP"`):

  ```sh
  $O create "Wishlist" --type epic                       # SHOP-40 in the checks below; External ID PAY-123
  $O create "Wishlist API" --parent SHOP-40              # 40.1
  $O create "Wishlist button" --parent SHOP-40 --deps SHOP-40.1
  $O create "Wishlist page" --parent SHOP-40 --deps SHOP-40.2
  $O create "Wishlist share link" --parent SHOP-40       # 40.4
  $O create "Wishlist export (CSV)" --parent SHOP-40     # 40.6 in the checks (40.5 comes from the lead)
  $O create "Search" --type epic                         # SHOP-45, PAY-130, no lead
  $O create "Search index API" --parent SHOP-45
  $O create "Search results page" --parent SHOP-45
  $O create "Split user full name"                       # SHOP-23: one recorded decision in review
  $O create "Checkout API"                               # SHOP-18: in review with no decisions
  ```

  The keys won't be 40, 45, 23 and 18; read the checks with your own numbers. Set the External IDs in each epic's
  panel. For the flows that start from an empty epic (Open lead, frames 14–16), create a second epic with no children.
- **The lead without a real agent.** The lead is an ordinary session that runs `claude` from `PATH`. The service
  knows who calls `otaq` by the process tree (`caller.rs`), so a command typed inside the lead's session runs **as
  the lead**. For the checks that only need the lead to run `otaq` commands, put a stand-in `claude` first on `PATH`
  before `npm run tauri dev`: an executable `#!/bin/sh` + `echo "stand-in agent"; exec sh -i`, with
  `target/debug` on `PATH` too. Then type `otaq update SHOP-40.3 --stdin`, `otaq note SHOP-40.4 --stdin`,
  `otaq answer <ID> --stdin`, `otaq ask human --pass-on <ID> --stdin`, `otaq suggest close SHOP-40.6 --stdin`,
  `otaq decision mark SHOP-40.1#1 --source PAY-123 --stdin` into the Lead tab's terminal (end stdin with Ctrl+D).
  The same stand-in in a child's session lets you run the child's `otaq ask human --stdin`,
  `otaq decision record --stdin` and `otaq review submit --stdin`. The tests' stand-in is `exec cat` (SDD §23.1);
  `sh -i` is only for these hand checks.
- **Checks that need a real `claude`** are marked **real `claude`** below: planning from the epic card (flow A), a
  real merge turn (flow C), a child recording a decision on its own, a note read at a commit, a lead answering from a
  written source.
- **Merged children** come quickest from Auto-merge "Epic children only" with a child whose stand-in session you
  submit by hand (`otaq review submit --stdin` in its terminal; validation must pass), or from the SQL of
  `review-M12-core.md` against `$OTAQ_HOME/otaq.db` (`status = 'merged'`; a merge made that way wakes nothing).
- **Gallery** (`OTAQ_GALLERY=1 OTAQ_HOME=/tmp/m17 npm run tauri dev`), the M17 sections:
  - "01 · Epics on the board · Row · Lead Status (0083) · full and short (T-0289)";
  - "… · Row · Track Label with a lead (0083) · 219 and 193 px";
  - "… · Row · Epic Card with a lead (0083)";
  - "04 · Ticket card variants · Row · Epic children & the lead · Unread notes & same file (T-0291, d0083)";
  - "05 · Ticket side panel · Epic panel · Lead tab (0083) · every state (T-0290)";
  - "07 · Decision Item (0084) · every variant (T-0287)";
  - "07 · SHOP-23 review · Decisions · SHOP-40 epic review · Decisions (T-0287)";
  - "07 · Change… on SHOP-23 · Add a fix child on 40.1 (T-0288)";
  - "10 · Needs-you inbox · Row · Epic lead items (0083) (T-0292)".

  The palette switch covers Otaq Light and Otaq Dark.
- **Window sizes:** 1440 × 900, and 1280 × 800 for *03 → Row · 1280 × 800 check (0083)*.

## 3. Hand checks

None of these is ticked by agents. Do each in Otaq Light and Otaq Dark (every row named here has a Dark twin), at
1440 × 900 unless it says 1280 × 800. Every frame is in `crewboard.pen`:
- "01 → …" means *01 · Components · Epics on the board (0080)*;
- "03 → …" means *03 · Board & columns*;
- "04 → …" means *04 · Ticket card variants · Light / Dark*;
- "05", "06", "07", "10" and "16" are *05 · Ticket side panel*, *06 · Terminal tab*, *07 · Diff, review, approve & send
  back*, *10 · Needs-you inbox* and *16 · Recovery & cleanup*.

Build from the 0083 / 0084 rows only, never rows tagged "before 0083" / "before 0084". **Lead's session** below means a
command typed in the Lead tab's terminal (stand-in `sh -i`, §2).

### Cards (the children's states, the card count)
- [ ] **T-0291** *04 → Row · Epic children & the lead (0083)*, *03 → Row · epic lead flows 3 (0083) → State · 20 · After
  the turn: one child edited, one added*:
  - In the lead's session, run `otaq update SHOP-40.3 --stdin` (a new description) and `otaq dep add SHOP-40.3
    SHOP-40.4`. The never-started 40.3 card shows the violet pill "Updated by lead" and "by Wishlist lead · description,
    now waits for SHOP-40.4".
  - Open 40.3's Ticket tab: the pill goes (seen).
  - At 1280 × 800 the pill reads "Updated" (*03 → Row · 1280 × 800 check → frame 20*).
- [ ] **T-0291** *04 → … · READY FOR DEV · NEW · ADDED BY WISHLIST LEAD*: queue SHOP-40 whole, then in the lead's
  session `otaq create --parent SHOP-40 "Wishlist empty state"`. The new card joins Ready for Dev with the violet
  outline and "new · added by Wishlist lead". Its stop reads "40.5 new · by Wishlist lead", and "new" at 1280 × 800.
- [ ] **T-0291 / T-0284** *04 → … · DEV · ASKED LEAD · WAITING* and *03 → epic lead flows 4 → State · 21*: with the lead
  live and 40.2 running (stand-in `sh -i`), run `otaq ask human --stdin` in 40.2's terminal with "Should the button
  show a filled heart once the item is saved?" on stdin.
  - The card shows the calm "Asked lead" pill and "asked lead · waiting: “…”", not turquoise.
  - The stop reads "40.2 asked lead".
  - The inbox count doesn't change.
  - The track label reads "answering SHOP-40.2".
- [ ] **T-0284** *03 → epic lead flows 4 → State · 22 · The lead answered*: in the lead's session `otaq answer <ID>
  --stdin` (the ID is in the wake-up text).
  - 40.2's terminal gets "Answer from Wishlist lead: …" as an automatic message.
  - The card is back to working, the lead idle, and the inbox still unchanged.
  - **Real `claude`:** the lead answering from a written source (the plan, PAY-123) on its own.
- [ ] **T-0291 / T-0284** *04 → … · DEV · ASKS YOU · VIA WISHLIST LEAD* and *03 → epic lead flows 5 → State · 23*: a
  second question from 40.2, passed on in the lead's session with `otaq ask human --pass-on <ID> --stdin` (recommended
  option and reason).
  - The card turns turquoise "Asks you" with "via Wishlist lead".
  - The stop turns turquoise and the inbox goes up by 1.
  - Wishlist's track moves first ("· 1 needs you").
  - The lead's own line stays idle.
- [ ] **T-0291 / T-0281** *04 → Row · Unread notes & same file (0083 · T) → DEV · 1 UNREAD NOTE FROM WISHLIST LEAD*: in
  the lead's session `otaq note SHOP-40.4 --stdin` while 40.4 runs.
  - The card says "1 unread note from Wishlist lead" (mail icon); hover it for the note, the time and "Read at the
    agent's next commit (otaq notes). Submit is refused until then."
  - `otaq notes` in 40.4's terminal prints it and the line goes.
- [ ] **T-0291 / T-0285** *04 → … · DEV · LEAD'S NOTE SENT (ROUND 0 OF 3)*, *03 → epic lead flows 5 → State · 24* and
  *flows 6 → State · 25*:
  - Let 40.3 fail validation 3 times (`otaq validate fail --stdin` from the validator's stand-in session each
    round). Nothing reaches the inbox; the lead is woken ("re-planning").
  - In the lead's session run `otaq note SHOP-40.3 --view "The agent keeps changing the API instead of the test."
    --stdin`. The card shows "Lead's note sent · validation round 0 of 3: “…”" and the stop "40.3 lead's note".
  - Let it fail 3 more rounds: turquoise "Failed 3 times" with "Lead's note sent once · hover for it". Hover shows
    LEAD'S NOTE · SENT AFTER ROUND 3 and LEAD'S VIEW. The stop is "40.3 failed".
  - Known difference: the failed stop keeps the legend's shield-x icon, where frame 25 draws octagon-alert.
- [ ] **T-0291 / T-0282** *03 → Row · same file (0083 · T) → State · 28* and *04 → … · DEV · SAME FILE AS …* / *BOTH ON ONE
  CARD*:
  - Two running children change `lib/wishlist.ts` (edit the file in each worktree from its terminal). Both cards say
    "same file as SHOP-40.x"; hover names the file and "Doesn't wake the lead".
  - With an unread note on one of them, "same file" moves to the meta row.
  - The lead stays idle.
- [ ] **T-0288** *04 → Row · Unread notes & same file → READY FOR REVIEW · “1 DECISION”*: SHOP-23 in Ready for Review with
  one recorded decision shows the muted signpost "1 decision" in its meta row. The count goes after Approve, and a
  working card never shows it.
- [ ] **Flag order** against *04 → Flag order · with the lead (0083)* and *· with update T (0083 · T)*: a card with an
  unread note and a same-file flag, and a passed-on question with an unread note. The pill and the Entry row follow
  the chain in `card.rs`.

### Epic card and strip
- [ ] **T-0289** *03 → Row · epic lead flows 1 (0083) → State · 14 · SHOP-40 in Backlog, no lead yet* and *01 → Row ·
  Epic Card with a lead (0083) → NONE · OPEN LEAD*:
  - An epic in Backlog with no lead shows the quiet "Open lead" button under the branch line, and its tooltip "Starts
    the Wishlist lead: a read-only agent that plans with you. Its tokens count on the epic."
  - Click it: the lead starts and the epic panel opens on the Lead tab (State · 15). The card reads "Wishlist lead ·
    planning", then "· idle".
- [ ] **T-0289** **real `claude`** (flow A) *03 → epic lead flows 1 → State · 15* and *State · 16 · The lead split the
  epic*:
  - Open lead on an empty epic and type "Split PAY-123 into tickets of about a day."
  - The lead runs `otaq prime --epic`, then `otaq create --parent` / `dep add` / `-p`. The rows appear live with
    "added by Wishlist lead".
  - The lead edits no file, makes no commit and starts no subagent (`git status` clean in the repo). It ends idle and
    nothing is queued.
  - With the stand-in, `otaq create --parent SHOP-40 "…"` in the lead's session shows the same "added by" rows.
- [ ] **T-0289** *03 → Row · epic lead flows 2 (0083) → State · 17 · Drag an epic that has no lead*:
  - Drag SHOP-45 to Ready for Dev. The toast reads "Queued 2 children of SHOP-45 · its lead started · Undo".
  - The track label has a second line "checking the plan" (56 px track).
  - Its tooltip: "Search lead · checking the plan", with the click hint.
- [ ] **T-0289** *03 → epic lead flows 2 → State · 18*: drag SHOP-40, whose lead is open. The toast has no "its lead
  started"; the label reads "checking the plan", then "idle".
- [ ] **T-0289 / T-0279** **real `claude`** (flow C) *03 → epic lead flows 3 → State · 19 · SHOP-40.1 merged: the lead
  re-plans*:
  - Merge SHOP-40.1 (Auto-merge). The label reads "re-planning"; hover shows "SHOP-40.1 merged", the tokens today and
    the click hint.
  - The real lead's merge turn: it reads 40.1's summary and decisions and updates never-started descriptions naming
    the source. It may run `otaq decision mark … --source …` and send at most one note. It ends idle.
  - With the stand-in only the wake-up text and the label can be checked.
- [ ] **T-0289** *03 → epic lead flows 6 → State · 26 · The lead itself asks you*:
  - In the lead's session `otaq ask human --option Never --option "After 30 days" --stdin`, with "Should the share
    link (40.4) expire? PAY-123 doesn't say." on stdin.
  - The whole Wishlist label turns turquoise ("asks you"), the header counts "· 1 needs you", and no child turns
    turquoise.
  - The Backlog epic card (if not in flight) gets a turquoise border (*01 → Epic Card with a lead → ASKS YOU*).
  - The tooltip lists the options and "Click to answer in the Lead tab".
- [ ] **T-0289** *03 → Row · 1280 × 800 check (0083) → frame 20* and *frame 23*: at 1280 × 800 the label keeps name,
  PAY-123, n/N and the lead line (no mini bar). The needs-you stop keeps "asks you".
- [ ] **T-0289** *03 → epic lead flows 7 → State · 27 · The last child merged*: when every child is done, the epic card
  in Ready for Review reads "Wishlist lead · ended · in review".
- [ ] **T-0289** Gallery: the three 01 lead sections against *01 → Row · Lead Status (0083)*, *Row · Track Label with a
  lead (0083)* (219 and 193 px) and *Row · Epic Card with a lead (0083)*. The idle tone is quiet text with a violet
  icon, as the 01 master draws it.
- [ ] **Interactions** against *03 → Note · Interactions (0083)*:
  - Hover the Lead Status for its tooltip.
  - Click it: the Lead tab opens.
  - Double-click or ↵ on the track's name: the epic panel opens; on the lead line: straight on the Lead tab.
  - The lead line isn't a drag handle.

### Epic panel and Lead tab
- [ ] **T-0290** *05 → Row · Epic panel · Lead tab (0083) → Lead tab · idle*:
  - SHOP-40 in Backlog → Open lead. The panel opens on the Lead tab; the tabs read Ticket · Lead · History.
  - ⌘⌥1–3 follow the shown order.
  - Type to the lead in its terminal.
- [ ] **T-0290** *05 → Lead tab · working (re-planning)*:
  - In the lead's session `otaq update SHOP-40.3 --stdin`. The change is listed under "Changes by the lead" ("this
    turn"); click it to open SHOP-40.3.
  - After a merge the group reads "after SHOP-40.1 merged · HH:MM" (Q65: lines say what the event says, shorter than
    the frame's).
- [ ] **T-0290** *05 → Lead tab · asks you*:
  - The lead's `ask human --option Never --option "After 30 days" --stdin` shows the turquoise question bar in the panel
    header, and the Lead tab's dot turns turquoise.
  - 1–2 pick and Send answer reaches the lead's terminal.
- [ ] **T-0290** *05 → Row · Epic panel · Lead tab · ended (0083) → Lead tab · ended · epic in review*: merge every
  child.
  - SHOP-40 in Ready for Review shows Diff · Ticket · Lead · History.
  - "Changes by the lead" reads "whole epic · N changes".
  - The terminal is read-only and the status is "Ended · epic in review", with no Resume.
- [ ] **T-0290 / T-0284** *05 → Row · Messages · the lead (0083) → Messages on the epic (SHOP-40 · Ticket tab)* and
  *Messages on SHOP-40.2 · a passed-on question waiting*:
  - A question the lead answered and one it passed on show in the epic's Ticket tab Messages ("N threads · with
    Wishlist lead") and on 40.2, **once each** (3d15efa).
  - A note shows as "Lead's note"; it doesn't say which stop it answered (§5, known gap).
- [ ] **T-0290** the panel in Otaq Dark against *05 → Row · Epic panel · Lead tab · Dark*, and its "Before the restart"
  under **Restart** below.

### Review and decisions
- [ ] **T-0287** *07 → Row · SHOP-23 review · Decisions (0084) → SHOP-23 · review with 1 recorded decision*:
  - SHOP-23 in review lists one Decision Item under the agent summary, above the validation report: question, Built,
    Other option, Why, and Change….
  - The button reads "Approve · keeps 1 decision", with Local merge / Open PR in its ▾.
  - A click on the main button opens the Approve choice (Q63).
- [ ] **T-0287** *07 → … → A review with no recorded decisions*: SHOP-18 has no Decisions section and "Approve · Local
  merge".
- [ ] **T-0287 / T-0280** *07 → Row · SHOP-40 epic review · Decisions (0084) → SHOP-40 · epic review: decisions grouped by
  child*:
  - SHOP-40's review groups the decisions by child (40.1 / 40.3 / 40.4, each "merged").
  - 40.1's item has the violet "Lead: contradicts PAY-123: …" row with its source chip. Make it with `otaq decision
    mark SHOP-40.1#1 --source PAY-123 --stdin` in the lead's session before the epic reaches review.
  - The button reads "Approve · keeps 3 decisions".
- [ ] **T-0287** Gallery "07 · Decision Item (0084) · every variant": the six variants and both lists against *07 →
  Row · Decision Item (0084)*.
- [ ] **T-0288** *07 → Change… on SHOP-23* and *After: SHOP-23 back in Dev*:
  - Change… on SHOP-23 opens "Change this decision", prefilled with "everywhere". Send back.
  - The card reads "Sent back · 1 decision changed · round 2".
  - Its terminal gets "Review feedback on a recorded decision: …".
  - History says "Changed by you → sent back".
  - With unsent text in its terminal, the unsent-text question comes first.
- [ ] **T-0288 / T-0277** *07 → Change… on 40.1's decision: add a fix child* and *State · Result: SHOP-40 is a track
  again*:
  - Change… on 40.1 in SHOP-40's review opens "Add a fix child": the violet Lead row, title "Change: …", and "Creates
    SHOP-40.7, added by you …".
  - Confirm: SHOP-40.7 is queued "added by you", and SHOP-40 is a track again (5/6).
  - The decision reads "Changed by you → SHOP-40.7".
  - The lead resumes the same conversation with no message.
- [ ] **T-0288** *05 → Row · Recorded decisions (0084) → SHOP-23 working · Ticket tab · Decisions (read-only)*: while
  SHOP-23 works, its Ticket tab lists its decisions read-only under "What and why", with the hint and no Change….
- [ ] **T-0288 / T-0277** *05 → … → SHOP-23 · History after Approve* and *History · a changed decision*:
  - Approve SHOP-18 and SHOP-23. History says "Kept by you · 1 decision" on SHOP-23 only.
  - A changed decision's History row matches *History · a changed decision*.
- [ ] **T-0286 / T-0276** **real `claude`** (a child recording a decision): give SHOP-23 a description that only
  mentions the profile form and drag it to Dev.
  - The agent runs `otaq decision record --stdin` and keeps going: no Asks you, no inbox item.
  - In Ready for Review the card says "1 decision" and the Diff lists SHOP-23#1.
  - With the stand-in, `otaq decision record --stdin` typed in SHOP-23's terminal makes the same row.

### Inbox
- [ ] **T-0292** *10 → Row · Epic lead items (0083) → Epic lead items · inbox*, passed-on question: in the lead's session
  `otaq ask human --pass-on <ID> --stdin` on 40.2's guest question.
  - The inbox goes up by 1: "asks · passed on by Wishlist lead", the lead's option preselected as "Recommended by the
    lead" with its reason.
  - 1–3 pick and ↵ sends.
  - ⌘J opens SHOP-40.2 with its terminal focused.
- [ ] **T-0292 / T-0285** second stop: 40.3 fails validation, the lead sends `otaq note SHOP-40.3 --view "…" --stdin`,
  then 40.3 fails again.
  - One row with the LEAD'S NOTE · SENT AFTER THE FIRST 3 FAILURES and LEAD'S VIEW boxes, its three usual actions,
    and "the lead isn't woken a second time".
  - The first stop never made a row.
- [ ] **T-0292 / T-0280** suggestions: `otaq suggest close SHOP-40.6 --stdin` in the lead's session (40.6 never
  started).
  - A row in "SUGGESTIONS · NO OS NOTIFICATION" titled with the lead's headline, and no OS notification.
  - Close… opens the usual close confirmation with "Suggested by Wishlist lead: “…”" (*10 → Close… · the usual
    confirmation*; the footer is today's, Q68). Confirm: 40.6 goes to Done.
  - Suggest again (or delete): Dismiss removes the row and doesn't wake the lead.
  - Suggesting a started child is refused.
- [ ] **T-0292** the lead's own question: `otaq ask human --stdin` in the lead's session.
  - A group 1 row "Wishlist lead · SHOP-40 Wishlist", with an OS notification "shop-web · Wishlist lead asks a
    question".
  - Click, ↵ and ⌘J open the epic panel on its Lead tab with the lead's terminal focused.
- [ ] **T-0292** the rows in Otaq Dark against *10 → Row · Epic lead items · Dark (0083)*, and the group order: ASKS
  YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS, then Suggestions, Interrupted, Review.

### Terminal (notes)
- [ ] **T-0281 / T-0286** *06 → Row · Unread notes (0083 · T) → SHOP-40.4 · submit refused while a note is unread*: with an
  unread note on 40.4, `otaq review submit --stdin` in its terminal is refused: "Submit refused: 1 unread note from
  Wishlist lead. Run `otaq notes`, then submit again." The user's own Submit for review (panel) is never refused.
- [ ] **T-0286 / T-0281** **real `claude`** (a note read at a commit), *06 → … → SHOP-40.4 · otaq notes before a commit*:
  the lead's note to a working 40.4 waits; at its next commit the agent runs `otaq notes` first (the Commit skill),
  applies it, and the card's line clears.
- [ ] **T-0281** typed in at idle: a note to a child whose turn has just ended (stand-in: the child is Idle) is typed
  into its terminal as one automatic message "Note from Wishlist lead: …". The child stays Working and no Idle stop or
  notification happens.

### Restart
- [ ] **T-0289 / T-0279** *16 → Row · Epic lead after a restart (0083) → State · Board after a restart · the lead ended*:
  - With the lead live and a child running, restart the service (quit and start the app).
  - The track reads "ended by restart · Resume".
  - The banner and reboot summary say "N sessions were interrupted · 1 epic lead ended". Leads are never in Resume
    selected.
  - Click Resume on the track: the same conversation continues.
- [ ] **T-0290** *16 → … → Lead tab · ended by restart · Before the restart* and *05 → Lead tab · ended by restart*: the
  Lead tab shows "Before the restart" (the last lines of the lead's conversation) with Resume, and "Resume lead" leads
  the header bar.
- [ ] **T-0284 / T-0285** a question or a first stop still with the lead at the restart is handed back before the
  reconcile: the child is Interrupted from Asks you (or from its stop), as with no lead.
- [ ] **Known gap (T-0289):** a restart that ends **only** leads (no ticket session interrupted) shows no banner; the
  track's "ended by restart · Resume" is the only sign. See §4.

### Windows (Ctrl in place of ⌘)
- [ ] The agent's run in §6 covers only the parts that start no agent. Still to do by hand in WebView2:
  - the Lead tab terminal with a live lead (Open lead starts a real `claude` in the VM);
  - Ctrl+Alt+1–3 on the epic tabs;
  - a Decisions review with Change… and Add a fix child;
  - the inbox rows with Ctrl+J.

## 4. Implementation choices (SDD §26)

- **The lead is a Project agent row with an epic (T-0278):** migration 0031 adds `project_agents.epic_id`; the name
  "<epic title> lead" is worked out, never stored. `Caller::EpicLead` comes from the live session's epic, and the
  lead's actor stays `project-agent:<id>`. `LeadStatus` is a pure function of the row, the session, its question or
  prompt, the epic's column and the turn reason. Leads are left out of every Project agent list, so they never show
  under "Terminals & agents" (`tests/epic_lead.rs`
  `leads_are_absent_from_every_project_agent_list_and_named_after_their_epic`).
- **Wake-ups and handovers (T-0279, T-0284, T-0285):**
  - A wake-up is a `LeadWake` delivery (migration 0033), held during the plan limit.
  - What was sent to the lead is a handover, kept in memory and owned by the lead turn its text was typed in.
  - At that turn's Stop, or when the lead ends, every unsettled handover goes back to its owner ("question",
    "stuck"), which routes it to the user as if no lead were live.
  - The stored rows (migrations 0035, 0037) cover a restart: those are handed back before the reconcile.
  - Close epic wakes nothing (Q62).
- **The lead's rules (T-0280):** `policy.rs` lets the lead read anything; edit, link and prioritise its epic's
  never-started children (`epic_flight::is_never_started`); create children; answer its children's questions; and run
  its own commands (note, mark, suggest). Anything else is refused with "Wishlist lead can only change SHOP-40's
  children that haven't started". The child's own History events carry the lead's name; the epic gets `lead_changed`.
- **Notes (T-0281):** `lead_notes` (migration 0036). `otaq notes` and `otaq prime` mark them read. An idle child gets
  them typed in through `Signals::on_idle` before its Stop turns Idle. Only the agent's `review submit` is refused while
  one is unread.
- **Stuck children (T-0285):** the first Validation failed / Keeps conflicting / Idle of a child with a live lead is
  held: no inbox item, the child stays Working, and the lead is woken. The lead's note counts as Answer the agent. The
  second stop's inbox item carries the note.
- **Overlaps (T-0282):** an in-memory cache on the Board of each running child's changed files (against the merge
  base with the epic branch, untracked included), refreshed by the scheduler. It wakes nothing and pauses nothing.
- **Recorded decisions (T-0276, T-0277):**
  - Stored in `decisions` (migration 0032); the ID `<KEY>#<n>` is built at read time.
  - The user's Local merge and a reported PR keep every open decision (Q61); Auto-merge keeps nothing.
  - Change… sends back with "Review feedback on a recorded decision", or creates a fix child as the user.
  - The card count `Flags.open_decisions` is on ticket cards only (Q64).
- **Views:**
  - Every wording comes from `otaq-view`: `decision_item.rs`, `decision_change.rs`, `lead_status.rs`, `lead_tab.rs`,
    the `card.rs` flag chain (Q67), and the `inbox.rs` lead items (Q68).
  - "Changes by the lead" groups the epic's `lead_changed` events by the children's merge and close times (Q65).
- **`otaq prime --epic` (T-0283)** prints the epic, the children in start order (summaries, decisions), same-file
  pairs, open questions, unread notes, suggestions, stuck children and the rules; `--json` gives `EpicPrime`. Notes
  for later tickets stay inside the submit summary (Q66). The `work-ticket` skill asks for a closing "Notes for later
  tickets:" paragraph.
- **Plugin (T-0286):** `/otaq:epic-lead` runs `otaq prime --epic` each turn and works only through `otaq`: no file
  edit, no commit, no subagents, every turn ends idle. `work-ticket` teaches `otaq decision record` and `otaq notes`;
  `commit` runs `otaq notes` first.
- **Sweep for what 0083 / 0084 replaced (T-0293):**
  - **"An epic has no agent"** was left in three places:
    - the epic review's note, "No Send back: an epic has no agent. Add or reopen a child ticket to change it."
      (`otaq-view` `epic_review.rs` and its test). It now reads "No Send back on an epic. Add or reopen a child ticket
      to change it.", as SDD EPC-08 words it. 0083 keeps "Send back on an epic still isn't available".
    - two code comments, in `otaq-core` `start/worktree.rs` and `pr/service.rs`, reworded: the epic has no worktree,
      and its lead never delivers.
  - **The lead under "Terminals & agents":** the drawer reads `project_agent.list`, whose queries keep
    `epic_id IS NULL`, so no lead is listed. The drawer's own texts name only Project agents.
  - **`otaq edit`:** none in `crates/`, `plugin/`, the gallery or the tests. `"edit"` remains only as the panel
    action and the editor's mode, not as a CLI command.
- **Known gaps, recorded here, not fixed:**
  - **A restart that ends only leads shows no banner (T-0289).** The banner's `on_connect` needs at least one
    interrupted session; the track's "ended by restart · Resume" and the reboot summary line still say it.
  - **`LeadEntry::Note.after` is unfilled** (`otaq-desktop` `api/sections.rs`). The note log doesn't record which stop
    a note answered, so Messages can't say "Lead's note after Validation failed 3 times · round counter back at 0". It
    would need the stop on the `lead_notes` row, or a join with `lead_stuck`.
  - **The stuck wake-up names `otaq note KEY --stdin` without `--view`.** That is the ticket's and the frame's
    wording; the epic-lead skill teaches `otaq note KEY --view "<cause>" --stdin`.
  - **"Changes by the lead" lines are shorter than the frame's.** They say what the event says (Q65).
  - **The orchestrator's slip about the mark's prefix:** T-0287 was told to name the lead ("Wishlist lead:
    contradicts …"), but the 07 frame draws "Lead: contradicts …". T-0288 restored "Lead:"; History keeps "Marked by
    Wishlist lead".
- **Files outside the planned lists** are named in each commit message. The most common: `service/permissions.rs`,
  `board_requests.rs`, `tests/every_request.rs`, `history.rs`, `domain/flags.rs`, `board/tickets.rs`,
  `signals/mod.rs`, `service/mod.rs`, `attention/notify.rs`, `api/gallery.rs`, `api/sections.rs`, `generated.rs`,
  `app/registry/overlays.ts`, and test literals for new fields. T-0293 touched `otaq-view` `epic_review.rs`, and
  `otaq-core` `start/worktree.rs` and `pr/service.rs` (comments only).

## 5. Open questions

From `tickets/questions.md`, every M17 question was deferred and built as option 1:
- **Q61** (T-0277): Open PR keeps the decisions when the PR is reported (`otaq pr opened`), as the user. A cancelled
  flow keeps nothing.
- **Q62** (T-0279): Close epic wakes nothing and leaves the lead as it is. Option 2 would also end it.
- **Q63** (T-0287): "Approve · keeps N decisions" opens the Approve choice (Local merge / Open PR). It doesn't go
  straight to Local merge.
- **Q64** (T-0288): "N decisions" on ticket cards only, not on the Epic Card in Ready for Review.
- **Q65** (T-0290): "Changes by the lead" works out turns from the children's merges and closes, and its lines say
  only what the event says.
- **Q66** (T-0283): a child's "notes for later tickets" are part of its submit summary, not a separate field.
- **Q67** (T-0291): only "Updated by lead" on the card; "Updated by agent" stays the editor's marker.
- **Q68** (T-0292): the close confirmation opened from a suggestion keeps today's footer, not the frame's
  never-started footer.

## 6. Platforms checked

| Machine | Script | Rust tests | Vitest | Result |
|---|---|---|---|---|
| macOS (the Mac) | `scripts/check.sh`, by the orchestrator on every landing head (§7) | all passed | all passed | green on every landing |
| macOS (the Mac) | `scripts/check.sh` mid-milestone on main 0b86ea4 (12 tickets landed) | all passed | all passed | **green** (14:00) |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` mid-milestone on main 0b86ea4 | `otaq-core` `tests/decision_outcome.rs`: SqliteError 5 "database is locked" (1 of 7 tests in the full run; 3 of 7 in each of 8 repeat runs) | passed | **red**, fixed in b50341c |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on a3a8198 (= b50341c, the lock fix) | `decision_outcome.rs` green 6 of 6 runs; `test` still red: `otaq-core` `lead_questions` and `lead_stuck`, `otaq-desktop` `api::terminal` typing test | passed | **red**, fixed in 6b126b9, 43aa1df, baac410 |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on main baac410 (by the win-fix teammate) | all passed | passed | **green**, 25.8 min |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on T-0293's commit (on baac410; same tree, only this file's §6 text differs) | 1,485 passed, 0 failed | 55 files passed | **green**, 26 min |
| Ubuntu Arm VM | — | — | — | **not checked** |

**Windows.** VMPal VM "Windows" (Windows 11 Pro 24H2, Arm64). The M16 approach: in `%USERPROFILE%\cb-src`,
`git fetch Z:/software-factory main` (Z:\ is the main checkout, so only landed commits). T-0293's own commit is sent
as a `git format-patch` file and applied in a fresh detached worktree `%USERPROFILE%\cb-m17-final`, with
`CARGO_TARGET_DIR=%USERPROFILE%\cb-target`, then `powershell -ExecutionPolicy Bypass -File scripts\check.ps1`. Nothing
runs inside `Z:\`.

- **13:51–14:34 on 0b86ea4: red.** Every step passed except `test`: `otaq-core` `tests/decision_outcome.rs` failed
  with SqliteError 5 "database is locked". It failed 1 of 7 tests in the full run and 3 of 7 in each of 8 repeat runs.
  macOS was green on the same tree. A fresh teammate (`m17/win-fix`) investigated it with the VM. The cause: a
  deferred `BEGIN` that reads and then writes (`decisions::record`: `SELECT MAX(ordinal)`, then `INSERT`) fails at
  once with `SQLITE_BUSY` when another connection holds the write lock as it upgrades, because SQLite skips the busy
  handler there. On Windows the scheduler's and the start sequence's background writes after `queue_epic`
  overlapped it on every run. b50341c makes every transaction `BEGIN IMMEDIATE` (`Store::begin_write`), so the busy
  timeout applies.
- **14:41–15:06 on a3a8198 (b50341c before its rebase): red.** `decision_outcome.rs` passed 6 of 6 runs, but
  `check.ps1` stopped at `test` again. A `cargo test --workspace --no-fail-fast` run (~30 min) found three more
  Windows-only failures, every other target green:
  - `otaq-core` `tests/lead_questions.rs`: failed 6 of 6 runs with "status.0 never written"; passes with
    `--test-threads=1`.
  - `otaq-core` `tests/lead_stuck.rs`: failed 3 of 9 runs, the same way, and the note text was run by `sh` as a
    command.
  - `otaq-desktop` `api::terminal::tests::typing_paste_and_resize_reach_the_pty_and_the_unsent_flag`: "waited for the
    new size", and the first typed character was lost.

  The same teammate fixed them on `m17/win-fix2`, landed as three commits:
  - 6b126b9: the stand-in lead's `PATH` leaves out cargo's build folders, as an agent shell's does.
  - 43aa1df: the test harness types an agent shell a short line that sources the command, and types it again if it
    never started. Git for Windows' `sh` drops typed-ahead input under parallel load.
  - baac410: the terminal typing test types `stty size` again when the resize repaint loses a key.

  These are test-harness fixes; the product code was not changed for them.
- **On main baac410: green** (`check.ps1`, 25.8 min, run by the win-fix teammate); `check.sh` on the Mac green on the
  same tree.

- **16:57–17:23 on T-0293's commit (on baac410): green**, every step, `check.ps1: all checks passed`. The tree was
  applied from a `git format-patch` file in `%USERPROFILE%\cb-m17-final` (tree hash identical to the Mac branch).
  Only this section's text changed afterwards. The Mac `check.sh` was green on the same tree too.

**Windows app check: not done.** The ticket asks to try the Lead tab terminal, Open lead and a Decisions review in
WebView2. The VM has the real `claude` installed, and each of these needs a live agent: Open lead starts a real lead,
the Lead tab terminal shows only a live or ended lead, and a ticket reaches Ready for Review with decisions only after
an agent has worked on it in Dev. So nothing was started there. They are in §3 → Windows as hand checks.

**Linux: not checked.** There is no Ubuntu VM in VMPal. By the M14 rule, a VM that can't be reached is skipped and
recorded, not a blocker. Not seen on Linux: the Lead tab terminal, the Decisions review and the inbox rows in
WebKitGTK.

## 7. How it was run (decision 0079)

As in M16, an orchestrator on `main` started each ticket as a teammate in its own worktree and branch (`m17/T-xxxx`),
at most three at once. It landed each finished branch on `main` by **fast-forward only**, after a green
`scripts/check.sh` (exit code tested) on that exact head. A branch that fell behind was rebased onto the new `main` and
checked again. Bookkeeping (`progress.md`, ticked boxes, questions) was added on the rebased branch just before its
check. Times are Mac local time, 2026-10-16.

**Start.** At 10:43 `main` was 3734ea5 and clean, with 399 GB free on disk. The user said no other session was
building. `check.sh` was not re-run at the start, because 3734ea5 had landed after a green check.

**Order and timeline**

| Time | What | Why / notes |
|---|---|---|
| 10:43 | **Wave 0:** T-0278 ∥ T-0276 ∥ T-0282 | different modules (`project_agents/` + caller + launch; `decisions/` + prime + `cli decision`; `epic_lead/overlap.rs`); shared only registration files; migrations numbered at landing |
| 10:58 | T-0282 6691d89 landed (no rebase; check ~6 min) | touched `board/mod.rs`, `board/tickets.rs`, `scheduler/mod.rs`, `signals/mod.rs` outside its list |
| 11:50 | T-0276's check on f5caba8 **red** (`tests/store.rs` table list missing `decisions`) | the teammate was idle and didn't notice; see lessons |
| 11:50 | T-0278 8d11da1 landed (rebased once onto 6691d89 by the teammate first) | migration 0031_epic_leads |
| 11:51 | T-0279 started alone in phase 1 | T-0277 waits on T-0276's re-check |
| 11:58 | T-0276 17311b9 landed (rebased onto 8d11da1, migration 0031 → 0032, store tests fixed; check 293 s) | a second request `decision.list` added (outside its list) |
| 11:58 | Phase 1/2: T-0279 ∥ T-0280 ∥ T-0277 | T-0277 and T-0280 both touch `src/decisions/`: each got its own file (`outcome.rs` / `mark.rs`), one `mod` line each; T-0279 vs T-0277 one call each in `review/mod.rs`; T-0280 owns `policy.rs` / `permissions.rs` |
| 12:17 | T-0277 dd6b62e landed (no rebase; check ~5 min; Q61) | touched `card.rs` (the Sent back text), `pr/service.rs`, `flags.rs`, test literals outside its list |
| 12:18 | T-0287 started (∥ T-0279, T-0280) | review view models, `DiffTab`, desktop `api/review.rs`; no core files shared |
| 12:22–12:29 | T-0279 green on c7a1f7c but based on 17311b9; sent back to rebase (Q61 → Q62); landed as 7a112ad (check ~5.5 min) | migration 0033_lead_wake_deliveries; Q62 |
| 12:30 | T-0289 started (∥ T-0280, T-0287) | epic card / strip / `after_restart` / Lead Status: different view-model and React files from T-0287; shared only Gallery / generated / `mod.rs` lines |
| 12:30 | the fix-child resume test was missing from T-0279 | its worktree was removed before the teammate added it; carried into T-0284's brief |
| 12:38 | T-0280 36a6879 landed (check ~5.5 min) | migration 0034_lead_suggestions |
| 12:39 | T-0284 started (∥ T-0287, T-0289) | the long chain first; T-0281 waits for a slot |
| 12:41–12:51 | T-0287 green on 29a4ebd but based on 7a112ad; sent back to rebase (and to name the lead on the mark, see the slip below); two red re-checks (stale generated types; clippy redundant clone); landed as d25d294 | Q63 |
| 12:51 | T-0281 started (∥ T-0284, T-0289) | T-0281 / T-0284 share only registration-style lines (`flags.rs`, `protocol.rs`, `deliveries.rs`); chosen over T-0288 because it unblocks T-0283 / T-0285 / T-0286 |
| 13:01 | T-0289 4e88c44 landed (check ~4.5 min) | gap carried to T-0293: a leads-only restart shows no banner |
| 13:02 | T-0290 started (∥ T-0281, T-0284) | chosen over T-0288 because T-0292 waits on it |
| 13:14 | T-0284 f200807 landed (check ~5.5 min) | migration 0035_lead_questions; includes the carried fix-child resume test |
| 13:14 | T-0288 started (∥ T-0281, T-0290) | review dialogs, Ticket-tab Decisions, the card count; one `card.rs` meta-row entry while T-0291 wasn't running |
| 13:27 | T-0281 a3de219 landed (rebased onto f200807, migration 0035 → 0036; one red re-check on `naming_literals`) | |
| 13:28 | T-0285 started (∥ T-0288, T-0290) | the long chain; T-0283 waits for a slot |
| 13:41 | T-0288 7a76473 landed (check ~5.5 min; Q64) | T-0290 meanwhile red twice (Node 20 on `PATH`; `naming_literals`) |
| 13:42 | T-0283 started (∥ T-0285, T-0290) | `epic_lead/prime.rs` + `otaq-cli` `prime.rs`; different core files from T-0285 |
| 13:50 | T-0290 0b86ea4 landed (rebased onto 7a76473; Q65) | an amend after the check changed only the message; the tree was verified identical with `git diff` |
| 13:51 | **Mid-milestone checks** on main 0b86ea4 (12 tickets landed, none ready until T-0285): macOS `check.sh` in a detached worktree, Windows `check.ps1` in the VM | the M16 lesson: Windows is a check of its own |
| 13:51 | T-0290 follow-up `m17/T-0290-fix` (duplicate Messages threads) given to the same teammate in a fresh worktree | beside T-0283 / T-0285, different files |
| 14:00 | macOS `check.sh` on 0b86ea4: **green** | |
| 14:03 | T-0290 fix 3d15efa landed (check ~8 min) | |
| 14:10 | T-0285 f144444 landed (rebased onto 3d15efa; one red re-check on `naming_literals`) | migration 0037_lead_stuck; T-0283 was also green on 3d15efa but landed second, since T-0285 unblocks T-0291 / T-0292 / T-0286 |
| 14:11 | T-0291 ∥ T-0292 started (∥ T-0283's re-check) | `card.rs` / `card_page` / strip stops / `TicketCard` vs `inbox*.rs` / `Inbox.tsx`: different files |
| 14:18 | T-0283 2aa0ab7 landed (rebased onto f144444; Q66) | |
| 14:19 | T-0286 started (∥ T-0291, T-0292) | plugin skills and their tests only |
| 14:31 | T-0286 6eaf9da landed (check ~6 min) | |
| 14:34 | Windows `check.ps1` on 0b86ea4: **red** (`decision_outcome.rs` "database is locked") | macOS green on the same tree; a fresh teammate on `m17/win-fix` (from 6eaf9da) investigates with the VM |
| 14:44 | T-0291 99461bb landed (check ~7 min; Q67) | T-0292, also using Q67, must rebase and renumber |
| 14:54 | T-0292 958c189 landed (rebased twice, onto 6eaf9da then 99461bb; Q67 → Q68; one red re-check on `naming_literals`; check ~7 min) | |
| 14:54 | **T-0293** started (∥ win-fix) | this review and the sweep first; its Windows run waits for the win-fix landing |
| 15:51 | T-0277 fix b50341c landed (rebased onto 958c189; Mac check ~6 min) | every transaction begins IMMEDIATE; on Windows `decision_outcome` then passed 6/6, but `check.ps1` stayed red on three failures that had been hidden behind it |
| 15:53 | the same teammate on `m17/win-fix2` (from b50341c); T-0293 holds its Windows run | `lead_questions`, `lead_stuck`, the desktop terminal typing test |
| 16:56 | 6b126b9 T-0285 fix, 43aa1df T-0284 fix, baac410 T-0293 (terminal test) landed | Windows `check.ps1` on baac410 **green** (25.8 min), Mac green; none was a product bug: Git for Windows' `sh` drops typed-ahead input under parallel load, and cargo's `target\debug` on `PATH` let the stand-in lead run the real `otaq` |
| 17:28 | **T-0293** 29c57ef landed (rebased onto baac410; Mac check 209 s) | Windows `check.ps1` green on the same code tree (26 min); 29c57ef differs from it only in Markdown |
| 17:30 | the orchestrator's last bookkeeping commit (these rows), `check.sh` on it | the final check on `main` |

The 17 tickets and the T-0290 fix landed in about 4 h 10 min (10:43–14:54). Each `check.sh` took 4.5–8 minutes with
the shared build cache.

**Rebases and re-checks:**
- Rebased before landing: T-0278, T-0276, T-0279, T-0287, T-0281, T-0290, T-0285 and T-0283 once each, T-0292 twice.
- Red re-checks: T-0276 (store table list), T-0287 twice (stale generated types, clippy), T-0290 twice (Node 20,
  `naming_literals`), and T-0281, T-0285 and T-0292 once each (`naming_literals`).
- Every head that landed had its own green `check.sh`.

**What collided, and the lessons:**
- **Migrations numbered at landing:** three renames on rebase (T-0276 0031 → 0032, T-0281 0035 → 0036, and the store
  test's table list). Naming a migration by its placeholder until landing worked; the store and migration tests list
  every table, so a new table also edits `tests/store.rs`.
- **Question numbers again:** T-0279 (Q61 → Q62) and T-0292 (Q67 → Q68) renumbered on rebase. Question numbers come
  from `main` at landing time.
- **`naming_literals`:** a test forbids the literal "otaq" outside `naming.rs`, and four teammates tripped on it
  (T-0281, T-0290, T-0285, T-0292). The brief warned about it, so grep the diff before the check.
- **An idle teammate isn't woken by its background check:** T-0276's red check went unnoticed until the orchestrator
  read the log. The orchestrator now polls each teammate's check log, and the briefs tell teammates to wait on the log
  (Monitor or polling) instead of ending their turn.
- **Node on `PATH`:** a check run with the system Node 20 failed `node`; nvm's Node 24 must come first.
- **A worktree removed too early:** T-0279's last test (a fix child resumes the lead) never reached its commit; it was
  carried into T-0284.
- **The orchestrator's slip:** T-0287 was told to name the lead on the decision mark ("Wishlist lead: contradicts …"),
  but the 07 frame draws "Lead: contradicts …". The correction went into T-0288's brief, which restored the frame's
  wording.
- **Mid-milestone Windows check (the M16 lesson, applied):** it found a Windows-only failure (`decision_outcome.rs`,
  SQLite "database is locked") while the remaining tickets were still building, instead of at the end.
- **Windows failures hide behind each other:** `check.ps1` stops at the first failing step, so the first fix only
  revealed three more. Running `cargo test --workspace --no-fail-fast` in the VM listed them all at once.
- **Splitting a shared folder into files:** T-0277 and T-0280 both needed `src/decisions/`; giving each its own file
  (`outcome.rs` / `mark.rs`) let them run side by side with one `mod` line each.
