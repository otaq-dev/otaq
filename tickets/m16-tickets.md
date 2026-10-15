# M16 · Epics on the board — tickets

M16 makes epics readable and startable on the board. Epic children wear a violet sleeve, an "Epics in flight" strip shows
one track per epic above the columns, dragging an epic queues its children, and dragging it back takes them back. The
sources are:
- `decisions/0080` (this feature; with its changelog it overrides anything older that disagrees) and `decisions/0079` (how
  the milestone is built: an orchestrator, teammates, one worktree and one squashed commit per ticket);
- `SDD.md` §6.5 EPC-09…EPC-18, with §6.4, §7.3–§7.5, §13, §15, §21.3 and §23;
- the design, update Q in `crewboard.pen`; the frame map and the audit are in `design-briefs/CHANGES-2026-10-15-q.md`.

**14 tickets, T-0262–T-0275, one epic: M16 · Epics on the board.**

- **Format:** the same as `tickets/m15-tickets.md` (the shape for `otaq create -f`, 0032). The IDs continue that file's.
- **Definition of done** (0056): `scripts/check.sh` is green, new behaviour has tests, the "Done when" items are met, and the
  commit names the ticket, its requirement IDs and decisions 0080 / 0079. **A conflict with a decision → defer or stop as
  `prompts/implementer.md` says.**
- **Hand checks** compare the running app with the named frames. Agents never tick them. Run the app with
  `OTAQ_HOME=/tmp/m16 npm run tauri dev` in `crates/otaq-desktop`, in Otaq Light and Otaq Dark, at 1440 × 900 and
  1280 × 800. Use the test project of § "The example" below.
- **Build only what 0080 and EPC-09…EPC-18 say.** Out of scope: swimlanes, per-epic colours or shapes, a token rename
  (violet stays `st-investigate`), ⌘Z or any new shortcut other than ⇧⌘G, and anything about moving away from worktrees.
  The Auto-merge part of EPC-15 is short-term: keep it small.
- **Build from the frame map only**, never from the exploration pages or rows tagged "before 0080". Where a frame and 0080
  differ, 0080 wins.

## Words used in these tickets
- **Never-started child:** a child that has never been to Dev (`has_been_to_dev` is false). It is a row in the epic card,
  a card in Ready for Dev, or a "not queued" chip on its track.
- **Active child:** a child in Ready for Dev, Dev or Ready for Review.
- **The epic's queue mode** (stored on the epic, T-0262):
  - **None:** the epic was never queued, or it was taken back.
  - **Whole:** the user queued the whole epic (the epic card or its track dragged to Ready for Dev / Dev, ⇧⌘D, or the menu).
    New children join the queue (EPC-13).
  - **Single:** the user queued one child row only. New children wait as "not queued" (EPC-13).
- **The epic's place** (EPC-11), a pure function of the epic and its children (T-0262):
  - **Card** (in Backlog): no child is active, and either no child has ever started, or a child is Parked (after Park all,
    or a single Park). The children are rows inside the card, never separate cards.
  - **Track** (in the strip): otherwise, until the epic reaches Ready for Review.
  - **Delivery card:** Ready for Review, Approved, Done (EPC-04, unchanged).
  - Examples: SHOP-45 with nothing started is a card. SHOP-45.1 merged and SHOP-45.2 taken back (never started) is a track
    with a "not queued" chip (flow 3: "SHOP-45.1 has started, so the track stays"). After Park all it's a card with "Parked
    · has work" rows. A child reopened after Review makes it a track again (0080).

## Where the code goes
This list is what makes parallel work possible, so keep to it. If a ticket really needs another place, name it in its
commit and in `tickets/review-M16.md`.
- **Core** (`crates/otaq-core`), tested without a UI (SDD §23.1):
  - `migrations/0030_epic_queue.sql`: T-0262 only.
  - `src/board/epic_flight.rs` (new): queue mode, the epic's place, queue / take back / Undo. Created by T-0262; T-0263 and
    T-0264 add to it.
  - `src/board/moves.rs`: `check_move` for epics and child rows (T-0263).
  - `src/board/tickets.rs` (create) and `src/board/epics.rs` (`epic_back_to_backlog`, `reevaluate_epic`): T-0265.
  - `src/board/order.rs`: the start order with epic pins (T-0262).
  - `src/review/park.rs`: Park all (T-0264).
  - `src/protocol.rs` and `src/service/board_requests.rs`: one request each for T-0263 and T-0264 (see "Shared files").
- **View models** (`crates/otaq-view/src/view_models/`): every wording and every rule the screen shows (SDD §5.5).
  - `card.rs`, `card_page.rs`, `card_fixtures.rs`: the sleeve (T-0266).
  - `epic_card.rs`: the epic card's rows (T-0267).
  - `board.rs`, `board_page.rs`: where epics and children show (T-0267), the strip on the page (T-0269), drag answers
    (T-0270, T-0271), the Done fold (T-0273).
  - `epic_strip.rs` (new): tracks and stops (T-0268).
  - `epic_toast.rs` (new) and `toasts.rs`: the queue toast and its actions (T-0272).
  - `drag.rs`: the drag feedback (T-0270, T-0271).
  - `filter.rs`, `card_menu.rs`, `crates/otaq-view/src/keymap.rs`: focus, the menu and ⇧⌘G (T-0274).
