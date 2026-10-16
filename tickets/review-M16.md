# M16 review: Epics on the board

M16 puts epics on the board (decision 0080): a child card wears its epic's **sleeve** instead of the old epic chip; an
epic that has started is a **track** in the "Epics in flight" strip above the columns, with one **stop** per child under
the column it's in; the epic card in Backlog is dragged to Ready for Dev or Dev to queue the whole epic (or one row to
queue one child), and a track is dragged back to Backlog to take it back (Park all when every child runs); a toast
says what was queued, with Undo and the Auto-merge-off variant; merged children fold into one line in Done; and an
epic can be focused (click its track name, ⇧⌘G / Ctrl+Shift+G) so the other cards dim. Each child is on the board
exactly once. It was built by parallel teammates in worktrees with fast-forward landings (decision 0079), on
2026-10-15 and 2026-10-16.

## 1. What was built

Oldest first, all on `main` (`git log --oneline 417c68a..main`):

| Commit | What |
|---|---|
| 0d13622 | Q55 resolved before the build: focusing an epic dims the other cards instead of hiding them (0080 refined; SDD EPC-10, EPC-18) |
| 4531280 | M16 planning: tickets T-0262–T-0275 (decisions 0080, 0079) |
| be59bbb | **T-0262** Core: the epic's queue mode (migration 0030), its place (`epic_place`: Card / Track / Delivery), and epic pins in the start order |
| 214bebf | **T-0267** Where epics and children show: the epic card only in its Card place, children as its rows, never a child card in Backlog |
| c4e0f6f | **T-0268** The strip's view model: tracks, stops, chips and their order (Q56) |
| c252e57 | **T-0265** Core and CLI: new children join a whole-queued epic ("Joined the queue"); adding or reopening after Review |
| 2d8a778 | **T-0263** Core: `epic.queue` (the whole epic, one child, resume parked children); `EPICS_NOT_TO_DEV` removed |
| ace43bc | **T-0266** The sleeve on child cards (replaces the epic chip), the "New · added by SHOP-40.2 · queued" line |
| 1b4ee54 | **T-0269** The strip on the board: tracks, stops, fold, hover and click (Q57) |
| 8870b56 | **T-0264** Core: take back, Park all, and Undo (`epic.take_back`, `epic.undo_queue`; Q58). **Landed with a red check, see §7** |
| ac066a2 | **T-0264 fix** the take-back and Undo futures built outside the board handler (tokio stack overflow in debug) |
| b0f224c | **T-0272** The queue toast: Undo, and the Auto-merge-off variant with "Turn on for epic children" |
| 81fd81f | **T-0270** Drag to queue: the epic card, one row, a "not queued" chip, ⇧⌘D |
| 083f31c | **T-0271** Take back: drag a track to Backlog, and the Park-all confirmation |
| bd80a96 | **T-0273** The folded Done line ("Wishlist · 2 of 4 merged") |
| 66a7d4e | **T-0274** Focus an epic: track name, ⇧⌘G, Esc, ↑↓, and the epic menu (Q59, Q60) |
| (this branch) | **T-0275** This review, the sweep for what 0080 replaced (the gallery's Move-to-epic cell), the Windows check, and the fix it needed: every board request's future built outside `board_requests::handle`'s frame |

## 2. How to try it

Node 24 comes from nvm on the Mac; nothing is installed globally. The app starts the service from the `otaq` binary
next to it, so build the workspace once first. Keep `OTAQ_HOME` short (macOS's 104-byte socket path limit).

```sh
export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"
cargo build --workspace                       # the otaq CLI / service the app starts
cd crates/otaq-desktop
npm ci
rm -rf /tmp/m16 && OTAQ_HOME=/tmp/m16 npm run tauri dev
```

- **The example project** (`tickets/m16-tickets.md` § "The example"). In onboarding, add a small temporary repository
  as the project `shop-web` with the key `SHOP` (`git init /tmp/shop-web && git -C /tmp/shop-web commit --allow-empty
  -m init`), then in Settings → shop-web set **Auto-merge Off** and keep validation on. Create the tickets with the CLI
  (`O="env OTAQ_HOME=/tmp/m16 ../../target/debug/otaq -P SHOP"`):

  ```sh
  $O create "Wishlist" --type epic                       # SHOP-40 in the checks below
  $O create "Wishlist API" --parent SHOP-40              # 40.1
  $O create "Wishlist button" --parent SHOP-40 --deps SHOP-40.1
  $O create "Wishlist page" --parent SHOP-40 --deps SHOP-40.2
  $O create "Search" --type epic                         # SHOP-45
  $O create "Search index API" --parent SHOP-45
  $O create "Search results page" --parent SHOP-45
  $O create "Gift cards" --type epic                     # SHOP-60, children 60.1, 60.2 (deps 60.1), 60.3
  $O create "Fix checkout rounding"                      # SHOP-51, standalone
  ```

  The keys won't be 40, 45, 60 and 51; read the checks with your own numbers. Set the External IDs (PAY-123, PAY-130,
  PAY-150) in each epic's panel.
- **Running children without a real agent.** A child that starts runs `claude` from `PATH`. For the checks that only
  need a child to sit in Dev, put a stand-in first on `PATH` before `npm run tauri dev`: a folder with an executable
  `claude` that is `#!/bin/sh` + `echo "stand-in agent"; exec cat` (the same stand-in the tests use, SDD §23.1). Checks
  that say **real `claude`** need the real one (resuming a parked conversation, an agent asking you, an agent running
  `otaq create --parent`). Merged and Closed children (the Done fold, flow 4) are quickest with a real agent and
  Auto-merge, or with the SQL of `review-M12-core.md` against `$OTAQ_HOME/otaq.db` (`status = 'merged'`).
- **Gallery** (`OTAQ_GALLERY=1 OTAQ_HOME=/tmp/m16 npm run tauri dev`): the sections "04 · Ticket card variants · Row ·
  epic children · the sleeve (T-0266, d0080)" and "03 · Board & columns · epic flows 2, 9, 12 · the queue toast (T-0272,
  d0080)", and the epic cards in "04 · Ticket card variants · Row · epics (fixtures)". The palette switch covers Otaq
  Light and Otaq Dark.
- **Key log:** `OTAQ_KEY_LOG=1` shows the last keys and where each went (for ⇧⌘G, Esc, ↑↓).
- **Window sizes:** 1440 × 900 (the 03 boards) and 1280 × 800 (the "1280 × 800 board (0080)" row; Done becomes its
  34 px rail).

## 3. Hand checks

None of these is ticked by agents. Do each in Otaq Light and Otaq Dark, at 1440 × 900 unless it says 1280 × 800.
Every frame is in `crewboard.pen`; "03 → …" means *03 · Board & columns → …*, "04 → …" means *04 · Ticket card variants ·
Light / Dark → Variant grid → …*, "01 → …" means *01 · Components · Epics on the board (0080) → Masters → …*. Build from
the 0080 rows only, never the rows tagged "Before 0080".

### Cards (the sleeve, the epic card)
- [ ] **T-0266** the sleeve against *01 → Epic Sleeve* and *04 → Row · epic children & Done (0080)*:
  - *Dev · epic child · working*: queue SHOP-40.1 (stand-in agent) so it sits in Dev; its card wears the sleeve
    "Wishlist · 1 of 3" (position among all of the epic's children, key order), no epic chip anywhere on the card.
  - *Dev · epic child · asks you*: a child whose agent asks you (**real `claude`**): the turquoise card keeps its sleeve.
  - *Pinned epic · child (no own pin)*: pin SHOP-40 (right-click → Pin epic), queue 40.3: the pin shows in 40.3's
    sleeve, not on the card itself; a child pinned on its own keeps its own pin.
  - SHOP-51 has no sleeve.
  - A long epic name (rename SHOP-40 to 60+ characters) at 1280 × 800 against *03 → 1280 × 800 board (0080) → Note ·
    Narrow window* ("Sleeve"): the name truncates, the "n of m" stays.
- [ ] **T-0266 / T-0265** *03 → epic flows 3 (0080) → State · 10 · A child added to a queued epic joins the queue*: queue
  SHOP-40 as a whole, then `otaq create "Wishlist share link" --parent SHOP-40` (and once from a child's agent, **real
  `claude`**): the new card goes straight to Ready for Dev with the violet line "New · added by you · queued" ("added by
  SHOP-40.2" from the agent); its stop on the track is marked new; History says "Joined the queue".
- [ ] **Epic card progress line** (seen in the Windows run, §6): "0 / 2 merged" and its segments draw cut in half under
  the title, on the card and its drag ghost. Compare the epic card with *01 · Components → Epic Card* on the Mac and
  on Windows; if both cut it, it is a bug to fix (where it came from was not looked into).
- [ ] **T-0267** *04 → Backlog · epic · drag one row out* and *03 → epic flows 1 → 1 · Drag the SHOP-45 card onto Ready
  for Dev* ("Each child is on the board exactly once"): with nothing started, SHOP-45 is one card in Backlog with its two
  children as rows; no child card anywhere in Backlog; the Backlog count counts the epic card once. After Park all
  (below) the rows read "Parked · has work".

### The strip
- [ ] **T-0269** against *03 · Board — shop-web · 1440 · Light* and *· Dark → Body → Board → Epics in flight*, *04 → Row ·
  Epics in flight strip (0080)* (the strip and its "Stops legend") and *01 → Track Label, Track Stop, Strip Header*:
  queue SHOP-40, SHOP-45 and SHOP-60 so all three are in flight.
  - Order: a pinned epic first (latest pin first), then one with a child needing you, then by when the first child was
    queued. The header's count and its needs-you count.
  - Each stop under its own column; Backlog's cell is the track label (name, External ID, "1/3", the segments); the
    stop states of the legend: not queued, parked, next, waits 40.2, working, asks you, conflict, in review, merged,
    closed. States the legend doesn't draw (Interrupted, Waiting for a slot…) reuse the nearest stop (Q56).
  - Fold and unfold the strip with its header's button; the fold is kept per project after a restart.
  - Hover a stop: its card gets the outline and scrolls into view; hover a card: its stop pulses; hover a track name:
    the other cards dim (preview).
  - Click a stop: opens that ticket; click a track name: focuses the epic (T-0274); double-click a name or ↵ on it:
    opens the epic's panel. ←/→ walk a track's stops.
  - *03 → epic flows 2 (0080) → 5 · A child needs you · the track says so too* (**real `claude`**): the stop turns
    turquoise and the track says so.
  - *03 → epic flows 2 → 7 · Scale: six epics in flight*: six epics in flight (create three more small epics).
- [ ] **T-0269** at 1280 × 800 against *03 → 1280 × 800 board (0080) → Board · 1280 × 800 · Light* and *Note · Narrow
  window*: Done is the 34 px rail and the track's Done cell is the rail too; stops drop their words when they don't
  fit (icon + key, the tooltip has the rest; needs-you keeps "asks you"); "+n" when even the short ones don't fit. The
  app's columns never go below 216 px, so the frame's "below ~200 px" form shows only when a station is crowded (Q57).
- [ ] **T-0262 / T-0266** *03 → epic flows 4 (0080) → State · 13 · A pinned epic*: pin SHOP-40 and leave SHOP-51 unpinned,
  both queued at P3: SHOP-40.1 sorts before SHOP-51 in Ready for Dev and starts first; pin SHOP-51 later than SHOP-40:
  SHOP-51 first.

### Drags and dialogs
- [ ] **T-0270** *03 → epic flows 1 (0080) → 1 · Drag the SHOP-45 card onto Ready for Dev* and *→ 2 · Right after the
  drop*, *03 → Notes → Note · Interactions (0080)* (Drag): drag SHOP-45 to Ready for Dev: the drop hint says "Queue 2
  children"; after the drop the toast, the track appears, two sleeved cards in Ready for Dev; Undo (below) takes it back.
- [ ] **T-0270** *03 → epic flows 3 (0080) → State · 8 · Drag one child out of the epic card* and *→ State · 9 · Right
  after*: hover a row of SHOP-45's card (the grip shows in the card's padding), drag only SHOP-45.1's row to Ready for
  Dev ("Queue SHOP-45.1"): a track with "SHOP-45.2 · not queued". Hover the chip: an inline **Queue** button; press it:
  45.2 queues. Also drag the chip onto Ready for Dev.
- [ ] **T-0270** ⇧⌘D on the selected SHOP-60 card queues the whole epic (Ctrl+Shift+D on Windows); ⇧⌘D on a selected
  child row queues that child. Drag SHOP-45 onto Done: not allowed, and the hint says why. Drag SHOP-45 onto Review: not
  allowed.
- [ ] **T-0271** *03 → epic flows 1 (0080) → 3 · Drag the track back onto Backlog*: with SHOP-45.1 running in Dev
  (stand-in agent) and 45.2 queued, drag the track by its name onto Backlog: hint "Unqueue 1 never-started child
  (45.2)" with the second line "SHOP-45.1 keeps running"; after the drop 45.2 is back as a "not queued" chip and the
  track stays. A short press on the name is still a click (focus).
- [ ] **T-0271** *03 → epic flows 4 (0080) → State · 11 · Drag the track back when every child has started*: with every
  child running, drag the track onto Backlog: hint "Park 2 running children"; the drop opens the Park-all
  confirmation (the shape of the existing Park dialog, 05 / 03, 0052), Cancel focused and on ↵: Cancel leaves both
  running; confirm parks both, the epic is a card again with "Parked · has work" rows. Drag that card to Ready for Dev:
  "Resume 2 children"; both resume their conversations (**real `claude`**: same conversation, new ports).
- [ ] **Q58** with SHOP-45.1 parked and 45.2 never started, drag SHOP-45 to Ready for Dev (45.1 resumed, 45.2 queued,
  both waiting), then drag the track back onto Backlog: both go back (45.1 to Parked with its work, 45.2 to Backlog),
  hint "Unqueue 2 queued children (45.1, 45.2)".

### Toasts
- [ ] **T-0272** against *03 → epic flows 1 (0080) → 2 · Right after the drop* (the toast **without "⌘Z"**), *03 → epic
  flows 3 → State · 9* (one child) and *03 → epic flows 4 → State · 12 · Auto-merge is off: the toast says so*, and
  *01 · Components → Toast*:
  - with Auto-merge On, queue SHOP-45: "Queued 2 children of SHOP-45" with Undo; queue one row: "Queued SHOP-45.1 of
    Search".
  - with Auto-merge Off in shop-web, queue SHOP-45: "Queued 2 tickets. Auto-merge is off for shop-web, so each one waits
    for your review." with **Turn on for epic children** and Undo; press Turn on: the toast becomes "Auto-merge: epic
    children only for shop-web" (Undo kept), and Settings → shop-web shows Auto-merge "Epic children only".
  - queue SHOP-60, press Undo: everything goes back, the toast just goes.
  - queue SHOP-40, wait for 40.1 to start (stand-in agent), press Undo: "Took back SHOP-40.2 · SHOP-40.1 already
    started" (40.2 and 40.3 back, 40.1 keeps running).

### Done
- [ ] **T-0273** against *01 → Done Fold*, *04 → Done · merged children folded* and *→ Done · fold opened*, *03 → epic
  flows 1 (0080) → 4 · Last child merged → the track becomes the Review card*: SHOP-40 with 40.1 and 40.2 merged and a
  40.5 closed: Done shows one line "Wishlist · 2 of 4 merged" where the first merged card sorts, and 40.5 as its own
  card ("Closed · not merged"). Open the fold (the cards appear under it, keyboard walk skips them while folded), close
  it; the Done count counts the fold once. At 1280 × 800 the Done rail's count. Merge the last child: the track goes
  and the epic is a Ready for Review card.

### Keys and the menu
- [ ] **T-0274** against *03 → epic flows 2 (0080) → 6 · Focus on Wishlist · click the track name* and *04 → Row · Epics
  in flight strip (0080)* (Focus):
  - Click Wishlist's track name: the other cards, tracks and Done lines dim (not hidden, the board keeps its shape);
    the filter bar shows `epic:Wishlist`; counts read "strong/total" (Backlog "0/2", Q59).
  - Select SHOP-40.2 and press ⇧⌘G: Wishlist is focused; the same from its terminal column (the key reaches the board
    from the terminal).
  - ↑↓ walk only the focused epic's cards; Esc leaves focus (after a drag cancel, the first Esc cancels the drag).
  - The Keyboard map lists "Focus the selected card's epic ⇧⌘G".
- [ ] **T-0274** *03 → Notes → Note · Interactions (0080)* (Right-click): right-click a track (and an epic card): Open
  epic, Queue never-started (n) / Resume parked (n), Unqueue never-started (n) / Park running (n)…, Add ticket to epic,
  Focus (⇧⌘G), Pin epic, Close epic…, plus Priority and Delete… (Q60). Pin epic on the second track: it moves first.
  Park running… opens the Park-all confirmation.

### Windows (Ctrl in place of ⌘)
- [ ] The agent's run in §6 covered one row drag, the toast, Ctrl+Shift+G with the panel open, Esc, a track dragged back
  and the drag answers. Still to do by hand in WebView2: a whole-epic drop with its toast and Undo, Turn on for epic
  children, Park all, the Done fold and its 34 px rail cell at 1280 × 800, and Ctrl+Shift+G from a terminal column
  (xterm.js gets the keys first). These start agents (stand-in or real `claude`).

## 4. Implementation choices (SDD §26)

- **The epic's place is computed, not stored (T-0262):** `epic_place(epic, children)` (Card / Track / Delivery) in
  `otaq-core` `board/epic_flight.rs` is a pure function every view reads; only the queue mode (`none` / `whole` /
  `single`, migration 0030) and a child's `joined_queue_by` are stored.
- **Each child once (T-0267):** `board::is_card(ticket, all)` decides what is a card; counts and "hidden by filter"
  count only cards, the epic card counts once with its rows.
- **Queue and take back in core (T-0263, T-0264):** `epic.queue` (whole epic, or `child` for one row or chip),
  `epic.take_back` (unqueue, or Park all with `confirm`), `epic.undo_queue`; each one transaction with History lines
  ("Queued with the epic by you", "Taken back"); user-only in the permission list. A parked child is resumed through the
  normal Parked → Dev path (0052). A waiting child that worked before is taken back too (Q58).
- **The drag reads the plan (T-0270, T-0271):** the drag feedback, the drop hint ("Queue 2 children", "Unqueue 1
  never-started child (45.2)", "Park 2 running children") and the drop step all come from `otaq-view` `drag.rs` /
  `board_page.rs`; React only lifts the card, row, chip or track. No new shortcut besides ⇧⌘G; ⇧⌘D was reused.
- **The strip (T-0268, T-0269):** the view model gives one station per shown column; React lays a track's cells out with
  the columns' own flex rules so they stay aligned on resize and scroll. Stops drop their words by the real station
  width (Q57). Missing stop states reuse the nearest drawn one (Q56).
- **Focus is a filter token (T-0274, Q55, Q59):** `epic:<title>` dims instead of hiding; the counts say strong/total.
- **The toast (T-0272):** toasts gained action buttons (`ToastLink::Actions`); Turn on writes the project's Auto-merge
  override as Epic children only (0064), the short-term part of EPC-15.
- **The 2 MB tokio stack (T-0264 fix, then T-0275):** a debug build gives every future that
  `service/board_requests.rs` `handle` holds inline its own slot in `handle`'s frame, about 1.57 MB of the 2 MB worker
  stack. The T-0264 fix moved its two new arms into plain fns returning a boxed future (`Answer<'a>`); the Windows
  check still overflowed (§6), so T-0275 builds **every** arm's future in `boxed(move || async move { … })`, its own
  call, and `handle` holds only the box. A new arm follows the same pattern. On the Mac the `otaq-cli` `agent` and
  `epic_children` tests overflowed with a 1 MB stack (`RUST_MIN_STACK=1048576`) before, and pass with 512 KB after.
- **Sweep for what 0080 replaced (T-0275):** nothing left of `card-epic-chip`, `epic_title` on cards, "Epics stay in
  Backlog", "Epics don't go to Dev", `EPICS_NOT_TO_DEV` or "can't be dragged" in `crates/`, the gallery or the tests
  (`epic_title` remains only as the toast's parameter name and the filter's epic-title match; `.epic-chip` is the
  External ID chip of the epic's own panel section, not a card chip). One child card in Backlog was left in the
  gallery: the Variant grid's "Ticket · Move to epic…" cell (SHOP-51 → SHOP-40.4, sleeved, status Backlog). It is now in
  Ready for Dev (a never-started ticket moved into an epic from Ready for Dev), so the gallery shows no child card in
  Backlog (`card_fixtures.rs`).
- **Files outside the planned lists**, named in each commit: `service/permissions.rs`, `tests/every_request.rs`,
  `history.rs`, `components/Dialog.tsx`, `app/registry/overlays.ts`, `screens/components/index.tsx`, `chain.test.ts`
  fixtures, `api/board.rs` / `api/mod.rs` / `generated.rs` lines for `board.park_all` and `board.focus`; T-0275:
  `card_fixtures.rs` and `service/board_requests.rs` (the stack fix).

## 5. Open questions

From `tickets/questions.md`:
- **Q55** (resolved before the build, user, option 1): focusing an epic dims the other cards (flow 6) instead of hiding
  them; 0080 refined, SDD EPC-10 and EPC-18 reworded (0d13622).
- **Q56** (deferred, built as option 1): stop states no frame draws (Interrupted, Resume failed, Waiting for limit reset,
  Start interrupted, Waiting for a slot, Held, Parent closed) reuse the nearest shape (`working` in Dev, `waits` in Ready
  for Dev), the tooltip names the real status. Option 2 would add a red `stopped` stop.
- **Q57** (deferred, built as option 1): the narrow-stop rule measures the real station width (words go below 200 px or
  when they don't fit; "+n" after that), not the 1280 frame's 193 px columns.
- **Q58** (deferred, built as option 1): taking back (and Undo) also takes back a child that waits in Ready for Dev but
  has worked before (resumed parked, reopened): back to Parked with its work.
- **Q59** (deferred, built as option 1): while an epic is focused the column counts read strong/total ("0/2"), not the
  frame's plain totals.
- **Q60** (deferred, built as option 1): the epic's right-click menu keeps Priority and Delete…, and words mixed cases by
  what the drop would do ("Resume parked (2)", "Unqueue queued (2)", "Park running (3)…").

## 6. Platforms checked

| Machine | Script | Rust tests | Vitest | Result |
|---|---|---|---|---|
| macOS (the Mac) | `scripts/check.sh`, by the orchestrator on every landing head (§7) | all passed | 846, 52 files | green on every landing (8870b56 excepted, §7) |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on 66a7d4e (main) | **stack overflow** in `otaq-cli` `tests/agent.rs` | 846, 52 files | **red**, fixed in T-0275 |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on a02fb2f (T-0275's fix) | all passed, 0 failed | 846, 52 files | **green**, 25 min |
| Ubuntu Arm VM | — | — | — | **not checked** |

**Windows (T-0275).** VMPal VM "Windows" (Windows 11 Pro 24H2, Arm64), running. The main checkout had another
session's uncommitted doc edits, so instead of robocopying the working tree the committed `main` was fetched into the
VM's old copy and checked out as a separate worktree: `git fetch Z:/software-factory main` in `%USERPROFILE%\cb-src`,
then `git worktree add --detach %USERPROFILE%\cb-m16 66a7d4e` (the old copy's own files were left as they were).
`CARGO_TARGET_DIR=%USERPROFILE%\cb-target`, then `powershell -ExecutionPolicy Bypass -File scripts\check.ps1` in
`cb-m16`. Nothing ran inside `Z:\`.

- **02:05–02:20 on 66a7d4e: red.** node, npm-ci, types, tsc, eslint, prettier, vitest (846), page, fmt and clippy passed;
  `test` stopped in `otaq-cli` `tests/agent.rs`: `thread 'tokio-rt-worker' has overflowed its stack`
  (STATUS_STACK_OVERFLOW), in `review_submit_refuses_uncommitted_files_then_submits`. The macOS checks had been green on
  the same tree: a debug build's `board_requests::handle` frame (about 1.5 MB, §7) plus a review submit's own futures
  passes the 2 MB tokio worker stack on Windows only. M15's Windows run (65ab766) was green, so M16's arms used up the
  last of the room. Fixed in T-0275 (§4): every arm's future is built and boxed outside `handle`'s frame.
- **02:26–02:51 on a02fb2f (the fix, fetched as `m16/T-0275`): green**, every step, `check.ps1: all checks passed`.

**Windows app check (agent-run, not the user's hand checks).** Release build as the README says
(`cargo build --release -p otaq-cli`, then `npm run tauri build`, about 2.5 min), started on the signed-in desktop
through a scheduled task (`%USERPROFILE%\m16\app.cmd` with `OTAQ_HOME=%USERPROFILE%\m16\h1` and `OTAQ_KEY_LOG=1`; the
task `cb-t0275-app` was deleted and the service stopped afterwards). Onboarding with `%USERPROFILE%\m15\shop-web` as
shop-web (Auto-merge off), then `otaq create` for an epic Search (SHOP-1) with SHOP-1.1 and SHOP-1.2 (depends on 1.1),
an epic Wishlist (SHOP-2) with two children, and SHOP-3. The VM has the real `claude` installed, so nothing was dropped
that would start an agent: only SHOP-1.2, which waits for SHOP-1.1 to merge, was queued.

| Check | Result | Evidence |
|---|---|---|
| Epics in Backlog are cards with their children as rows, no child card | **Pass** | Backlog "3": SHOP-1 and SHOP-2 cards with rows, SHOP-3 |
| Drag one row (SHOP-1.2) to Ready for Dev | **Pass** | ghost row, hint "Queue SHOP-1.2", Ready for Dev / Dev outlined, the others red; after the drop the strip "EPICS IN FLIGHT · 1", track "Search 0/2" with the chip "1.1" and the stop "1.2 waits 1.1" under Ready for Dev; the card wears the sleeve "Search · 2 of 2" with "waits for SHOP-1.1 to merge" |
| The toast with Auto-merge off | **Pass** | "Queued 1 ticket. Auto-merge is off for shop-web, so each one waits for your review." with Turn on for epic children and Undo (not pressed) |
| Ctrl+Shift+G with SHOP-1.2 selected (panel open) | **Pass** | filter bar `epic:Search` "2 results", counts "0/2", "1/1", "0/0"…, Wishlist and SHOP-3 dimmed, the track label outlined; key log "Ctrl+Shift+G (KeyG) → focus_epic" |
| Esc | **Pass** | the first Esc closed the panel, the second left the focus (both logged as `close_panel` in the key log) |
| Drag the track (by its name) to Backlog | **Pass** | hint "Unqueue 1 never-started child (1.2)"; after the drop the strip is gone and SHOP-1 is a card again |
| Drag the SHOP-1 card over Ready for Dev, then over Done, then Esc | **Pass** | "Queue 2 children"; over Done not allowed: "Use Close ticket, Approve or Mark merged to finish a ticket"; Esc cancelled, nothing queued |
| Whole-epic drop, Undo, Park all, the Done fold, the terminal column's Ctrl+Shift+G | **Not done** | each starts an agent (the real `claude` is installed in the VM) |

Seen there, for the hand checks: the epic card's progress line ("0 / 2 merged" and its segments) draws cut in half
under the title, on the card and on its drag ghost (Otaq Light, the maximised 1280-wide window). Not looked at on the
Mac in this run; compare with *04 → Row · epics* / the epic card master on both platforms.


**Linux: not checked.** There is no Ubuntu VM in VMPal (the orchestrator's VM list on 2026-10-16 had only "Windows").
By the M14 rule, a VM that can't be reached is skipped and recorded, not a blocker. Not seen on Linux therefore: the
strip, the drags and the toasts in WebKitGTK, and Ctrl+Shift+G there.

## 7. How it was run (decision 0079)

Like M15: an orchestrator on `main` started each ticket as a teammate in its own worktree and branch (`m16/T-xxxx`), at
most three at once, and landed each finished branch on `main` by **fast-forward only**, after the orchestrator's
`scripts/check.sh` on that exact head. A branch that fell behind was rebased onto the new `main` and checked again.
Bookkeeping (`progress.md`, ticked boxes, questions) was added on the rebased branch just before its check. Times are
Mac local time, 2026-10-15 evening into 2026-10-16.

**Order and timeline**

| Time | What | Why / notes |
|---|---|---|
| ~22:30 | `check.sh` green on main 417c68a before the start | a stale verification worktree removed (user OK) |
| 23:10–23:18 | planning: 0d13622 (Q55, its own commit) and 4531280 (tickets) each checked, landed 23:18 | the user approved the tickets and Q55 option 1 (dim) |
| 23:10–23:29 | **T-0262 alone**; be59bbb checked 23:24–23:29, landed (no rebase) | the migration and the `Ticket` fields everything reads |
| 23:30 | **Wave 1:** T-0263 ∥ T-0267 ∥ T-0268 | different files (core `epic_flight.rs` / `moves.rs` / protocol; `otaq-view` `board.rs` / `board_page.rs` / `epic_card.rs`; the new `epic_strip.rs`); chosen over T-0265 / T-0266 because they unblock the most (T-0264, T-0269, T-0270, T-0272, T-0273) |
| 23:36–23:41 | T-0267 214bebf ready, checked, landed; T-0265 started in its slot | T-0266 waits for T-0267 (both `card_page.rs`) |
| 23:39–23:47 | T-0268 ready (Q56), rebased → c4e0f6f after T-0267 (progress row only), checked, landed; T-0266 started 23:42 | |
| 23:47–23:53 | T-0263 ready on be59bbb; rebased → da067dc (T-0267's `board_page.rs` drag test, progress row) | it touched `drag.rs`, `board_page.rs`, `permissions.rs`, `history.rs` outside its list, forced by the compiler |
| 23:49–23:54 | T-0265 c252e57 ready on c4e0f6f, checked first (ready before T-0263's rebase), landed | |
| 23:50 | T-0269 started beside T-0263 (rebasing) and T-0266 | no overlap but T-0263's one `board_page.rs` test (T-0263 landed first) |
| 23:54–00:05 | T-0263 rebased again → 2d8a778 (`history.rs`, progress row), checked, landed; T-0264 started | order T-0265 → T-0263 → T-0266: T-0263 unblocks the most |
| 00:07–00:11 | T-0266 rebased → ace43bc, checked, landed | it touched one line of `board.rs` outside its list |
| 00:08–00:17 | T-0269 ready (Q57) on 2d8a778, rebased → 1b4ee54 after T-0266, checked, landed | |
| 00:21–00:28 | T-0264 ready on ace43bc; its question collided with T-0269's Q57 → rebased onto 1b4ee54 and renumbered **Q58** → 8870b56; landed 00:28 | **with a red check, see below** |
| 00:28–00:41 | main red at 8870b56; new work stopped; fix forward ac066a2 by the T-0264 teammate, checked 00:36–00:41, landed; main green again | |
| 00:41–00:56 | **T-0272 alone** (the only ready ticket; T-0270 needs it); b0f224c checked, landed | |
| 00:56–01:14 | **T-0270 alone** (`Board.tsx`, `drag.ts`, `board_page.rs`, `EpicCard.tsx`); 81fd81f checked 01:10–01:14, landed | |
| 01:14–01:30 | **T-0271 alone** (the same files); 083f31c checked 01:26–01:30, landed | |
| 01:30–01:44 | **T-0273**; bd80a96 checked 01:39–01:44, landed | T-0273 and T-0274 both touch `board.rs` / `board_page.rs` (the Done fold and the focus dimming): run one after the other ("when unsure, sequence") instead of side by side |
| 01:44–02:04 | **T-0274**; 66a7d4e ready (Q59, Q60), checked 02:00–02:04, landed | |
| 02:04 | **T-0275 alone** | this review, the sweep, the Windows VM |
| 02:05–02:20 | Windows `check.ps1` on 66a7d4e: **red** (stack overflow in `otaq-cli` `agent.rs`) | macOS green on the same tree, §6 |
| 02:21–02:26 | T-0275 boxes every arm of `board_requests::handle` (a02fb2f); `otaq-core` and `otaq-cli` tests green on the Mac (722 passed) | the fix the ticket asks for ("fix what fails") |
| 02:26–02:51 | Windows `check.ps1` on a02fb2f: **green** | |
| 02:53–03:00 | Windows release build and the app checks of §6 | |

Between 01:31 and 01:40 the orchestrator found the Windows VM running (agent control on), no Ubuntu VM, and another
session's uncommitted doc edits in the main checkout (decisions 0081–0083, `DESIGN.md`, briefs, `reports/`,
`research_notes/`): not M16's, left untouched; the fast-forward landings never touch those files.

The 13 code tickets (and the fix) landed in about 2 h 35 min (23:29–02:04); each `check.sh` took 4–5 minutes with
the shared build cache.

**Rebases and re-checks:** T-0268 once (progress row), T-0263 twice (T-0267's drag test; `history.rs` and the
progress row after T-0265), T-0266 once, T-0269 once (after T-0266), T-0264 once (Q57 → Q58). Every rebased head got its
own `check.sh` before landing; conflicts were only in `tickets/progress.md`, `tickets/questions.md`, `history.rs` and one
`board_page.rs` test, resolved by keeping both sides.

**The T-0264 incident.** T-0264's rebased head 8870b56 was landed although its `check.sh` was **red**: `otaq-cli`
`tests/epic_children.rs` aborted with a tokio worker **stack overflow** (SIGABRT). The orchestrator's landing command
didn't test the check's exit code and fast-forwarded anyway, so `main` was red at 8870b56 from 00:28. New work was
stopped (T-0272's worktree was created, but no teammate started), and the T-0264 teammate fixed forward on
`m16/T-0264-fix`: measured with lldb, the cause was the size of `board_requests::handle`'s future (about 1.57 MB of
the 2 MB worker stack in debug builds) once the two new arms were inlined; ac066a2 builds the take-back and Undo
futures in plain functions returning a boxed future (`Answer<'a>`), outside the handler's frame. Its check was green
at 00:41 and `main` was green again. **The landing command now checks that `check.sh` exited 0 before the
fast-forward.**

**What collided, and the lessons:**
- **Question numbers again (Q57):** T-0269 and T-0264 both took Q57 from their own branches; T-0264, landing second,
  renumbered to Q58 on rebase. Question numbers are taken from `main` at landing time (the M16 ticket file says so),
  and that still needs the lander to check.
- **The 2 MB tokio stack:** `board_requests::handle`'s frame was close to the worker stack's limit in debug builds,
  and closer on Windows: the macOS `check.sh` stayed green after the T-0264 fix while Windows `check.ps1` overflowed in
  an unrelated test (`otaq-cli` `agent.rs`, a review submit). T-0275 boxed every arm (§4), which leaves well over 1 MB
  of headroom. Run the full `otaq-cli` and `otaq-core` tests before landing a new arm; a quick probe of the headroom is
  `RUST_MIN_STACK=1048576` on an already built test binary (it applies to rustc too, so build first).
- **Windows is a check of its own:** M16's code landed with green macOS checks only; the Windows run at the end found
  the overflow. A Windows `check.ps1` part-way through a milestone (after the core tickets) would have found it earlier.
- **Check the exit code:** a check that ran is not a check that passed; the landing step tests `exit 0` first.
- **Files outside the lists** are named in each commit (§4); most were forced by the compiler (a new field or a
  removed constant) or were one-line registrations.
- **Sequencing the busy files:** `Board.tsx`, `drag.ts` and `board_page.rs` made T-0272 → T-0270 → T-0271 → T-0273 →
  T-0274 a single lane; only phases 1–3 ran three teammates side by side.
