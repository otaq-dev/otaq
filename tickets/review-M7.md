# M7 · Attention & recovery — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0110 | Focused-terminal rule (core) | d439543 |
| T-0111 | Startup reconcile and Interrupted | f51ffb5 |
| T-0112 | Resume, Resume all, Resume failed, Start from scratch | 8a9ac33 |
| T-0113 | "Before the restart" reader | 899d781 |
| T-0114 | Inbox answers: type into the session; resume and deliver for Interrupted | 390933d |
| T-0115 | Inbox events, counts and resolution | ba903cf |
| T-0116 | Cleanup scan and delete (core) | faa3699 |
| T-0117 | OS notifications (macOS) and setting | d9a4ce4 |
| T-0118 | Inbox surface | 1f3b053 |
| T-0119 | Next-item hotkey (⌘J) | a4337d6 |
| T-0120 | Inline answer in the inbox | 624922c |
| T-0121 | Interrupted, Before the restart and Resume failed UI | 89553fc |
| T-0122 | After-reboot summary and "reconnected" | d49c6b0 |
| T-0123 | Cleanup view | cde7ff4 |
| T-0124 | "Your turn" hint while the terminal has focus | a739c7a |
| T-0165 | Answer options in the inbox | 2b9f96e |
| T-0166 | Answer options on the card and in the panel | cdfb7ff |

No test flaked in this milestone.

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)        # a short path: the socket lives in it
mkdir -p $CREWBOARD_HOME/repo && (cd $CREWBOARD_HOME/repo && git init -q -b main && echo a > a.txt && git add . && git commit -qm init)
B=$PWD/target/debug/crewboard
PATH=/usr/bin:/bin $B daemon start
echo '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"'$CREWBOARD_HOME/repo'"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock
for t in "Order confirmation email" "Upgrade to Next.js 15" "Split user full name" "Checkout API"; do $B -P REPO create "$t" --done-when works; done
```

Sample states without claude (the service only reads them; the app refetches on the next event, a project switch or a
restart of the app):

```sh
sqlite3 $CREWBOARD_HOME/crewboard.db "
  UPDATE tickets SET state='asks_you', board_column='dev', signal_text='Which email provider?' WHERE number=1;
  UPDATE tickets SET state='interrupted', board_column='dev' WHERE number=2;
  UPDATE tickets SET state='interrupted', board_column='dev', interrupted_from='asks_you', signal_text='Split everywhere?' WHERE number=3;
  UPDATE tickets SET state='ready_for_review', board_column='ready_for_review' WHERE number=4;
  INSERT INTO human_questions (id, ticket_id, question, options_json, recommended, asked_at)
    SELECT 'q1', id, 'Which email provider?', '[\"Postmark\",\"SES\",\"Resend\"]', 1, 1 FROM tickets WHERE number=1;"