- **Desktop** (`crates/otaq-desktop`):
  - `src/api/board.rs`: the board view, drag and drop (T-0269, T-0270, T-0271, T-0273). `src/api/notices.rs`: the toast
    actions (T-0272).
  - `ui/src/cards/`: the sleeve (T-0266), the epic card's draggable rows (T-0270).
  - `ui/src/screens/board/strip/` (new): the strip, its tracks and stops (T-0269; T-0270, T-0271 and T-0274 add to it).
  - `ui/src/screens/board/Board.tsx`, `Column.tsx`, `drag.ts`, `board.css`: see each ticket. **`Board.tsx` is the busiest
    file: the tickets that touch it run one after the other.**
  - `ui/src/components/Toast.tsx`: the toast's action buttons (T-0272).
- **No new npm package or crate.** dnd-kit (or whatever the board's drag uses today) is already there. If a ticket needs a
  package, it is a blocker (it changes `package.json` / `Cargo.toml`, which runs alone).

## Shared files
A ticket may add **one entry** to each of these registration files; conflicts there are one-line merges that keep both sides:
- `crates/otaq-core/src/protocol.rs` (one `Request` variant and its name) and `src/service/board_requests.rs` (its dispatch);
- `crates/otaq-desktop/src/generated.rs` (`export_all`, one line per new type) and the files it writes in
  `ui/src/generated/`;
- `crates/otaq-view/src/view_models/mod.rs` (one `pub mod` line);
- `crates/otaq-view/src/icons.rs`, its SVG assets and the generated `IconName.ts` (the M15 lesson: icons are a shared list);
- `crates/otaq-desktop/src/api/mod.rs` (one command line);
- `crates/otaq-desktop/ui/src/screens/components/Gallery.tsx` (one section per ticket).

Bookkeeping (`tickets/progress.md`, the ticked boxes here, `tickets/questions.md`) is added on the rebased branch just before
the check (0079). **Question numbers are taken from `main` at landing time** (Q55 onwards), not from the branch.

## The example
A test project `shop-web` (key SHOP), Auto-merge Off, validation on. Use it for every hand check:

| Key | Title | Notes |
|---|---|---|
| SHOP-40 | Wishlist (External ID PAY-123) | epic, children 40.1 Wishlist API, 40.2 Wishlist button (depends on 40.1), 40.3 Wishlist page (depends on 40.2) |
| SHOP-45 | Search (PAY-130) | epic, children 45.1 Search index API, 45.2 Search results page |
| SHOP-60 | Gift cards (PAY-150) | epic, children 60.1, 60.2 (depends on 60.1), 60.3 |
| SHOP-51 | Fix checkout rounding | standalone ticket |

Sessions use an ordinary command in place of `claude` where a test needs a running child (SDD §23.1); hand checks that
need a real agent say so.

| Epic | Tickets |
|---|---|
| M16 · Epics on the board | T-0262 – T-0275 (14) |

## M16 · Epics on the board
Goal: the user can tell epic children from standalone tickets at a glance, see how far each epic is, and start or take back
a whole epic with one drag.

### T-0262 · Core: the epic's queue mode, its place, and epic pins in the start order
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-11, EPC-13 (storage), EPC-17
- **Decisions:** 0080, 0047, 0065, 0059, 0039
- **Design:** none (no UI)
- **Depends on:** none
- **Size:** M
- **Shared files:** `migrations/0030_epic_queue.sql` (new), `src/board/tickets.rs` (the `Ticket` struct and its read only),
  `src/store/tickets.rs`, `src/board/order.rs`, creates `src/board/epic_flight.rs`, one line in `src/board/mod.rs`

**What and why**
The data every later ticket reads.
- **Migration 0030:**
  - on the epic: its **queue mode** (`none` / `whole` / `single`) and when it was set;
  - on a child: **who added it to the queue by joining** (`joined_queue_by`, an actor: the user, or the ticket whose agent
    ran `otaq create --parent`), set only when a child joins a whole-queued epic (EPC-13; T-0265 sets it). It clears when
    the child starts or is taken back.
- `Ticket` gains `epic_queue_mode` (epics) and `joined_queue_by` (children), read from the store, serialised only when set.
- **`epic_flight.rs`**, pure functions with tests:
  - `epic_place(epic, children) -> EpicPlace { Card, Track, Delivery }`, by the rule in "Words used in these tickets".
    Test every example listed there, plus: an epic whose children are all never-started but one is in Ready for Dev
    (Track); every child merged (Delivery once `reevaluate_epic` moved it); every child Closed ("Closed · not merged",
    Delivery).
  - `never_started(children)`, `active(children)`, `running(children)` (an agent session or Dev state), `parked(children)`.
  - `set_queue_mode` on the board (store + History line "Queued the whole epic" / "Taken back"), used by T-0263 and T-0264.
- **Start order (EPC-17, 0047, 0065):** a child of a **pinned epic** sorts as if it were pinned, with the epic's
  `pinned_at`; a child pinned on its own keeps its own pin (the later of the two counts). The Ready for Dev column order
  follows (it uses the start order). Test: SHOP-40 pinned, SHOP-51 not pinned, both queued at P3: 40.1 starts first;
  SHOP-51 pinned later than SHOP-40: SHOP-51 first.

**Done when**
- [x] migration 0030 applies on an M15 database and keeps every row; the store reads and writes both fields
- [x] `epic_place` has a test for each listed case
- [x] start order with an epic pin is tested (scheduler plan and the Ready for Dev sort)
- [x] checks green; the commit names EPC-11, EPC-13, EPC-17 and decisions 0080, 0079

### T-0263 · Core: queue an epic, one child, and resume parked children
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-12, EPC-14 (resume), EPC-15 (the outcome the toast reads)
- **Decisions:** 0080, 0066, 0016, 0049, 0052, 0058, 0064
- **Design:** none (no UI); the wording the toast needs comes from T-0272
- **Depends on:** T-0262
- **Size:** M
- **Shared files:** `src/board/epic_flight.rs`, `src/board/moves.rs`, one request in `src/protocol.rs` and
  `src/service/board_requests.rs`

**What and why**
- **`check_move` (moves.rs):** an epic in its **Card** or **Track** place may be dropped on Ready for Dev or Dev
  (`MoveAction::QueueEpic`). Remove `EPICS_NOT_TO_DEV` and its refusal (0080 supersedes "Epics can't be dragged",
  SDD §7.4). An epic in Delivery keeps today's rules. A child's own move rules don't change.
- **`queue_epic(epic, actor)`** (epic_flight.rs; one request, e.g. `epic.queue`):
  - every **never-started** child goes to Ready for Dev (Queued) in one transaction, as the user's drag (`queued_at` set,
    History "Queued with the epic by you");
  - every **Parked** child is **resumed** through the existing Parked → Dev path (0052: same conversation, new ports);
  - the queue mode becomes **Whole**;
  - normal rules then apply: dependencies (0049), the optional limit (0016), start order (0065); nothing starts that the
    scheduler wouldn't start for a dragged ticket. The user's drag is the go-ahead (0016, 0066 still hold).
  - It returns `EpicQueueOutcome { epic, queued: [keys], resumed: [keys], auto_merge_off: bool, project }`, where
    `auto_merge_off` is the project's effective Auto-merge being Off (0064).
  - Nothing to queue or resume (every child active or done): refused with "Every child of SHOP-45 is already queued or
    started".
