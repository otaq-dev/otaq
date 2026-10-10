# M5 · Scheduler — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0077 | Readiness (beads-style) and waiting reasons | ce00627 |
| T-0078 | Needs-you item derivation (shared by back-pressure and inbox) | d593a74 |
| T-0079 | Scheduler loop (replaces the M3 temporary trigger) | d85cade |
| T-0080 | Optional limits: max_concurrency and back-pressure | 18eeffa |
| T-0081 | Fetch the base after a parent merges, then start ready tickets | 879cb46 |
| T-0082 | Agent limits: settings section and header control | 787fdd4 |
| T-0083 | Queued cards: waits-for text, waiting for a slot, held, start toast | 16c9f84 |

## 2. How to try it

**From M5 on, moving a ticket to Dev only queues it; the scheduler starts it once every dependency is merged.** With
the service started normally that launches a real `claude`; use the "no claude" variant to see the scheduler alone.

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)        # a short path: the socket lives in it
```

**Without claude (scheduler, readiness, limits, cards).** A PATH with git but no `claude`; starts stop at the launch
step (Start interrupted), which is enough to watch the scheduler pick tickets:

```sh
mkdir -p $CREWBOARD_HOME/repo && (cd $CREWBOARD_HOME/repo && git init -q -b main && git commit -q --allow-empty -m init)
PATH=/usr/bin:/bin target/debug/crewboard daemon start
echo '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"'$CREWBOARD_HOME/repo'"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock
B=target/debug/crewboard
$B -P REPO create "Checkout API" --done-when "works"       # REPO-1
$B -P REPO create "Price table" --done-when "works"        # REPO-2
$B -P REPO create "Checkout page" --done-when "works"      # REPO-3
$B dep add REPO-3 REPO-1 ; $B dep add REPO-3 REPO-2
PATH=/usr/bin:/bin target/debug/crewboard-app
```

In the app, drag REPO-1, REPO-2 and REPO-3 to Dev: REPO-1 and REPO-2 start at once (and stop at the launch step:
Start interrupted), REPO-3 stays Queued with "waits for REPO-1, REPO-2 to merge". `$B -P REPO blocked` prints the same
text. Nothing in M5 merges a ticket (Local merge is M6, Mark merged M10); to move a parent by hand, set its status
and wake the scheduler with any change (or wait for its 30 s tick):

```sh
sqlite3 $CREWBOARD_HOME/crewboard.db "UPDATE tickets SET state='ready_for_review', board_column='ready_for_review' WHERE number=1"
sqlite3 $CREWBOARD_HOME/crewboard.db "UPDATE tickets SET state='merged', board_column='done' WHERE number IN (1,2)"
```

then switch projects (or restart the app) so the board refetches. With REPO-1 in Review the card turns into the
"waits for merge" callout; with both merged the scheduler starts REPO-3 at its next pass ("Starting…", then Start
interrupted without claude). The "started · all dependencies merged" toast needs a start that reaches Agent Working,
so it shows only with the real claude.

**Limits:** Settings → Agent limits → Limit agents on; the header becomes "● N / 5 agents ⌄" and its popover edits the
same two values. With max 1 and one ticket started, a second ready ticket shows "Waiting for a slot"; raising the max
starts it at once.

**Cleaning up:** `target/debug/crewboard daemon stop`, then remove `$CREWBOARD_HOME` (worktrees live inside it; the
repo only got branches `crewboard/<KEY>`; `git worktree prune` in it afterwards).

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0082 · Agent limits**
- [ ] `15 · Settings → Global · Agent limits OFF (default)`: the section is first in Settings; the toggle is off; the
  two steppers (5, 3) are visible but dimmed and can't be changed; the descriptions read "Suggested 5. Queued tickets
  wait for a free slot." and "Suggested 3 items. Running agents are never stopped."
- [ ] `15 · Settings → Global · Agent limits ON`: turning it on enables the steppers with 5 and 3, the descriptions
  change, and the "Right now: N running · N queued tickets waiting for a slot · inbox N (new starts paused)" line shows.
  Known difference: the section uses the Settings page's own row layout (label column, control after it), not the
  frame's right-aligned controls, and the nav lists sections without icons (as built in M2).
- [ ] `02 · App shell & project switcher → Header states → Header · running chip tooltip`: with limits off the header
  shows "● 3 running" (the dot lit while agents run) with the tooltip "Agents running across all projects / shop-web 2
  · billing-api 1 · …".
- [ ] `02 → Header · agent limits ON + inline popover`: with limits on the chip reads "● 3 / 5 agents ⌄"; a click opens
  the 300 px popover under it (Agent limits, two steppers, the note); − / + save at once (Settings shows the new value);
  a click outside closes it. Check that the popover draws above the board.

**T-0083 · Queued cards**
- [ ] `04 · Ticket card variants · Light` and `· Dark` → **Queued · waiting on deps** (SHOP-21: hourglass "waits for
  SHOP-20 to merge" in the meta row, no pill), **Queued · waits for merge** (SHOP-20: "Queued" pill, hourglass callout,
  "SHOP-18 SHOP-19" chip), **Queued · ready to start** ("Starting…" pill, "4 criteria"), **Queued · held: base
  diverged** (BILL-6: the diverged message and Retry; Retry runs Retry start and nothing retries without it). The
  gallery (`CREWBOARD_GALLERY=1`) has all four. Known differences: the text keeps the ticket's parentheses, "(SHOP-18 in
  Review)", where the frame has "· SHOP-18 in Review" (Q19); the held card's pill reads "Held" where the frame shows
  "Queued".
- [ ] `12 · Stack & dependencies → 2 · The chain runs`, `3 · One parent merged`, `4 · All parents merged`: the
  waiting line updates as each parent moves ("waits for SHOP-19 to merge (in Review)"); when the last parent is merged
  the child starts and the toast "SHOP-20 started · all dependencies merged / Checkout page branched from main @
  3b7d0e1" appears (play icon). Known differences: in storyboard 4, SHOP-21 (parent not started) shows the meta-row
  line of the 04 variant, not a callout; the started card has no "from main @ sha · all dependencies merged" callout
  (the toast says it); the toast shows only for the project on screen.
- [ ] `05 · Ticket side panel → SHOP-20 · Queued, waits for merge`: the notice under the header shows the card's text as
  a sentence ("Waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)."), not the frame's longer explanation; Stack
  list lines of Queued tickets show the same text under their title.

**With the real claude (once):** drag two tickets with a dependency to Dev; when the parent's agent submits and you
set it merged (above), the child starts by itself and the toast appears. Earlier milestones' hand checks stay as listed
in review-M1…M4.

## 4. Implementation choices (SDD §26)

- **Readiness** (`board::readiness`): pure `readiness(ticket, tickets) -> Ready | Waiting { parents } | ParentClosed {
  parent }` with `reason()` as the one waiting text (cards, panel, Stack list, `crewboard blocked`). A Merged parent
  counts only when its base equals the child's (DEP-02 makes this always true); an epic whose children all ended Closed
  counts as closed before the epic itself moves; the first closed parent in key order is named. Epic parents read
  "waits for epic SHOP-40 to be merged (1 / 3 merged)" (the 08 editor note). `dep.readiness` rows gained
  `parent_closed` and `reason` (additive JSON).
- **Needs-you items** (`board::needs_you`): ATT-06 groups, blocks-more first (the board's `order::blocked_counts`),
  then oldest, then key; reasons in brief 10's wording. Back-pressure counts every item of all three groups. Age = the
  orange signal's start, else the time the ticket entered its status: **migration 0005** `tickets.state_since`, kept
  by every status UPDATE through one SQL fragment (`set_state_since!`; no trigger, so the SQL stays portable).
- **Scheduler** (`scheduler`): a pass per burst of ticket/project events plus a **30 s tick**; one pass at a time;
  ready tickets start oldest first by **`queued_at` (migration 0006)**, set by the Backlog → Dev drag (Retry start and
  sub-state moves keep it). It only moves tickets among Queued / Waiting for a slot / Parent closed (Parent closed moves
  get a History line), never touches Backlog, Held or Start interrupted. Scheduler starts are recorded as the service's
  OS user. `Starter::start` claims a ticket before reading its status, so two callers can't start it twice.
- **Starts cut short:** the start sequence marks the step it is at (`start_problem_step` with no message, hidden from
  `Ticket.start_problem` unless Held / Start interrupted); at service start, a ticket still waiting with a marked step
  becomes Start interrupted ("the background service stopped during this step") instead of being re-run by the
  scheduler (SES-02).
- **Limits:** the existing settings `agent_limits.enabled / max_concurrency / back_pressure` (T-0028; off / 5 / 3) are
  the ticket's `limits.*`. Slots are held by Working, Working in background, the orange family and Waiting for limit
  reset, plus starts that are running; Review, Approved, Parked, Interrupted and Resume failed don't hold one. A stored 0
  (flagged in Settings) acts as 1. `settings.set` wakes the scheduler. Steppers accept 1–50.
- **Parent merged** (`Scheduler::on_parent_merged`): syncs the parent's base branch (the epic branch is skipped), then
  a pass; a diverged base or failed fetch **holds** the children (Held, step "sync base", the message) until Retry
  start. Nothing calls it yet: Local merge (M6) and Mark merged (M10) must call it after setting the parent Merged.
- **New protocol:** `scheduler.status` (Reply `SchedulerStatus`: limits, slots held, waiting for a slot, needs-you count,
  starts paused). `EventBus::listen` gives in-service listeners that `subscriber_count` (connections) doesn't count.
- **App:** `components::stepper::Stepper`, `Tooltip::with_detail`, `view_models::limits`, `view_models::queue`;
  `StackRow.waits` in core. The header's "3 / 5" counts slots held; the plain chip counts running agents (the rail's
  numbers). The limits popover closes on a click outside it.
- **No new crates.**

## 5. Open questions

**Q19 added** (`tickets/questions.md`): the waiting text's format. Decision 0049 and the ticket use parentheses ("waits
for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"); the 04 frame uses a middle dot ("… to merge · SHOP-18 in Review").
**Deferred: built as option 1** (parentheses, following the decision); one function to change either way.

Notes for later milestones:
1. **M6 Local merge** and **M10 Mark merged** call `Scheduler::on_parent_merged(parent)` after setting the parent Done ·
   Merged (the service has it as `shared.scheduler`). M6's local merge itself is not built here; in M5 a merged parent
   is only a state tests set up.
2. **M6 Send back** into a full limit (d0034): put the ticket in Queued with `queued_at` set; the scheduler's slot rule
   then holds it as Waiting for a slot. `Starter::start` accepts Queued and Waiting for a slot (`WAITING_TO_START`).
   A sent-back ticket still has its worktree and maybe a live session: the start sequence reuses both.
3. **M7 inbox** reuses `Board::needs_you_items(now)`; the focused-terminal (T-0110) and plan-limit (T-0129) filters
   belong in that function so back-pressure follows them. `group_of` already places Interrupted and Resume failed.
4. **M7 reconcile** should know that `Starter::interrupt_cut_short` already handles starts cut short at service start.
5. **T-0129 (SCH-04)** adds the plan-limit hold to `scheduler::plan` / `Gate`.
6. The ports test `freed_ports_are_reused` didn't flake in this milestone (unchanged).