PATH=/usr/bin:/bin target/debug/crewboard-app
```

- **Inbox**: the header's "Needs you 4" button or ⇧⌘I; groups, rows, Answer (REPO-1 with options, REPO-3 "Answer &
  resume"), Resume all (1) with its confirmation; ⌘J steps through the items ("Inbox · 1 of 4").
- **Cards**: REPO-1 lists its options with "Send answer"; REPO-2 shows Resume; REPO-3 keeps "Asks you" with "Session
  ended by restart · answering resumes it".
- **Panel**: REPO-2's Terminal tab shows "Before the restart" (here "conversation no longer available", since no Claude
  conversation exists) with Retry and Start from scratch; REPO-1's panel shows the option list above the tabs.
- **Restart**: with a ticket set to `working`, `$B daemon stop && $B daemon start` (or the app's Restart now) → the
  ticket is Interrupted, the app shows "Service restarted · 1 session was interrupted by the restart…" with Resume all.
  `kill -9` the service's PID instead → "1 session was interrupted · … after a reboot or crash".
- **Cleanup**: `git -C $CREWBOARD_HOME/repo branch crewboard/OLD-1` → the rail's broom gets a dot within a minute
  (or at the next app start); ⌘K "cleanup" ↵, Settings → Data → Cleanup… or the broom open the window.
- **Focus**: needs a live session (the real claude, or an agent shell from the test harness): with the panel's terminal
  focused, an `ask human` keeps the card calm with "your turn"; leaving the terminal turns it orange.

**With the real claude (once):** a resume of a deleted conversation (Resume failed: Retry, Start from scratch); "Before
the restart" from a real transcript; answering a question with options from the inbox, the card and the panel; an
answer to an interrupted question (`claude --resume <id> "<answer>"`).

**Cleaning up:** `$B daemon stop`, then remove `$CREWBOARD_HOME` (the repo got only `crewboard/*` branches; `git
worktree prune` in it afterwards).

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0112 · Resume failed wording**
- [ ] With the installed claude, `claude --resume <unknown id>` prints "No conversation found with session ID: …" and
  exits 1 (the binary of 2.1.296 contains this string; the service matches "no conversation found" in any case).

**T-0113 · Transcript location and format**
- [ ] For a real agent session, `~/.claude/projects/<worktree path with non-alphanumerics → '-'>/<session id>.jsonl`
  exists and "Before the restart" lists its user / assistant messages and tool actions in order.

**T-0117 · OS notifications**
- [ ] `11 · Banners, toasts & OS notifications → OS notifications` (macOS column): a permission prompt, an `ask human`
  question and an interrupted session each post one notification ("shop-web · SHOP-22 needs permission" / "Allow rm -rf
  .next? — Fix flaky login redirect", …); none for Ready for Review or while the ticket's terminal has focus; none
  again for the same item. More than three new items of one project at once give one "N agents need you".
- [ ] `15 · Settings → Global · Usage, notifications, data, appearance`: the Notifications section ("OS notifications
  for blocking items", toggle on by default); turning it off stops them at once.
- [ ] The click: today it opens Script Editor (osascript), not the ticket (Q22).

**T-0118 · Inbox surface**
- [ ] `10 · Needs-you inbox → Inbox popover · mixed, 2 projects`, `In context · 1440 window`, `One item`, `Empty ·
  nothing needs you`, `Resolved elsewhere`, `Inbox count in header & switcher`, `Dark · inbox`, `Resume all · confirm`,
  `Parent closed · new item in group 2`: the 440 px popover under the header button, the head ("Needs you 7 · all
  projects", Next ⌘J), group headings with counts, rows (icon tile, project chip, key, title, reason, age, "blocks 2"),
  Resume all (N) and its confirmation, the Parent closed buttons, the empty state, a resolved row struck through for 4 s,
  the foot hints. Header count = sum of the rail badges. Known differences: review rows say only "ready for review" (no
  "+412 −38 · 9 files"); the interrupted question sits in group 1 (as the frame *After a reboot* shows).

**T-0119 · ⌘J**
- [ ] `10 → 1 · ⌘J from anywhere`, `2 · Answered`, `3 · Next item`, `4 · All clear`: ⌘J from the board and from a
  focused terminal; the run bar on top of the panel ("Inbox · 1 of 7", segments, Next ⌘J, Exit; "Done · 6 left · next:
  ⌘J" after answering); no auto-advance; "All clear" over the board for 3 s, then the board has the keyboard.

**T-0120 / T-0165 · Answering in the inbox**
- [ ] `10 → Answer · inline`, `Answer · options, no recommendation`, `Idle · third orange label (0042)`, `After a reboot ·
  answer resumes (0045)`, `Dark · answer with options`: Answer opens the box in the row; the options list (numbered,
  Recommended badge, preselected; Other… as the last number with a field), Send disabled without a pick; "Answer &
  resume" for the interrupted question; the unsent-text prompt when the terminal holds typed text. Known differences: no
  option descriptions (labels only, d0061); no "Logged in … Messages" line (M10).

**T-0121 · Recovery UI**
- [ ] `06 · Terminal tab → Interrupted · Before the restart` (both variants), `16 · Recovery & cleanup → Resume failed ·
  Start from scratch`, `History · Claude session IDs (0045)`; `04 → Working · Interrupted`, `Dev · interrupted, keeps
  question` (light/dark). Known differences: no commit or uncommitted-file counts in the Start from scratch dialog.

**T-0122 · After a reboot**
- [ ] `16 → Opening after a reboot` and `11 → Banners → After the service restart`, `Background service stopped`: the red
  banner with the count, Resume all and Review & resume, dismissable; a normal reopen shows only "Reconnected". Known
  differences: no summary sheet with per-ticket checkboxes; while the service is down the stopped banner keeps Start
  service (no counts are known then).

**T-0123 · Cleanup**
- [ ] `16 → Cleanup · 3 items`, `Cleanup · 20 items`, `Cleanup · empty`, `Delete · confirm`, `Success`, `Dark · cleanup`,
  `Where Cleanup lives`: groups with checkboxes, path / why / size / age, nothing ticked (Q24), "N selected · 412 MB",
  the confirmation naming each item, the toast, the broom with its dot on the rail (and in the 1920 sidebar), Settings →
  Data → Cleanup…, ⌘K "cleanup".

**T-0124 · Your turn**
- [ ] `06 → 1 · Focused, ended turn · stays calm (SHOP-19)`, `2 · Left without replying → Idle (SHOP-19)`; `04 → Dev ·
  focused, ended turn (stays calm)` (light/dark): the strip's "your turn", the calm card with "your turn · in terminal";
  closing the panel, switching tab, clicking outside or ⇧⌘` turns it orange at once.

**T-0166 · Options on the card and in the panel**
- [ ] `04 → Row · answer options (0061)` (Asks you · with options, · 3 picked; light/dark), `05 → SHOP-21 · Asks you ·
  answer options`, `Dark · options & delete`, `06 → Answering a question (SHOP-23) · focused`: the compact list on the
  card with "Rec." and Send answer; a click on an option opens the panel with it picked; the panel's list (1–4, ↵,
  Other…); with the terminal focused, digits go to Claude Code.

Earlier milestones' hand checks stay as listed in review-M1…M6.

## 4. Implementation choices (SDD §26)

- **Focus** (`attention::Focus`): per client connection and session, shared by the board and the sessions; a ticket
  is focused while any client holds focus on one of its running sessions; a closed connection or an ended session lets
  go. The stored status stays the agent's real one; `Flags.your_turn` marks a waiting ticket whose terminal has focus,
  and `Ticket::shows_orange` / `needs_user`, the Dev order, the needs-you items (so the inbox, back-pressure and
  notifications) and the project counts follow it. `crewboard attach` reports focus after attaching. In the app one
  pure rule (`TerminalFocus`) says what the service is told: tab switch, panel close, blur and ⇧⌘` are "lost"; a
  reconnect re-reports.
- **Reconcile** (`recovery`): `reconcile(status, session_alive)` is REC-01's table; every service start runs it after
  the leftover sessions are recorded and starts cut short are Start interrupted. Interrupted orange tickets keep their
  status in `tickets.interrupted_from` (migration 0012), their text and their question. `service_runs` (migration
  0012) records each start and its clean stop; no clean stop = an unclean start. `service.status` gained
  `unclean_start` and `interrupted_at_start`. Following d0018 ("`claude` exited" → Interrupted), an agent that exits on
  its own while at work interrupts its ticket too. Run / Shell / Project terminal sessions end "stopped by restart";
  `Recovery::on_stopped_by_restart` is the hook point for M9.
- **Resume** (`review::resume`): Resume, Retry and Start from scratch go through the start sequence like a parked
  ticket, with `tickets.return_mode` (migration 0013: resume / scratch); the agent limit queues them. Resume failed =
  the session's output contains "No conversation found" when it ends; any other end is Interrupted. Start from scratch
  is also allowed for an Interrupted ticket (the "conversation no longer available" view offers it). Resume all = every
  Interrupted ticket, in key order (Q23).
- **Before the restart** (`recovery::transcript`): read-only from `<CLAUDE_CONFIG_DIR or ~/.claude>/projects/<folder>/
  <id>.jsonl`, with a search of every project folder as fallback; last 40 entries; unknown records skipped.
- **Inbox** (`attention::Inbox`): `inbox.list` / `inbox.counts`; `inbox_changed` only when the derived set changes (50
  ms settle, ages ignored). `inbox.answer {key, choice, clear}`: typed through the click path; an interrupted question is
  resumed with the answer as the conversation's first prompt (`tickets.resume_prompt`, migration 0014); a sent answer
  clears orange (`Input::Answered`) and records the question's answer. `attention::LimitStops` is the
  `suppress_limit_stopped` switch with marks for M8. `ProjectInfo.needs_you` is now the project's inbox count (Review
  included), so the switcher and the header agree. An interrupted question is listed with the orange items.
- **Cleanup** (`recovery::cleanup`): scan only reads; delete takes the confirmed items, checks each against a fresh
  scan, removes worktrees through their repository (`git worktree remove --force`, then the folder if needed, then
  `git worktree prune`), deletes only `crewboard/*` branches no ticket names, frees ports, and clears
  `worktree_kept`. Nothing in the service calls delete on its own.
- **Notifications** (`attention::notify`): once per (ticket, status, since); items seen while off stay quiet;
  more than three new items of a project are grouped; the daemon posts with `osascript` (tests and other platforms post
  nothing).
- **App**: the inbox popover (`inbox.rs`), the ⌘J run (`view_models::inbox_run`), the answer box and options
  (`view_models::answer`, `view_models::answer_options`, `components::answer_options`), the recovery view and Start from
  scratch dialog in the panel (`view_models::recovery`), the after-restart banner (`view_models::after_restart`), the
  Cleanup window (`cleanup.rs`, `view_models::cleanup`), the shared unsent-text dialog (`components::unsent`). New
  bindings: ⇧⌘I, ⌘J (d0058). New request kinds have their permission rows (writes user-only, reads allowed).
- **New protocol**: `ticket.resume`, `ticket.retry_resume`, `ticket.start_from_scratch`, `tickets.resume_all`,
  `session.before_restart`, `inbox.answer`, `inbox.list`, `inbox.counts`, `cleanup.scan`, `cleanup.delete`; event
  `inbox_changed`; History events `interrupted`, `session_ended`, `resumed`, `resume_failed`, `started_from_scratch`.
- **No new crates.**

## 5. Open questions

Added to `tickets/questions.md`, all deferred:
- **Q22** — clicking a macOS notification: without an app bundle the `osascript` notification opens Script Editor, not
  the ticket. Built as option 1 (post now, click later; one `Notifier` to replace). Tied to d0062's open item on how the
  personal build is installed.
- **Q23** — is an interrupted question part of Resume all? d0045 says yes ("covers only the Dev tickets above"); the
  frames say no. Built as option 1 (the decision); recommendation option 2.
- **Q24** — are Cleanup items preselected? The ticket and d0018 ("the user selects them") say no; the frame pre-ticks
  everything but kept worktrees. Built as option 1 (nothing ticked).

Notes for later milestones:
1. **M8 plan limit** (T-0128, T-0129): turn `Inbox::set_suppress_limit_stopped(true)` on while the banner is up and
   mark stopped agents with `Inbox::mark_limit_stopped`; back-pressure, counts and notifications follow
   (`needs_you_items_with`). A limit-stopped agent's exit currently becomes Interrupted (d0018 rule) unless its status
   is set to Waiting for limit reset first.
2. **M9 Run / Shell** (T-0133, T-0137): register `Recovery::on_stopped_by_restart` (a late hook is replayed with the
   last startup's sessions); their sessions already end "stopped by restart".
3. **M10 agent questions** (T-0153, T-0154): agent-to-agent items join group 1; `AnswerKind::of` and the options apply
   only to `ask human` questions; the inbox answer request is `inbox.answer`.
4. **M10 Send back resume**: a failed resume of a Send back loses its skill on Retry (`resume_skill` is cleared when the
   start runs).
5. **Installation** (d0062 open item): an app bundle would make notification clicks open the ticket (Q22).