- **One child row:** dropping a never-started or Parked child (a row of the epic card, or a "not queued" chip of a track)
  on Ready for Dev / Dev queues or resumes **only that child**. If the queue mode is None, it becomes **Single**; Whole stays
  Whole. The outcome has one key.
- **⇧⌘D on an epic** is a drop on Ready for Dev, as for a ticket (0058): same function.
- Tests with temporary repositories (SDD §23.1): queue SHOP-45 (both children queued, mode Whole, outcome lists both); queue
  one row of SHOP-45 (mode Single, place Track); queue after Park all (parked resumed, never-started queued); dependencies
  still hold (40.2 waits for 40.1); the limit holds; nothing to queue is refused.

**Done when**
- [x] `check_move` allows an epic to Ready for Dev / Dev; the old refusal and its constant are gone, with their tests
- [x] `queue_epic` and the single-child path are tested as listed, including resume of Parked children (an ordinary
  command in place of `claude`)
- [x] the outcome says `auto_merge_off` for a project with Auto-merge Off
- [x] checks green; the commit names EPC-12, EPC-14, EPC-15 and decisions 0080, 0079

### T-0264 · Core: take back, Park all, and Undo
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-14, EPC-15 (Undo)
- **Decisions:** 0080, 0052, 0066, 0018
- **Design:** none (no UI); the wordings below are from *03 → epic flows 1* (3) and *flows 4* (11)
- **Depends on:** T-0263
- **Size:** M
- **Shared files:** `src/board/epic_flight.rs`, `src/review/park.rs`, one request in `src/protocol.rs` and
  `src/service/board_requests.rs`

**What and why**
- **`take_back_plan(epic)`**, pure: what dropping the track on Backlog would do.
  - Some children never started and are queued → **Unqueue** those; running ones keep going. The drop hint reads
    "Unqueue 1 never-started child (45.2)" and "SHOP-45.1 keeps running" (flow 3).
  - Every child has started (nothing to unqueue) and some run → **Park all**, after one confirmation: title "Park 2 running
    tickets?", body "Their work is kept. SHOP-45.1 and SHOP-45.2 stop and go back to Backlog with their conversations.",
    buttons "Cancel" / "Park 2 tickets"; the drop hint "Park 2 running children" (state 11).
  - Nothing queued and nothing running → refused, "Nothing of SHOP-45 is queued or running".
  - These strings live in the core's plan (or in `otaq-view`, T-0271 decides) so the screen and the service agree.
- **`take_back(epic, confirmed, actor)`** (one request, e.g. `epic.take_back`):
  - Unqueue moves never-started queued children back to Backlog (0066, nothing had started) and clears `joined_queue_by`.
  - Park all calls the existing Park (0052) for every running child, one after the other in one request: agents stop, work
    and session IDs are kept, ports freed. Without `confirmed` it is refused with `NeedsConfirmation` and the plan's text.
  - Either way the queue mode becomes **None**. The place follows from T-0262 (nothing started → Card; Park all → Card with
    "Parked · has work" rows; otherwise Track).
- **Undo** of a queue toast (`epic.undo_queue` with the outcome's keys, or a flag on take_back): takes back only the keys
  that are still never-started and queued; it never parks. Its reply lists the ones that already started, so the toast can
  say "SHOP-45.1 already started". Mode → None when it took back everything it queued; otherwise unchanged.
- Tests: unqueue with one running child (flow 3); unqueue with nothing started → Card; Park all (two running children,
  confirmation first, both Parked with work kept) → Card; Undo right after queueing → Card, mode None; Undo after 45.1
  started → 45.2 back, reply names 45.1.

**Done when**
- [ ] the plan, take back and Undo are tested as listed
- [ ] Park all reuses Park (0052), including stopping a running validator (0063)
- [ ] checks green; the commit names EPC-14, EPC-15 and decisions 0080, 0052, 0079

### T-0265 · Core and CLI: new children join a whole-queued epic; adding or reopening after Review
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-13, EPC-11 (back from Review)
- **Decisions:** 0080, 0039, 0044, 0059, 0031, 0068
- **Design:** none (no UI)
- **Depends on:** T-0262
- **Size:** S
- **Shared files:** `src/board/tickets.rs` (create), `src/board/epics.rs`, tests in `crates/otaq-cli`

**What and why**
- **Creating a child** (by hand "Add ticket to epic", or `otaq create --parent SHOP-40` from any agent or the user's shell):
  - queue mode **Whole** → the child goes straight to Ready for Dev, `joined_queue_by` = the creator (the user, or SHOP-40.2
    when SHOP-40.2's agent ran it), History "Joined the queue: SHOP-40 is queued as a whole";
  - **Single** or **None** → Backlog, as today (a "not queued" chip on a track, or a row of the card).
  - The CLI reply and `otaq show` are unchanged: **nothing new for agents to learn**. The skill and `otaq prime` don't change.
  - A Project agent's `otaq create --parent` follows the same rule: the user's earlier drag of the whole epic is the
    go-ahead (0080), so this isn't the Project agent moving a ticket (0068).
- **After Review (EPC-11, refines EPC-05's "returns the epic to Backlog"):** adding a child to an epic in Ready for Review or
  Approved, or reopening one of its Closed children, moves the epic back out of Review as today (`epic_back_to_backlog`), and
  its place is then a **Track**. The added child follows the queue mode above; the queue mode is kept through Review, so an
  epic queued as a whole takes its new child into the queue. A reopened child goes to Backlog (CLS-03) and shows as "not
  queued".
- Tests: through the service with `otaq create --parent` (Whole → Ready for Dev with `joined_queue_by`; Single → Backlog);
  "Add ticket to epic" by the user (joined by "you"); add after Review → Track, child queued (Whole); reopen after Review →
  Track, child in Backlog.

**Done when**
- [x] the four tests above pass, the first one through the real CLI and service with `OTAQ_HOME` in a temporary folder
- [x] `otaq create --parent` output and `otaq prime` are unchanged (existing tests still pass untouched)
- [x] checks green; the commit names EPC-11, EPC-13 and decisions 0080, 0079

### T-0266 · The sleeve on child cards (replaces the epic chip)
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-09, EPC-17 (the sleeve's pin), EPC-13 (the card's "new" line)
- **Decisions:** 0080, 0039, 0047, 0068, 0077
- **Design:** crewboard.pen → 01 · Components · Epics on the board (0080) → Sleeve (`Epic Sleeve`); 04 · Ticket card
  variants · Light / Dark → Row · epic children & Done (0080) → Dev · epic child · working, Dev · epic child · asks you,
  Pinned epic · child (no own pin); 03 · Board & columns → epic flows 3 (0080) → 10 (the card's "New · added by SHOP-40.2 ·
  queued"); the 1280 × 800 note ("Sleeve")
- **Depends on:** T-0262
- **Size:** M
- **Shared files:** `otaq-view` `card.rs`, `card_page.rs`, `card_fixtures.rs`; `ui/src/cards/TicketCard.tsx`, `cards.css`,
  `look.ts`; one Gallery section

**What and why**
- **View model:** `TicketCardView` gains `sleeve: Option<Sleeve { epic_key, name, position, pinned }>` for every epic child,
  in every column: name is the epic's title, position "2 of 4" (the child's place among the epic's children in key order,
  out of all of them). `None` for standalone tickets. **The epic chip goes** (`epic` / `epic_title` fields, the
  `.card-epic-chip` markup and CSS, their tests and fixtures).
- **Pins (EPC-17):** a pinned epic shows a small pin in the sleeve; its children don't show their own pin icon unless they
  were pinned on their own.
- **"New" line (EPC-13):** a child with `joined_queue_by` shows "New · added by SHOP-40.2 · queued" (or "by you") as drawn
  in state 10, in the violet tone.
- **React:** the band on top of the card from *Epic Sleeve*: `st-investigate` violet, the epic name truncated with …, the
  position never truncated (1280 note). The violet glow of an agent-created ticket (0068) keeps working beside it.
- Turquoise needs-you cards keep their colour; the sleeve stays violet on top (*asks you* frame).

**Done when**
- [x] view-model tests: sleeve on children with the right position; none on SHOP-51; pin from a pinned epic; the "new" line
- [x] the epic chip is gone from the view model, the card, the CSS and the gallery
- [ ] matches the three 04 frames named above, light and dark
  - *Hand check:* SHOP-40.2 working in Dev, SHOP-40.2 asking you, SHOP-40.3 with SHOP-40 pinned; a long epic name at 1280 × 800.
- [x] checks green; the commit names EPC-09, EPC-13, EPC-17 and decisions 0080, 0079

### T-0267 · Where epics and children show: card, rows, track, no Backlog duplicates
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-11, EPC-14 (the rows after Park all), EPC-01 (refined)
- **Decisions:** 0080, 0039, 0052, 0059
- **Design:** 04 · Ticket card variants → Row · epic children & Done (0080) → Backlog · epic · drag one row out; 03 · Board &
  columns → epic flows 1 (0080) → 1 and 2 ("Each child is on the board exactly once")
- **Depends on:** T-0262
- **Size:** M
- **Shared files:** `otaq-view` `board.rs`, `board_page.rs` (column entries only), `epic_card.rs`

**What and why**
- **Columns:** an epic shows as a card in Backlog only in its **Card** place, and in Review / Approved / Done in its
  **Delivery** place. In its **Track** place it isn't a card at all (T-0268 draws the track). **Epic children never show as
  cards in Backlog**: they are rows of the card, or "not queued" chips on the track. Children in Ready for Dev, Dev, Review
  and Done are cards as usual.
- **The epic card's rows** (`epic_card.rs`): each row has the child's key, title and status as today, plus `draggable`
  (never-started or Parked children of an epic in its Card place) and the status "Parked · has work" for a parked child
  (the existing wording, 0052; the audit's decision for state 11).
- Column counts and the empty-column hints count what is shown.
- Tests in `board.rs` / `board_page.rs`: SHOP-45 with nothing started (one card, no child cards); SHOP-45.1 queued (no
  epic card, 45.1 in Ready for Dev, 45.2 nowhere in Backlog); after Park all (card, two "Parked · has work" rows,
  draggable); SHOP-40 in Review (Delivery card as today). Update the existing "epics in Backlog" tests.

**Done when**
- [x] the tests above pass; no child card is ever in Backlog
- [x] the epic card's rows say "Parked · has work" and know which are draggable
- [x] checks green; the commit names EPC-01, EPC-11, EPC-14 and decisions 0080, 0079

### T-0268 · The strip's view model: tracks, stops and their order
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-10, EPC-13 (the "new" flag), EPC-17 (order)
- **Decisions:** 0080, 0047, 0042, 0077, 0049
- **Design:** 01 · Components · Epics on the board (0080) → Track Label, Track Stop, Strip Header; 04 · Ticket card variants →
  Row · Epics in flight strip (0080) (ordering, focus, folded, **every stop state**); 03 · Board & columns → epic flows 2
  (0080) → 5, 7; the 1280 × 800 note
- **Depends on:** T-0262
- **Size:** M
- **Shared files:** creates `otaq-view/src/view_models/epic_strip.rs`; one line each in `view_models/mod.rs` and
  `generated.rs`

**What and why**
A pure view model, `Strip::from(tickets, columns)`, with Rust tests. It isn't wired into the page yet (T-0269 does that).
- **Present** only while at least one epic is in its **Track** place.
- **Header:** "Epics in flight · 3" and "· 1 needs you" (hidden at 0), and the hint "hover a stop to find its card · click
  a name to focus". The **folded** line: "Epics in flight 2 · 1 needs you".
- **Tracks**, ordered: pinned epics (most recently pinned first, 0047) → tracks with a needs-you child → the rest (by when
  the epic was queued). Each track: epic key, name, External ID, "1/5" merged of all, the mini bar (one segment per
  child), `pinned`, `needs_you` (the dot), and its **not queued** chips (never-started children in Backlog: "SHOP-40.3 · not
  queued", "+2" when more than fit is the screen's choice).
- **Stops**, one per child that isn't in Backlog, each with its **column** (so the screen places it under that column) and
  its **state**, matching the 04 strip row:
  - `next` (queued, nothing to wait for), `waits 40.2` (queued, waiting for a sibling or other dependency; the first
    blocker's short key), `working` (Dev, any working or validating state), `needs you` / `asks you` (turquoise: every
    needs-you state, 0042; "asks you" for Asks you), `conflict` (Merge conflict · fixing / Keeps conflicting), `in review`
    (magenta, Ready for Review), `merged` (Done · Merged);
  - the **new** flag, "new · added by 40.2" / "new · added by you" (`joined_queue_by`);
  - short labels at narrow widths: `label` and `short_label` (icon + key only), and the tooltip text; needs-you stops keep
    "asks you" (1280 note).
- **Design gaps** (no frame draws them; reuse, don't invent):
  - **A parked child while the epic is a track** (another child still active): shown as a "not queued" chip with the
    existing "Parked · has work" wording instead of "not queued".
  - **A closed child:** a stop under Done that reuses the `merged` stop's shape with the word "closed" in the muted
    closed tone the Done cards already use.
- Tests: the 04 strip row's sample (Search, Wishlist, Gift cards) gives that order and those stop states; a pinned epic
  first even with a needs-you track below it; a Single-mode track with a "not queued" chip; the two gaps.

**Done when**
- [x] `epic_strip.rs` with the tests above; every stop state of the 04 strip row has a test
- [x] the generated types are committed and fresh
- [x] checks green; the commit names EPC-10, EPC-13, EPC-17 and decisions 0080, 0079

### T-0269 · The strip on the board: tracks, stops, fold, hover and click
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-10
- **Decisions:** 0080, 0075, 0058
- **Design:** 03 · Board — shop-web · 1440 · Light / Dark (the strip); 01 · Components · Epics on the board (0080) → all five;
  04 · Ticket card variants → Row · Epics in flight strip (0080); 03 · Board & columns → epic flows 2 (0080) → 5, 7 and Note ·
  Interactions (0080) (Hover, Click); Row · 1280 × 800 board (0080) and Note · Narrow window
- **Depends on:** T-0267, T-0268
- **Size:** L
- **Shared files:** `otaq-view` `board_page.rs` (the `strip` field), `src/api/board.rs` (view), creates
  `ui/src/screens/board/strip/`, `Board.tsx` (mount above the columns), `board.css`, `prefs.ts`

**What and why**
- `BoardPage` gains `strip: Option<Strip>`; the board view sends it.
- **React, `screens/board/strip/`:** the header (Strip Header), one row per track (Track Label at the left over Backlog, then
  the stops under their columns, aligned with the columns as they resize, collapse to a rail or fold). Six tracks take 6 ×
  34 px (flow 7).
- **Fold:** clicking the header folds the strip to its one line; clicking again unfolds. No shortcut. The choice is kept per
  project the way the Done fold is (`prefs.ts`); an implementation choice to record.
- **Hover** a stop → its card gets an outline and scrolls into view. **Hover a sleeved card** → its stop pulses. **Hover a
  track name** → focus preview (other cards dim) (Note · Interactions).
- **Click** a stop → selects that child's card (as a click on the card). **Double-click** the track name or **↵** on a
  focused track → the epic panel (the existing epic panel). **Click the track name** → focus: wired by T-0274; until then
  it selects the epic.
- **1280 × 800:** below about 200 px per column a stop drops its words and keeps icon + key, with the rest in a tooltip;
  needs-you stops keep "asks you"; the epic cell keeps name, External ID and n/N, and the mini bar goes first; the Done
  station follows the Done rail and shows "n merged".
- Keyboard: the strip's tracks are in the tab order after the filter bar; ←/→ move between stops of a track.

**Done when**
- [x] the page carries the strip from the view model (a Rust test in `board_page.rs` or `api/board.rs`)
- [x] Vitest tests for the plain parts (column alignment, the narrow-stop rule, fold state)
- [ ] matches the 1440 boards (light and dark), the 04 strip row and the 1280 × 800 board
  - *Hand check:* SHOP-40, SHOP-45 and SHOP-60 in flight: order, header count, fold and unfold, hover outline and pulse,
    click selects, double-click opens the epic panel; the same at 1280 × 800 with Done collapsed to its rail.
- [x] checks green; the commit names EPC-10 and decisions 0080, 0079

### T-0270 · Drag to queue: the epic card, one row, a "not queued" chip, ⇧⌘D
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-12, EPC-14 (resume a parked row), EPC-15 (shows the toast)
- **Decisions:** 0080, 0066, 0058, 0052
- **Design:** 03 · Board & columns → epic flows 1 (0080) → 1, 2; epic flows 3 (0080) → 8, 9; 04 · Ticket card variants →
  Backlog · epic · drag one row out; 04 strip row → the "not queued" chip ("hover: Queue"); Note · Interactions (Drag)
- **Depends on:** T-0263, T-0267, T-0269, T-0272
- **Size:** L
- **Shared files:** `otaq-view` `board_page.rs` (`drag_answer`, `drop_step`), `drag.rs`; `src/api/board.rs` (drag and drop);
  `ui/src/screens/board/Board.tsx`, `drag.ts`, `Column.tsx`; `ui/src/cards/EpicCard.tsx`, `cards.css`; `strip/`

**What and why**
- **Drag answers** (view model, sent at drag start, SDD §5.5): for the epic card, a row and a chip, every column says
  allowed / not allowed with its reason; Ready for Dev and Dev accept. The drop hint over Ready for Dev reads "Queue 2
  children" / "Queue SHOP-45.1" (or "Resume SHOP-45.1" for a parked row). **The "Epics stay in Backlog" feedback and its
  constant go** (`drag.rs`, `board_page.rs` tests).
- **The epic card is draggable** as a whole. **Each row has a grip on hover** for draggable rows (T-0267's flag); a row is
  dragged on its own (state 8).
- **A "not queued" chip** on a track shows **Queue** on hover (04 strip row) and can be dragged to Ready for Dev; both queue
  only that child (T-0263's single-child path).
- **Dropping** calls T-0263; the drop's outcome shows the toast from T-0272 ("Queued 2 children of SHOP-45 · Undo",
  "Queued SHOP-45.1 of Search · Undo", or the Auto-merge-off variant).
- **⇧⌘D** on a selected epic card or track queues it as a drop on Ready for Dev (0058).
- Not allowed: an epic card onto Review / Approved / Done, a row onto any column other than Ready for Dev / Dev; the existing
  "not allowed" feedback with the view model's reason.

**Done when**
- [ ] view-model tests for the drag answers and drop steps of the epic card, a row and a chip
- [ ] the old "Epics stay in Backlog" feedback is gone, with its tests
- [ ] matches flows 1–2 and states 8–9
  - *Hand check:* drag SHOP-45 to Ready for Dev (toast, track appears, two sleeved cards); undo it; drag only SHOP-45.1's
    row (track with "SHOP-45.2 · not queued"); hover the chip and press Queue; ⇧⌘D on SHOP-60; drag SHOP-45 onto Done (not
    allowed).
- [ ] checks green; the commit names EPC-12, EPC-14, EPC-15 and decisions 0080, 0079

### T-0271 · Take back: drag a track to Backlog, and the Park-all confirmation
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-14
- **Decisions:** 0080, 0052, 0066
- **Design:** 03 · Board & columns → epic flows 1 (0080) → 3; epic flows 4 (0080) → 11; Note · Interactions (Drag); the
  existing Park confirmation dialog (05 / 03, 0052) for the dialog's shape
- **Depends on:** T-0264, T-0270
- **Size:** M
- **Shared files:** `otaq-view` `board_page.rs`, `drag.rs`; `src/api/board.rs`; `Board.tsx`, `drag.ts`; `strip/`

**What and why**
- **The track name is its handle.** Dragging it shows Backlog as the one accepting column, with T-0264's hint ("Unqueue 1
  never-started child (45.2)" / "SHOP-45.1 keeps running", or "Park 2 running children"); Ready for Dev / Dev accept too
  and queue the rest (T-0263, mode Whole). Other columns: not allowed.
- **Drop on Backlog:** unqueue → done, no dialog; the track stays or turns back into the card (T-0262's place). **Park all**
  → the confirmation "Park 2 running tickets?" / "Their work is kept. SHOP-45.1 and SHOP-45.2 stop and go back to Backlog
  with their conversations." / Cancel (default) / "Park 2 tickets", built like the existing Park confirmation; on confirm
  the card comes back with "Parked · has work" rows.
- **A single child card** dragged from Ready for Dev back to Backlog works as today (0066) and becomes a "not queued"
  chip (state 10's "Drag it to Backlog to take it out").
- Dragging the epic card (with parked rows) to Ready for Dev again resumes them: T-0270's path, nothing new here.

**Done when**
- [ ] view-model tests for the track's drag answer and both drop paths
- [ ] matches flow 3 and state 11
  - *Hand check:* with SHOP-45.1 running and 45.2 queued, drag the track to Backlog (45.2 back as a chip, track stays);
    with every child started, drag it to Backlog (dialog; Cancel leaves everything running; confirm parks both, the card
    shows "Parked · has work"); drag the card to Ready for Dev (both resume their conversations; needs a real `claude`).
- [ ] checks green; the commit names EPC-14 and decisions 0080, 0052, 0079

### T-0272 · The queue toast: Undo, and the Auto-merge-off variant
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-15
- **Decisions:** 0080, 0064
- **Design:** 03 · Board & columns → epic flows 1 (0080) → 2 (toast, **without "⌘Z"**); epic flows 3 (0080) → 9 (one child);
  epic flows 4 (0080) → 12 (Auto-merge off); 01 · Components → Toast
- **Depends on:** T-0263, T-0264
- **Size:** M
- **Shared files:** creates `otaq-view/src/view_models/epic_toast.rs`; `toasts.rs` (an action link kind);
  `src/api/notices.rs`; `ui/src/components/Toast.tsx`, `components.css`; one Gallery section

**What and why**
- **Wording** (view model, from T-0263's outcome):
  - several children: "Queued 2 children of SHOP-45" · **Undo**;
  - one child: "Queued SHOP-45.1 of Search" · **Undo**;
  - resumed parked children count as queued in the number ("Queued 2 children of SHOP-45");
  - **Auto-merge Off** for the project: "Queued 3 tickets. Auto-merge is off for shop-web, so each one waits for your
    review." · **Turn on for epic children** · **Undo**.
- **Undo** calls T-0264's Undo with the outcome's keys. When something had started, the toast is replaced by "SHOP-45.1
  already started" (a success toast naming the rest that was taken back, e.g. "Took back SHOP-45.2 · SHOP-45.1 already
  started"). **No ⌘Z** (0080).
- **Turn on for epic children** sets the project's Auto-merge override to **Epic children only** (0064's existing setting
  write) and the toast confirms "Auto-merge: epic children only for shop-web". *Short-term (0080): keep it to this one
  button and this one setting write; no new settings UI.*
- `ToastLink` (or a new field) gains an **action** kind; `Toast.tsx` draws one or two action buttons as the frames show.
  The toast's existing rules apply (auto-hide 6 s, hover pauses).

**Done when**
- [ ] view-model tests for the four wordings and the Undo reply
- [ ] Turn on writes the project override (a test through the service)
- [ ] matches the toasts in flows 2, 9 and 12
  - *Hand check:* queue SHOP-45 in shop-web with Auto-merge Off (variant toast), press Turn on (Settings → shop-web shows
    "Epic children only"); queue SHOP-60, Undo; queue SHOP-40, wait for 40.1 to start, Undo ("SHOP-40.1 already started").
- [ ] checks green; the commit names EPC-15 and decisions 0080, 0064, 0079

### T-0273 · The folded Done line
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-16
- **Decisions:** 0080, 0044
- **Design:** 01 · Components · Epics on the board (0080) → Done Fold; 04 · Ticket card variants → Row · epic children &
  Done (0080) → Done · merged fold · a closed child stays a card, Done · fold opened; 03 · Board & columns → epic flows 1
  (0080) → 4; the 1280 × 800 note ("Done rail")
- **Depends on:** T-0267, T-0271
- **Size:** S
- **Shared files:** `otaq-view` `board.rs`, `board_page.rs` (Done column); `ui/src/screens/board/Column.tsx`, `board.css`

**What and why**
- In Done, an epic's **merged** children fold into one line per epic as soon as the first merges: "Wishlist · 4 of 6
  merged" (merged out of all children). Clicking it shows the cards (opened state); clicking again folds. The opened state
  is screen-only (not saved).
- **Closed children stay as their own cards** ("Closed · not merged · Reopen").
- The fold sits where the first merged child would sort. The column's count counts the fold as one item.
- **Done rail** (1280, or the panel open): the Done station collapses with it and shows "n merged".

**Done when**
- [ ] view-model tests: two merged and one closed child give one fold line and one card; the label; the count
- [ ] matches the two Done frames and flow 4
  - *Hand check:* SHOP-40 with 40.1 and 40.2 merged and 40.5 closed; open and close the fold; at 1280 × 800.
- [ ] checks green; the commit names EPC-16 and decisions 0080, 0079

### T-0274 · Focus an epic: track name, ⇧⌘G, Esc, ↑↓, and the epic menu
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-18, EPC-10 (focus), EPC-17 (Pin epic)
- **Decisions:** 0080 (with Q55), 0058, 0047, 0044
- **Design:** 03 · Board & columns → epic flows 2 (0080) → 6 (focus); 04 strip row → Focus; Note · Interactions (Keys,
  Right-click)
- **Depends on:** T-0269, T-0271
- **Size:** M
- **Shared files:** `otaq-view` `keymap.rs`, `filter.rs`, `card_menu.rs`, `keyboard_map.rs`; `ui/src/app/keys.ts`;
  `ui/src/screens/board/Board.tsx`, `filter.ts`, `CardMenu.tsx`; `strip/`

**What and why**
- **Focus = the epic filter** (0080: "The epic filter stays and is the same as focusing an epic"). Clicking a track name,
  or **⇧⌘G / Ctrl+Shift+G** (also from the terminal column) on a selected card that is an epic child, an epic card or a
  track, puts `epic:Wishlist` in the filter bar. The epic's cards stay; **the other cards dim** (not hidden, so the board
  keeps its shape, flow 6). ↑↓ walks only the epic's cards. **Esc** leaves focus (clears that filter token).
  **Q55 (resolved):** an `epic:` filter token now dims the other cards instead of hiding them, whether typed or set by
  focus; other filter words still hide. Columns keep their counts of what is shown at full strength.
- **⇧⌘G** is added to the key map (`keymap.rs`, the keyboard-map screen) and to the webview's reserved keys. On a standalone
  ticket it does nothing.
- **Right-click on a track or an epic card** (Note · Interactions): Open epic · **Queue never-started (n)** / **Unqueue
  never-started (n)** (T-0263 / T-0264) · Add ticket to epic · Focus (⇧⌘G) · **Pin epic** / Unpin · Close epic…. The
  existing items keep their dialogs.
- Pinning from the track pins the epic (0047, T-0262 orders it).

**Done when**
- [ ] tests: the key map has ⇧⌘G / Ctrl+Shift+G on all three rows and no duplicate (0058's rule); the filter view model
  dims for an epic token; the menu's items and counts for a card, a track and a pinned epic
- [ ] matches flow 6 and the 04 Focus strip
  - *Hand check:* click Wishlist's name; ⇧⌘G on SHOP-40.2 and in the terminal column (Ctrl+Shift+G in the Windows VM);
    ↑↓ stays in the epic; Esc leaves; right-click a track and pin it (it moves first).
- [ ] checks green; the commit names EPC-10, EPC-17, EPC-18 and decisions 0080, 0058, 0079

### T-0275 · Hand-check pass and the Windows check
- **Epic:** M16 · Epics on the board
- **Requirements:** EPC-09 … EPC-18
- **Decisions:** 0080, 0079, 0062
- **Design:** every frame named in T-0266–T-0274
- **Depends on:** T-0262 – T-0274
- **Size:** M
- **Shared files:** `tickets/review-M16.md`, `scripts/README.md` (only if setup notes change)

**What and why**
- Prepare every hand check of M16 in `tickets/review-M16.md`, with steps and frame names, grouped by screen (cards, strip,
  drags and dialogs, toasts, Done, keys). Agents don't tick hand checks.
- Sweep for what 0080 replaced: no epic chip, no "Epics stay in Backlog" / "Epics don't go to Dev" text, no child card in
  Backlog, anywhere in the code, the gallery or the tests.
- Run `scripts/check.ps1` in the **Windows VM** (VMPal, `Z:\software-factory`) and try the strip, an epic drag and
  Ctrl+Shift+G there with WebView2. The same in the Ubuntu VM if it's reachable.
- A VM that can't be reached is **skipped** and recorded as "not checked", not a blocker (the M14 rule).
- Fix what fails in new commits that name the ticket they fix.

**Done when**
- [ ] `tickets/review-M16.md` lists every hand check, with "How it was run" (0079) and "Platforms checked"
- [ ] the sweep finds nothing left of the old rules
- [ ] `check.ps1` green in the Windows VM, or the VM is recorded as skipped
- [ ] checks green on the final tree; the commit names EPC-09…EPC-18 and decisions 0080, 0079

## Parallel lanes (M16)
**The rules are the Depends on lists and the shared files.** This table is only the earliest grouping. The orchestrator
decides the actual order (0079): at most 3 teammates at once; never two tickets in the same folder or file at once, except
the one-line registration files; tickets that change `package.json` or `Cargo.toml` run alone (none should).

| Phase | Tickets (side by side within a phase) | Watch out for |
|---|---|---|
| 0 | T-0262 (alone) | the migration and the `Ticket` struct everything reads |
| 1 | T-0263 ∥ T-0265 ∥ T-0266 | all three change Rust, in different files (moves.rs / epic_flight.rs; tickets.rs / epics.rs; otaq-view card files); disk allows it |
| 2 | T-0264 (after T-0263) ∥ T-0267 ∥ T-0268 | T-0264 and T-0263 share `epic_flight.rs`, so T-0264 waits; T-0267 and T-0268 are different `otaq-view` files |
| 3 | T-0269 (after T-0267, T-0268) ∥ T-0272 (after T-0263, T-0264) | T-0269 owns `Board.tsx` and `board_page.rs` now; T-0272 is toasts and notices only |
| 4 | T-0270 | `Board.tsx`, `drag.ts`, `board_page.rs`, `EpicCard.tsx` |
| 5 | T-0271 | the same files as T-0270 |
| 6 | T-0273 ∥ T-0274 | T-0273 is `Column.tsx` / Done in `board.rs`; T-0274 is keys, filter, menu and `Board.tsx` (check `board.css` overlap first) |
| 7 | T-0275 (alone) | the VMs |

The long chain is T-0262 → T-0263 → T-0264 → T-0272 → T-0270 → T-0271 → T-0274, with T-0267 / T-0268 → T-0269 feeding
T-0270. Start what unblocks the most: T-0263 first in phase 1, T-0264 first in phase 2.
