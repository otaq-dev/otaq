# M14 · Tauri UI — review

> **DRAFT.** Written while M14 is still being built, from the commits on `main` up to 9440591 (T-0245). Still to
> come: **T-0246**, **T-0206** and **T-0247**. Each has a place marked **TO COMPLETE** below. The last M14 teammate
> completes this file: fills those places, adds the final commit hashes and test counts, re-reads the hand checks
> against the final tree, and removes this note.

Decisions 0074 and 0075, SDD §5.5 (UIT-01…UIT-11). The desktop UI is rebuilt on Tauri 2 with React, TypeScript and Vite
(`crates/crewboard-desktop`); the view models moved into `crates/crewboard-view`, shared with the GPUI app until T-0245
removed it. The test build (UIT-10, branch `m14-test-build`, never merged) came first; the user said go on 2026-10-12.

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0207 | The `crewboard-view` crate: the view models move out of the GPUI app | d4171b3 |
| T-0208 | `crewboard-desktop`: Tauri 2, React, Vite, TypeScript, the one Tauri door and the TypeScript rules | e20dc1f |
| T-0208 (follow-up) | `THIRD-PARTY-LICENSES.md` regenerated for the Tauri crates | ad6dc28 |
| T-0209 | Shared types: `ts-rs` generation and the freshness check | 3e3ad9e |
| T-0210 | `check.sh` and `check.ps1`: the frontend steps | cb91c0b |
| T-0211 | Service connection, requests and the event stream | 05ad12b |
| T-0212 | The app frame: layout slots and the registries screens plug into | 027761e |
| T-0213 | Design tokens, fonts, icons and the 15 themes as CSS variables | 332354d |
| T-0214 | Window chrome: the app's own title bar on every platform | 8c9d1b6 |
| T-0215 | The shortcut map, key routing and the webview's own keys switched off | 8ac918e |
| T-0216 | Terminal component: xterm.js, attach with replay, input, and the renderer setting | 18967c8 |
| T-0216 (follow-up) | A quick reconnect no longer leaves a terminal silent; two flaky tests made deterministic | 86e1e26 |
| T-0216 (follow-up) | A second attach on one connection keeps the running feed | 1e10b4c |
| T-0217 | Components and the design gallery (01) | dca1498 |
| T-0217 (follow-up) | Menus, dialogs and popovers keep ↵ and Esc from the key dispatcher | c6a5149 |
| T-0218 | Ticket Card and Epic Card, every variant (04) | 6f79d1b |
| T-0219 | App shell: header and project switcher (02) | 74ea6e9 |
| T-0220 | Board: columns, order, fold, Done rail, empty states and drag and drop (03) | cd3fc86 |
| T-0221 | Board: search and filter, card menu, pin, priority and delete (03) | cdab522 |
| T-0222 | Side panel: frame, header, tabs, navigation and the action bar (05) | 84136a5 |
| T-0222 (follow-up) | The panel picks up card actions | 0b8e560 |
| T-0223 | Panel sections: Stack, History, Messages and the epic panel (05) | a55d8d0 |
| T-0224 | Terminal tab (06) | c2a468a |
| T-0225 | Diff tab: file list, unified and side-by-side, line comments (07) | a4d1cb6 |
| T-0226 | Review: Send back, Approve ▾, Local merge, changed since review (07) | e4f0795 |
| T-0227 | Open PR, Push to PR, PR open state and epic delivery (07) | 98a5777 |
| T-0228 | Ticket editor and the Ticket tab (08) | 9b8b75d |
| T-0229 | Dependencies end to end: chain highlight and the storyboards (12) | 55b7320 |
| T-0230 | Needs-you inbox and ⌘J (10) | cac124a |
| T-0231 | Banners, toasts, inline errors and notification clicks (11) | d097628 |
| T-0231 (follow-up) | The rail, board and panel dim while the service can't take input | c784256 |
| T-0212 / T-0231 (follow-up) | Banner row placement per *Usage limit banner active* | 065b251 |
| T-0232 | Run tab, Open URL, run-command dialog, processes and the Shell tab (13) | e475923 |
| T-0232 (follow-up) | A validating ticket's panel offers Run too | 074bace |
| T-0233 | First run and onboarding (14) | 5257757 |
| T-0234 | Settings, global and per project (15) | f05a5fd |
| T-0235 | Recovery and Cleanup (16) | 603d679 |
| T-0236 | Panel: validation actions, "…" menu, confirmations, token split, editing while it validates | 63566e4 |
| T-0237 | Terminal tab: the Agent / Validator switch (06) | 4786b42 |
| T-0237 (follow-up) | The panel's Validator buttons land on the Validator side; seam diagnostics in T-0216's mid-stream test | 79f80ad |
| T-0238 | Validation reports in History and Review; Create ticket from a note | 30ba2cf |
| T-0239 | Inbox, OS notifications and toasts for validation and Auto-merge (10, 11) | 32ea4c5 |
| T-0240 | Settings: Validation & Auto-merge, Project agent instructions; the onboarding line | a9d6904 |
| T-0241 | Project agent items in the inbox and their notifications | fbc89bf |
| T-0242 | Terminals & agents: header button, drawer, tabs, + menu, ⇧⌘T / ⇧⌘A | bb8f2a9 |
| T-0183 (follow-up) | The validator test waits for the stand-in's last line (the commit says T-0185; it is T-0183's test) | 2b205b0 |
| T-0243 | Project agent views: New agent, rename, close, Created and changed, recently ended | 8b947ea |
| T-0244 | After a restart: validating tickets and Project agents in the summary | 1a76808 |
| T-0245 | Remove the GPUI app; `crewboard-desktop` becomes `crewboard-app` | e1dfd92 |
| T-0246 | Windows and Linux: checks on the Tauri UI (local VMs) | 5f21d73 |
| T-0206 | M12 checks on Windows and Linux (local VMs) | **TO COMPLETE** |
| T-0247 | Hand checks on Windows and Linux, and one run on real Linux hardware | **TO COMPLETE** |

One commit per ticket plus a "progress row" commit with its hash. The tickets were built in parallel lanes
(`m14-lane-a`, `m14-lane-b`, `m14-lane-c`/`-c2`), each in its own worktree with its own `target/`, merged into `main`
fast-forward only; every commit was checked with `scripts/check.sh` on macOS on exactly the staged state. T-0210's
first full green run counted **1035 tests passed**; T-0245's run counted **1216 passed, 0 failed**. **TO COMPLETE:**
the final M14 tree's `scripts/check.sh` result and test counts, and the Windows / Linux counts from T-0246.

## 2. How to try it

Node 24 comes from nvm on the Mac; nothing is installed globally (the Tauri CLI is a dev dependency). The app starts
the service itself from the `crewboard` binary next to it, so build the workspace once first. Use a **short**
`CREWBOARD_HOME`: macOS limits a socket path to 104 bytes, and a long data folder makes the service fail to start
(the app then says so in the red banner, Q40).

```sh
export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"   # or: nvm use (reads crates/crewboard-desktop/.nvmrc)
scripts/check.sh                          # frontend steps, then fmt, clippy -D warnings, tests
cargo build --workspace                   # the crewboard CLI / service the app starts
cd crates/crewboard-desktop
npm ci
rm -rf /tmp/cbX && CREWBOARD_HOME=/tmp/cbX npm run tauri dev    # fresh data folder: opens on onboarding
```

- **Gallery** (components, cards, tokens, keyboard map, a palette switch over the 15 themes):
  `CREWBOARD_GALLERY=1 CREWBOARD_HOME=/tmp/cbX npm run tauri dev`. Closing the gallery restores the stored theme.
- **Key log:** `CREWBOARD_KEY_LOG=1 CREWBOARD_HOME=/tmp/cbX npm run tauri dev` shows the last 12 keys and where each
  went, bottom right.
- **Open a ticket at start:** `CREWBOARD_OPEN=SHOP-22 CREWBOARD_HOME=/tmp/cbX npm run tauri dev`.
- **Release build:** only through the Tauri CLI, `npm run tauri build` (it builds the page and turns on Tauri's
  `custom-protocol`; the binary is `target/release/crewboard-app`, next to `crewboard` after
  `cargo build --release -p crewboard-cli`, so `target/release/crewboard` with no arguments opens it). A plain `cargo build --release` binary still
  loads the Vite dev URL.
- **The service by hand:** `../../target/debug/crewboard daemon stop` / `start` / `restart` with the same
  `CREWBOARD_HOME` (several hand checks stop it on purpose).
- **Sample data:** the tickets of `design-briefs/00-README.md` (SHOP-17…SHOP-21) created with `crewboard new` and
  `crewboard dep add`; states the service can't reach by hand with the SQL of `review-M6.md`, `review-M7.md`,
  `review-M8.md` and `review-M12-core.md` against `$CREWBOARD_HOME/crewboard.db`; a few hundred cards with
  `for i in $(seq 300); do crewboard new "Sample $i"; done`.
- **Temporary repositories:** `git init` a small repo under `/tmp` and add it as a project (shop-web, crewboard); for
  the PR checks, an `origin` whose push URL is a bare repository (`git remote set-url --push origin /tmp/bare.git`).
- **Regenerating** (never by hand): `cargo test -p crewboard-desktop regenerate_types -- --ignored` (the `ts-rs`
  types in `ui/src/generated/`) and `cargo test -p crewboard-desktop regenerate_stylesheet -- --ignored` (the theme
  CSS in `ui/src/theme/generated/`).

**The test build, in short** (`git show m14-test-build:m14-test-build-report.md`): a throwaway Tauri app on a real
service with 300 tickets and ten terminals taking about 350 KiB/s (two redrawing the full screen 30 times a second).
macOS (M4 Max): 99.9 fps while streaming, 100 with board scroll and drag at once, typing echo 19 ms median, 72 % of one
core, 427 MB, launch to board 394 ms. Windows 11 VM: 58.6 / 58.5 fps (60 Hz ceiling), 28 ms echo. Ubuntu VM: DOM
renderer 53 / 31.5 fps and 45 ms echo; WebGL on software GL 64 / 49 fps at 3.5 cores; WebGL on the VM's GPU drew
garbage without any error, and the window froze until `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Hence Q37 (DOM default on
Linux), Q39 (the DMABUF variable) and the real-hardware run in T-0247. The macOS real-key check it left open is in §3
under "Keys and window chrome".

## 3. Hand checks

Compare the running app with the named frames in `crewboard.pen`, first in **Crewboard Light**, then in **Crewboard
Dark** against the frames' Dark copies (a dark theme, d0070). Each item names its ticket; the full steps are under that
ticket's unticked boxes in `tickets/v0.1-tickets.md` ("M14 · Tauri UI"). macOS only; Windows and Linux are T-0246 /
T-0247. Every screen ticket also redoes the earlier tickets' hand checks it names (listed per item).

### App frame and themes

- [ ] **T-0212** `02 · App shell & project switcher → Window anatomy`, `Annotations`: before the screens filled it,
  the frame's slots; now the filled window at 1440 × 900 and at the 1280 × 800 minimum: title bar / header row, rail,
  banner row, board, panel slot; the window itself never shows a scrollbar (board and panel scroll inside).
- [ ] **T-0212 / T-0231** `02 → Usage limit banner active`: stop the service (`crewboard daemon stop`): the banner row
  spans the whole window under the header and pushes the rail, board and panel down.
- [ ] **T-0213** `01 · Foundations → Color Tokens`: the gallery's token swatches for crewboard-light and crewboard-dark,
  every token and `ansi-0`…`ansi-15`; `01 · Foundations → Type · Spacing · Icons`: type scale, spacing, icons (Lucide,
  14 px, 1.5 stroke), Geist / Geist Mono loaded with the network off.
- [ ] **T-0213** a fresh data folder starts in Crewboard Light or Dark as macOS is, once; a later macOS switch doesn't
  change it (d0070); changing the theme in Settings recolours every open window live, terminals included.

### Keys and window chrome

- [ ] **T-0214** `02 → Title bar · macOS`: the traffic lights inset in the header row (their vertical offset, 16 px, is
  a guess; compare closely), the 80 px start padding. Dragging the header moves the window; double-click zooms (on
  mouse up); buttons and the search field in the header keep their clicks and never move the window.
- [ ] **T-0214** the window can't be made smaller than 1280 × 800; quit and reopen: same size and place; maximized
  reopens maximized; a place on a screen that's gone reopens centered. (`Title bar · Windows & Linux` is T-0247.)
- [ ] **T-0215 (test build's open check)** with `CREWBOARD_KEY_LOG=1`: press ⌘K, ⌘P, ⌘J, ⇧⌘I, ⌘F in a terminal and ⌘R;
  each reaches the app (listed in the key log) and ⌘R doesn't reload.
- [ ] **T-0215** every §21.3 row reaches its handler on macOS (key log); F5, ⌘R, ⌘=, ⌘P, ⌘F outside a terminal never
  reload, zoom, print or open a find bar; Tauri's own macOS menu keeps ⌘Q, ⌘W, ⌘H, ⌘M, ⌃⌘F and the Edit keys.
- [ ] **T-0215 / T-0216** in a focused terminal: plain ⌃-letter keys (⌃C, ⌃R, ⌃L) go to the shell; the ⌘ column
  (⌘F, ⌘J, ⇧⌘I, ⇧⌘T) reaches the app; ⇧⌘` leaves the terminal. Redoes T-0058's routing.
- [ ] **T-0215 / T-0242** `01 · Components → Keyboard map` (+ `M12 · Project agents`) in the gallery: every row, the
  "New Project agent ⇧⌘A · Ctrl+Shift+A · Ctrl+Shift+A" row and the M12 note.
- [ ] **T-0217 (follow-up)** in a confirm dialog ↵ presses the focused button (Cancel on a destructive one) and Esc
  closes; in an open menu ↑/↓ and ↵ choose and Esc closes, without the panel closing behind them.

### Gallery and components (`CREWBOARD_GALLERY=1`)

- [ ] **T-0217** `01 · Components → Primitives` (Status Pill, Meta Chip, Kbd, Button, Icon Button, Priority),
  `Buttons → Button matrix` (every variant and state, 2 px focus ring), `Badges & Inputs` (badges and pills, tabs and
  segmented, fields), `Overlays & feedback`, `Banners → Banner`, in Crewboard Light and Dark, then with the gallery's
  palette switch in a Catppuccin, Dracula and Solarized Light.
- [ ] **T-0217** `01 · Foundations → Status Legend` (with `Pills · M12 (0063–0066)`) and the Priority tier: icon +
  label on every pill, never colour alone.
- [ ] **T-0217** dialogs trap Tab focus and close with Esc; destructive dialogs default to Cancel; menus work by
  keyboard; tooltips and popovers stay inside the window and aren't clipped by a scrolling parent.
- [ ] **T-0217** `14 · First run & onboarding → 2 · Theme` (the picker in the gallery): a card click saves the theme
  and recolours every window; the palette switch above it recolours without saving.

### Cards

- [ ] **T-0218** `04 · Ticket card variants · Light → Variant grid` (every row) and `04 · … · Dark → Variant grid`:
  the gallery's Variant grid and the epic rows, in Crewboard Light and Dark (pill icon + label, callouts, meta row,
  buttons, outlines; selected / chain / dragging / hovered looks).
- [ ] **T-0218** `04 → Compact density` (Compare): the pill as its icon with the label as tooltip, one value per card.
- [ ] **T-0218** titles clamp at two lines (full title on hover); a long key and an External ID don't break the card;
  two card buttons ellipsize in 190 px (*Retry merge / Move to Review* overflows in the frame too). Redoes the card
  checks of T-0031, T-0032, T-0039, T-0063, T-0075, T-0083, T-0098, T-0106, T-0107, T-0108, T-0121, T-0124, T-0131,
  T-0150, T-0154, T-0166, T-0180, T-0192.
- [ ] **T-0222 (follow-up)** press each card button on the board: the panel opens and runs it (Resume on an
  Interrupted card, an option on an Asks-you card, Deliver now with typed text in the terminal asks first).

### App shell: header and project switcher

- [ ] **T-0219** `02 → Header states`: *agent limits ON + inline popover* (steppers save), *dollar estimate enabled*,
  *inbox clear* (quiet "Inbox"), *running chip tooltip*, *plan meter hover*; order usage, plan meter, agents, processes.
- [ ] **T-0219** `02 → One project`, `Another project needs you` (orange ring on the tile, counter), `Ten projects ·
  long names` (tooltips, clamping), `1280 × 800 · minimum`, `1920 × 1080 · sidebar expanded`, `Dark · 1920 × 1080`,
  and `03 · Board — shop-web · 1440 · Light → Header` (+ Dark). Two projects needed (review-M2's seed).
- [ ] **T-0219** `02 → Quick switcher · ⌘P`: ⌘P opens it, typing filters at once, ↑/↓ ↵, "· current", "2 running";
  ⌘1–⌘9 switch projects with real keys; switching and back restores each project's open ticket and tab. Redoes T-0033,
  T-0082 (header), T-0126 (header), T-0130, T-0171.

### Board

- [ ] **T-0220** `03 · Board — shop-web · 1440 · Light → Body → Board` (+ Dark) at 1440 × 900: six columns, icon,
  title, count; Ready for Dev's line; Approved's note while a PR-open ticket is there; Dev's pins and needs-you on top,
  "WORKING · VALIDATING N", the "N more · working" fold; Backlog's + and "Add ticket".
- [ ] **T-0220** `03 → Width strategy · recommended vs alternative`: at 1280 × 800 Done is the 34 px rail (click opens,
  ⇔ folds, kept per project); at 1920 × 1080 all six open. `Very full column · Dev, 13 cards, scrolled`: header stays,
  the "7 above · 6 need you" chip scrolls back. Switch projects and back: Done's choice, Dev's fold and scroll return.
- [ ] **T-0220** `03 → Empty column & empty board` and `14 → 4 · Done · empty board` on an empty project.
- [ ] **T-0220** `03 → Drag · over an allowed column`, `Drag · over a forbidden column`, `Epic can’t be dragged to
  Dev`, `Approve choice on drop`, `Park · Dev → Backlog (allowed)`, `Park · after, and back to Dev`, `PR rework ·
  Approved → Ready for Review (allowed)`: outlines, tint, the lifted card, the hint pill, the ban badge and reason;
  with `CREWBOARD_KEY_LOG=1` nothing is logged while the pointer moves. Keyboard: Space lifts, ←/→ move, Space drops,
  Esc cancels.
- [ ] **T-0220** ~300 tickets: scroll each column and drag across the board; no stutter. Redoes T-0035, T-0036,
  T-0107 (drag), T-0150 (PR rework drag), T-0179 and the drop part of T-0095.
- [ ] **T-0221** `03 → Search & filter · ⌘K`: ⌘K focuses the field, suggestions with counts, `is:needs-me ` becomes
  the orange chip, counts "shown/total" and "N hidden by filter", "N results"; ↑/↓ ↵, `is:wor` + ↵, × on a chip; Esc
  clears and leaves; the filter survives tab changes, not a project switch.
- [ ] **T-0221** `03 → Pinned tickets` (card right-click menu, also the menu key / ⇧F10): Pin / Unpin, P1…P4;
  Delete… greyed on a started ticket; `05 → Row · answer options & delete → Delete · never-started ticket`, `Delete
  ticket · confirm`, `Dark · options & delete`, and `Epic dialogs → Delete epic · confirm` (title keeps the ticket's
  title, Q42). Redoes T-0038, T-0039, T-0162 and the card-menu part of T-0180.
- [ ] **T-0229** `03 → Chain highlight · hover or select SHOP-20` and `Dark · chain highlight` with the sample tickets
  at 1440 × 900, Done folded: SHOP-20 2 px accent, members 1.5 px, the rest at 35 %, arrows only between members, the
  accent SHOP-17 chip on the Done rail, the legend bottom right; the above chip / Dev's fold turn accent when they hide
  a member; an unlinked card dims nothing; a drag removes the highlight.
- [ ] **T-0229** storyboards `12 · Stack & dependencies → 1 · Planning a chain` … `6 · Closed parent` played out with
  real tickets (waiting texts, the started callout "from main @ <sha> · all dependencies merged", Mark merged / Mark
  closed, the inbox's Reopen parent / Remove dependency).

### Side panel, editor, Run and Shell

- [ ] **T-0222** `05 · Ticket side panel →` *SHOP-18 · Ready for Review*, *SHOP-20 · Ready for Dev, waits for
  merge*, *SHOP-25 · Interrupted*, *SHOP-17 · Done*, *Long title & long branch* (3-line clamp, branch shortened in the
  middle), *Narrow panel · 480 px* (icon-only tabs under 560 px), *Expanded · ⇧⌘E*, *Action bar · every state*,
  *SHOP-18 · conflicts with main*, *SHOP-21 · Asks you · answer options* (1–4, ↵, Other…), `05/06 · Panel — SHOP-22
  Needs you · Terminal · Light / Dark`, `16 → Start interrupted · SHOP-28 panel`.
- [ ] **T-0222** ⇧⌘E, ⌘[ / ⌘] (disabled at the ends), ⌘⌥1–6, Esc; the edge drags 480–960 px (board keeps 420),
  double-click resets to 660, ←/→ on the focused edge, the width stays for the next ticket.
- [ ] **T-0222** `05 → Close ticket · panel “…” menu`, `Close ticket · confirm` (on a Working ticket), `Submit for
  review · dialog` (refusal inside it with uncommitted files), `Closed · not merged · Reopen`, `Dark · closed ticket`.
  Park is the board's drop (Q43). Redoes T-0059, T-0060, T-0063, T-0075, T-0098, T-0106, T-0166 (panel parts) and the
  header part of T-0180.
- [ ] **T-0223** `05 → SHOP-20 · Ready for Dev, waits for merge` (Stack, History) and `12 → Stack list · detail`: indent
  and `└`, the open ticket tinted, "waits for" lines; a key opens that ticket; `16 → History · Claude session IDs`.
- [ ] **T-0223** `05 → Messages · agent-to-agent (SHOP-40.2)`, `Message states`, `Dark · Messages` (two linked agents,
  real `claude`): a key opens the ticket; Answer and its reply box.
- [ ] **T-0223** `05 → Epic panel · SHOP-40 Wishlist` and `Epic dialogs` (Close epic, Move to epic), `08 → Move to epic
  · base changes (SHOP-51)`; a started ticket's menu has no Move to epic….
- [ ] **T-0228** `08 · Ticket editor → Quick-create in Backlog` (⌘N, Backlog's +; ↵ / Esc / ⇧↵), `Full form · filled`,
  `Dark · full form`, `Validation · empty form`, `Depends on · picker open`, `Cycle prevention`, `Row · base branch per
  ticket (0051)`, `Row · epics and base branches (0051)`.
- [ ] **T-0228** `08 → Ticket tab · read mode, with agent edits` (⌘⌥5), `Editing while the agent works` (the save
  hint, "Saved. … will be told what changed", Open terminal); with real `claude`, `Compare · before / after` marks the
  agent's edit seen.
- [ ] **T-0232** `13 · Run & Shell →` *Not started*, *1 · Empty* … *5 · Saved* (Ask Claude only on its click), *Starting*,
  *Running*, *Stopped*, *Failed to start*, *Port already in use*, *Stopped by restart*, *Reaching Done stops it*, *Card
  & panel-header controls*, *Run · backend (BILL-4) · no Open URL*, *All running processes · header menu*, *Shell tab ·
  BILL-4*, *Dark · Run tab*, *Dark · Run (backend)*, *Dark · Shell*, with `python3 -m http.server $PORT`.
- [ ] **T-0232** Open opens the substituted URL; Stop / Stop all; in the Shell tab `pwd` is the worktree and `echo
  $PORT` the Run port; Reopen after `exit`; Save writes Settings and nothing in the repo. Redoes T-0134–T-0137.
- [ ] **T-0236** `05 → Action bar · every state` rows *Validating…*, *Needs you · validator*, *Validation failed 3
  times*, *Validation ended without a result*, *Auto-merge stopped*, *Auto-merge stop · messages*; `05 → Row ·
  validation (0063) → SHOP-29 · Validating…`, `SHOP-32 · Validation failed 3 times`, `Close / Park · while validating
  (SHOP-29)` (+ Dark); each button changes the status as named; the "▾" beside Approve opens the choice.
- [ ] **T-0236** the header token split ("agent 58k · validation 9k (1 round)"); `08 → Editing while it validates
  (SHOP-29)` (+ Dark): amber warning, "unsaved · saving cancels the validation"; a title-only change doesn't cancel.

### Terminal

- [ ] **T-0216** in a terminal running `seq 1 5000`: ⌘F opens the find bar (`06 → Scrolled up · find in scrollback`),
  "1 of 1", ↵ / ⇧↵ / chevrons, Esc gives the keyboard back; scrolling up shows the "N new lines" pill; drag-select ⌘C,
  ⌘V pastes; ⌘+ / ⌘− change the size; a theme switch recolours it at once (`01 · Theme palettes → Terminal & diff`).
- [ ] **T-0216** a redraw-heavy program (`top`, or the test build's `cb_stream redraw 30 80 24`) against `06 → Live ·
  streaming, app has focus`; `crewboard daemon stop` against `Reconnecting to the background service` (scrollback
  stays, typing does nothing).
- [ ] **T-0216 / T-0234** Settings → Appearance → Terminal renderer: switching WebGL ↔ DOM redraws open terminals (DOM:
  the text is selectable in the inspector's element tree); macOS defaults to WebGL (Q37).
- [ ] **T-0224** `06 · Terminal tab →` *Live · streaming, app has focus*, *Answering a question (SHOP-23) · focused*,
  *Reconnecting…*, *Session ended normally*, *Scrolled up · find in scrollback*, *Narrow panel · 480 px*, *Interrupted
  · Before the restart*, *Working in background (SHOP-19)*, *Message waiting · unsent text (SHOP-19)*, *Ticket change
  delivered (SHOP-19)*, *1 · Focused, ended turn · stays calm*, *2 · Left without replying → Idle*, *Dark · live
  terminal*.
- [ ] **T-0224** the attach chip copies `crewboard attach <KEY>` (paste it in a shell: same session); "Instructions:
  <project>" opens Settings on the project's Agent instructions.
- [ ] **T-0224** typed text without Enter, then edit the ticket: "1 message waiting · Deliver now"; Deliver now asks
  *Unsent-text prompt (SHOP-23)*'s question; Cancel keeps the text, "Clear it and send" delivers.
- [ ] **T-0224 (real `claude`, once)** chat in a focused terminal until the agent asks: no orange, no inbox item, no
  notification, "your turn"; ⇧⌘` or closing the panel → Idle and in the inbox. Redoes T-0056, T-0057, T-0058, T-0061,
  T-0076, T-0121, T-0124 (terminal parts) and T-0170.
- [ ] **T-0237** `06 → Row · validator (0063) →` *Validator running (SHOP-29)*, *Validator waiting on a permission
  prompt (SHOP-33)*, *Validator interrupted (SHOP-29)* and `Row · validation results in the terminal (0063) →`
  *Validator ended with a verdict · read-only (SHOP-30)*, *Validation failed · message in the agent’s terminal
  (SHOP-30)*, *Validation cancelled · message in the agent’s terminal (SHOP-29)* (+ Dark rows); no switch before the
  first validation.
- [ ] **T-0237** typing into a running validator reaches it ("Typing goes to the validator"); an ended one is
  read-only until the next round; ⌘J or a notification click on Needs you · validator lands on the Validator side,
  focused; the panel's Validator buttons do too (follow-up 79f80ad).

### Diff and review

- [ ] **T-0225** `07 · Diff, review, approve & send back → Diff tab · SHOP-18 · unified (panel expanded)`, `Side-by-side`
  (unified again below 900 px), `Dark · diff tab`, `Annotations`; then Dracula and Solarized Light: a theme switch
  recolours the code at once.
- [ ] **T-0225** `07 → Large diff · 54 files` (folder groups, "Filter 54 files", generated collapsed, "Load diff",
  smooth scrolling) and `Binary, renamed, empty`.
- [ ] **T-0225** line comments on a Ready for Review ticket: the "+" on hover, ⌘↵ adds, Esc cancels, Reply threads,
  edit and delete pending ones, "2 pending comments"; a Dev or Done ticket shows threads read-only. Redoes T-0091,
  T-0092, T-0093 and T-0169's diff check.
- [ ] **T-0226** ⌘↵ (`03 → Approve choice on drop` rows) and ⌘⌫ (`07 → Send back · review & confirm`, "Send back
  (2)"); `Local merge · confirm`, `Local merge · rebasing onto main… (0038)`, `Local merge · success`, `Merge conflict
  during Local merge`, `Local merge · refused, uncommitted changes (0038)`, `Uncommitted files · refused`, `Approve ·
  changed since review (0043)`, `Base branch · SHOP-31 on release/2.3 (0051)` against a temporary repository.
- [ ] **T-0226** Send back reaches the agent with both comments and the note (the unsent-text question first); Local
  merge never runs before Squash & merge; dropping a Review card on Approved and pressing 1 opens the same
  confirmation. Redoes T-0094, T-0095, T-0098 (review part).
- [ ] **T-0227** `07 → Open PR · skill is preparing…`, `Open PR · confirmation (SHOP-18, 0050)`, `Open PR · invalid or
  taken branch name`, `Open PR · pushing`, `Open PR · success`, `Open PR · not logged in` (`gh auth logout`), with a
  bare test remote; Cancel before Push & open PR pushes nothing (`git ls-remote`).
- [ ] **T-0227** `05 → SHOP-26 · Approved, PR open` (the PR box, Mark merged / Mark closed); the PR chip through
  rework; `07 → Approve · Push to PR #481 (0041)`, `Push to PR · confirmation (branch locked, 0050)`, `Send back ·
  ticket with an Azure DevOps PR (BILL-5)`.
- [ ] **T-0227** `05 → Epic in Review · Diff by child (SHOP-40)`, `Approve ▾ on an epic`, `07 → Open PR · epic SHOP-40
  (PAY-123)`, `Epic child · Approve offers only Local merge`. Redoes T-0149, T-0150 (panel), T-0109 (review).
- [ ] **T-0238** `07 → Row · validation report & Auto-merge (0063, 0064) → SHOP-18 · Diff · Validated · round 2` and
  `Review · Failed 3 times · moved by you (SHOP-32)` (+ Dark): the report folds, a linked key opens its ticket;
  `05 → Row · validation (0063) → History · validation rounds (SHOP-32, 0063)` (+ Dark): a round opens its report;
  `Auto-merge · success (SHOP-40.1)` and `Auto-merge · conflict handed back vs manual Local merge`.
- [ ] **T-0238** Create ticket on a "Not covered by any ticket" note: one Backlog ticket in the source's epic and the
  toast `11 → Success toasts → Created SHOP-41 from a validation note`; the toast opens the new ticket.

### Inbox

- [ ] **T-0230** `10 · Needs-you inbox →` *Inbox popover · mixed, 2 projects* (440 px), *In context · 1440 window*,
  *One item*, *Empty · nothing needs you*, *Resolved elsewhere* (struck through, gone after 4 s), *Usage limit active ·
  no duplicates*, *Inbox count in header & switcher*, *Agent question for you*, *Answer · inline*, *Answer · options,
  no recommendation*, *Dark · agent question*, *Dark · answer with options*, *Resume all · confirm*, *Parent closed ·
  new item in group 2*, *Dark · parent closed*, *Row · Idle, after reboot*, *Dark · inbox*.
- [ ] **T-0230** ⇧⌘I toggles it (also from a focused terminal); storyboard `1 · ⌘J from anywhere`, `2 · Answered`,
  `3 · Next item`, `4 · All clear`: the terminal takes the keyboard, "Inbox · 1 of 7", "Done · 6 left", nothing moves
  until the next ⌘J, Exit; "All clear" for 3 s.
- [ ] **T-0230** an inline answer reaches the agent's terminal and the row leaves; with typed text the unsent-text
  question first. Resume all asks, then resumes; "This is the usage limit" folds the Idle items. Redoes T-0118, T-0119,
  T-0120, T-0131 (inbox), T-0132, T-0153 (inbox), T-0165.
- [ ] **T-0239** `10 → Row · M12 item types (0063, 0064, 0068) → New item types · group 1 (M12)` and `Dark · new item
  types (M12)`: heading "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · PROJECT AGENTS"; SHOP-33, SHOP-32, SHOP-34, SHOP-40.4
  rows with their buttons; group 3's "validated · round 2".
- [ ] **T-0239** each inline action once (Move to Review anyway, Give it another round, Answer the agent… with its note
  box, Retry validation, Retry merge, Move to Review); each row then resolves and leaves after 4 s.
- [ ] **T-0241** a Project agent asking (⇧⌘A, `crewboard ask human` with two options): its row against *New item types
  · group 1 (M12)* ("Import GitHub issues") and the Dark copy; Answer → pick → Send reaches the agent; with typed text
  "Clear it and send" names the agent; a click opens the drawer on that agent's tab.

### Banners, toasts and OS notifications

- [ ] **T-0231** `11 · Banners, toasts & OS notifications → Banners`: *Background service stopped* (red, Start service,
  rail and board dimmed and unclickable), then the *restarted* toast; *Background service reconnecting*; an over-long
  `CREWBOARD_HOME` → "Background service couldn’t start" with the socket-path reason (Q40); *Service update ready* (an
  older build's service); *Restart required*; *After the service restart*.
- [ ] **T-0231** the usage-limit banners (*just hit*, *waiting for reset* with the countdown, *after reset*, *weekly*,
  *reset time unknown*; the SQL of review-M8.md), *Stacking · usage limit + reconnecting* (one row), *Permission-mode
  warning · first run* (after onboarding).
- [ ] **T-0231** `Success toasts`, `Error toasts & inline errors`, `Dark · toasts & banner`: bottom right, at most three,
  success hides after 6 s and pauses on hover, errors stay. `Service restart · confirm` (480 px; Esc, Cancel, a click
  outside restart nothing).
- [ ] **T-0231** `11 → OS notifications` (macOS): with another window in front, click SHOP-22's notification: the
  window comes forward on its Terminal tab, focused; a grouped one opens the inbox; with the app quit, the click starts
  it on the ticket (the service starts `crewboard-app` next to it, the Tauri app since T-0245: use the release build
  of §2). Redoes T-0017, T-0131, T-0076, T-0122, T-0117, T-0159.
- [ ] **T-0239** the toasts "SHOP-40.1 merged automatically into the Wishlist branch" (merge icon) and "Validation
  cancelled: SHOP-29’s code changed" (ban icon), a click opens the ticket; `11 → OS notifications → macOS · M12` for the
  validator permission prompt, "failed validation 3 times", "ended without a result" (the validator's opens on the
  Validator side).
- [ ] **T-0241** `11 → OS notifications → macOS · M12`: "shop-web · Project agent asks a question" / "needs
  permission"; a click brings the app forward on the inbox.

### Settings

- [ ] **T-0234** `15 · Settings →` *Global · Agent limits OFF (default)*, *Global · Agent limits ON*, *Global · Usage,
  notifications, data, appearance*, *Global · Tools*, *Theme credits · dialog*, *Project · fresh (zero config works)*,
  *Project · configured + invalid value* (the service's reason under the field, red alert in the nav), *PR provider ·
  not detected (0050)*, *Dark · global settings*, *Dark · project settings (crewboard, M12)*; About rows excluded.
- [ ] **T-0234** each setting saves (text on blur / ↵, toggles and steppers at once) and applies live: theme,
  notifications, limits (header steppers follow).
- [ ] **T-0234** `15 → Remove project · confirm`: the project leaves the app; `git status` in the repo is unchanged.
  Redoes T-0045, T-0082 (settings), T-0117, T-0126, T-0139, T-0141, T-0153, T-0174.
- [ ] **T-0240** `15 → Global · Usage, notifications, data, appearance` (nav "Validation & Auto-merge", "Project
  agents"; Workflow skills → Validate; the VALIDATION & AUTO-MERGE and PROJECT AGENTS sections), `Dark · global
  settings`; crewboard with Validation off vs `Project · fresh` and `Dark · project settings (crewboard, M12)`; shop-web
  vs `Row · validation & Auto-merge per project → Project · shop-web · Validation & Auto-merge` (+ Dark).

### Onboarding

- [ ] **T-0233** fresh `CREWBOARD_HOME`: `14 · First run & onboarding → 1 · Welcome` / `Dark · welcome` (own title bar,
  five steps), `2 · Theme` / `Dark · 2 · Theme` (a click recolours at once and survives a restart; the first launch
  preselects Light or Dark as macOS is), `2 · Tool checklist · everything installed`, `3 · Add project · valid`, `Not
  a git repo`, `Already added`, `4 · Done · empty board` / `Dark · 4 · Done · empty board`.
- [ ] **T-0233** `PATH=/usr/bin:/bin … npm run tauri dev` → `claude missing · blocking` (Continue off; Re-check after
  putting it back); `gh and az missing · non-blocking`; each copy button; the permission-mode warning and its docs link.
- [ ] **T-0233** with every project removed: `02 → No projects yet` / `Dark · no projects` (Add project folder… → the
  system picker → the dialog; Check tools); a folder dropped on the window opens Add project; the rail's + and ⌘P →
  `14 → Adding a second project later`. Redoes T-0018, T-0034, T-0173 and the onboarding part of T-0172.
- [ ] **T-0240** the onboarding empty board vs `14 → 4 · Done · empty board` (+ Dark): the Project agent text, New
  ticket / Project agent / Project terminal, the hints, "Auto-merge is off. …"; Project agent opens one.

### Recovery and Cleanup

- [ ] **T-0235** `16 · Recovery & cleanup → Where Cleanup lives` (the rail's broom, dot, "Cleanup · 3 leftovers"),
  `Cleanup · 3 items`, `Cleanup · 20 items` (13+ leftovers: groups folded, "select all"), `Delete · confirm` (Cancel
  focused), `Success` (broom toast), `Cleanup · empty`, `Dark · cleanup`; `cleanup` + ↵ in the board search and
  Settings → Data → Cleanup open it too.
- [ ] **T-0235** `16 → Resume failed · Start from scratch` (Kept / Lost, History note; Cancel and Esc do nothing);
  `Start interrupted · SHOP-28 card` and `panel` (stop the service during a `sleep 30` setup command).
- [ ] **T-0235** stop the service while two tickets run an ordinary long command, reopen: the banner and the summary
  sheet vs `16 → Opening after a reboot` (rows ticked, "Resume selected (2)", Later); nothing resumes until a click;
  Resuming… → Resumed / Couldn't resume (Retry, Start from scratch); 7+ rows fold into "+N"; orange and Review tickets
  in the note box ("NOT IN RESUME SELECTED", Q44); a normal reopen shows only "Reconnected".
- [ ] **T-0244** restart the service while a ticket validates (ordinary command as the validator) and two Project
  agents run: `16 → Opening after a reboot`: the row "was Validating · Resume = fresh validation", the PROJECT AGENTS
  part ("NOT IN RESUME SELECTED · TERMINALS ARE GONE", Resume each), the discarded New agent line, the title and banner
  "N sessions were interrupted · M Project agents ended" (the agent count arrives a moment after the banner);
  `Row · validation & Project agents after a restart (0063, 0068) → Validating ticket after a restart (SHOP-29)`
  (+ Dark): the card's "Validation stopped by restart · Resume starts it fresh (not counted)". Known: a restart that
  ended only Project agents raises no banner or sheet.

### Terminals & agents

- [ ] **T-0242** `02 → Row · Terminals & agents button & switcher (0068) → Terminals & agents button · a Project agent
  waits` (+ Dark): the button between New ticket and Settings, its tooltip ("Terminals & agents ⇧⌘T", "New Project
  agent ⇧⌘A"), the orange dot and its line; focusing the agent's terminal removes the dot.
- [ ] **T-0242** + → New terminal twice, + → New Project agent (real `claude`); agents before terminals; × on a terminal
  tab ends its shell; each project keeps its own tabs; hiding keeps shells running; `18 → Row · lifecycle → + menu ·
  New terminal / New Project agent` (+ Dark).
- [ ] **T-0242** ⇧⌘T opens "Terminal 1" focused, later the last-used one; ⇧⌘A opens a "New agent" tab; both work from
  a focused terminal.
- [ ] **T-0242** `17 · Project terminals →` *Empty · shell prompt* (380 px drawer, the board ends above it), *Plain shell
  use*, *Ended*, *Cycle rejected by the tool*, *Adding a ticket to an epic (0039)*, *After a restart · terminals are
  gone*. Redoes T-0138.
- [ ] **T-0243** with the real `claude`: `18 · Project agents & terminals → Row · investigating`, `Row · importing`,
  `Row · dark`, `Row · states` (+ Dark; the pill per state, Idle has no frame), `Row · lifecycle` (+ Dark), and
  `04 → Backlog · created by Project agent` ("by Project agent “<name>”", violet glow for 8 s); Rename (button and
  double-click), close idle (no dialog) and mid-turn (Close mid-turn's dialog), Resume, Recently ended popover; Created
  and changed updates live and its Delete opens the 0060 dialog ("Project agent “X” created it and it never started.").

### After T-0245 (the GPUI app is gone)

- [ ] **T-0207 / T-0245** the app launched by `crewboard` is the Tauri one (replaces T-0207's "the GPUI app builds,
  starts and behaves as before"): `npm run tauri build` in `crates/crewboard-desktop` and `cargo build --release -p
  crewboard-cli`, then `rm -rf /tmp/cbR && CREWBOARD_HOME=/tmp/cbR target/release/crewboard`: the Tauri window opens
  without the Vite server and starts the service; the board, a ticket's panel and its terminal work; with the app quit,
  a notification click starts it on the ticket.

## 4. Implementation choices (SDD §26)

**Structure and tooling**
- **Two new crates.** `crewboard-view` (no GPUI, Tauri or alacritty) holds all 66 view-model files with their tests,
  the keymap table, the theme data, the icon names and the diff highlighting; the frozen GPUI app depended on it
  (until T-0245) and only changed `use` lines (Q41: everything moved at once, so screens could be built side by side).
  `crewboard-desktop` is the Tauri app; its page sits inside the crate (`package.json` next to `Cargo.toml`, the page in `ui/`, built into
  `ui/dist`).
- **The one door (UIT-02):** `src/door.rs` and `ui/src/tauri.ts` are the only places naming Tauri (an ESLint rule and
  the Rust test `tests/one_door.rs` enforce it). The door has a generic surface: one request call, one event stream,
  one terminal channel per session, the window calls, the folder picker.
- **Requests and screens:** a non-Tauri request table (`src/api/mod.rs`, one line per area, one file per area); a page
  frame with typed registries (`ui/src/app/registry/`: slots, overlays with a fixed id list, panel tabs and sections,
  settings sections, shortcut handlers; one owner per id); one line per screen in `ui/src/screens/index.ts`; the
  recipe in the crate's README ("How a screen plugs in").
- **Logic stays in Rust (UIT-04):** wording and rules are never rewritten in TypeScript; each screen gets a `*_page.rs`
  view model in `crewboard-view` and a request area. Dialog state that spans requests lives in the desktop `App`
  (switcher rings, PR dialogs, run-command dialogs, inbox fading, review memory, drawer tabs, cleanup scan).
- **Shared types** are generated with `ts-rs` 12 into `ui/src/generated/` (one file per type, `export_all` in
  `src/generated.rs`); a freshness test and a named `check.sh` step fail on stale files; 64-bit integers are numbers;
  generic names get a page name (`TerminalPhase`, `KeyModifiers`).
- **Tool versions:** TypeScript 6.0 and ESLint 9.39 (TypeScript 7 and ESLint 10 aren't supported by the plugins yet);
  the ESLint config in TypeScript via the experimental `unstable_native_nodejs_ts_config` flag;
  `eslint-plugin-react-hooks` 7's flat preset, which also turns on the React Compiler rules.
- **No UI packages:** menus, popovers, tooltips, the dialog focus trap and the drag are hand-built; Vitest runs in node
  without DOM packages (component rules are plain functions; markup through `react-dom/server`). New packages:
  React 19, Vite, the xterm.js 6 packages (WebGL, search, web-links, fit add-ons). New crates: `tauri` 2.12,
  `tauri-build`, `tauri-plugin-dialog` (the folder picker), `ts-rs`, and on Windows only `webview2-com` /
  `windows-core`.
- **`check.sh` / `check.ps1`** run node (version from `.nvmrc`), npm-ci, types, tsc, eslint, prettier, vitest, page,
  then fmt, clippy, test. A plain cargo build is Tauri's dev flavour and needs no built page; release builds only
  through `npm run tauri build`. The Linux VM, like the Windows one, runs checks on a copy in its home folder.
- **Bundle identifier** `io.github.owner.crewboard` (0072's placeholder) until the release work.

**Platform**
- **Window:** built in code by the door; macOS keeps inset traffic lights (overlay title bar; their offset 14, 16 is a
  guess), Windows and Linux draw their own caption buttons. Size and place in `<data folder>/window.json` (no
  window-state plugin, which writes outside the data folder).
- **Linux:** `WEBKIT_DISABLE_DMABUF_RENDERER=1` set before the webview unless already set (Q39, option 1); the terminal
  renderer defaults to DOM on Linux, WebGL elsewhere (Q37, option 1), a global setting with a row under Appearance
  (Q38, option 1).
- **Keys:** one dispatcher driven by `crewboard-view`'s keymap (fetched with `keymap.map`; no TypeScript file lists a
  shortcut); webview keys (reload, print, find, zoom, devtools, caret browsing, history, save, open, view source) are
  swallowed; WebView2's accelerator keys are switched off in the door (compiled and checked in the Windows VM by
  T-0246); a focused menu, dialog or text field keeps ↵ and Esc.
- **Service:** a long socket path fails at once with a reason in the red banner (Q40); a new core event
  `settings_changed`; the link resyncs on first connect too.
- **Terminal:** attach with replay over a JSON channel with base64 data (fast enough in the test build); 10,000 lines
  of scrollback; WebGL falls back to DOM only when the context fails. Two follow-ups fixed real bugs: a quick reconnect
  left a terminal silent (86e1e26), and a second attach of one session on the same connection lost or repeated chunks
  (1e10b4c, a service-side fix).

**Screens**
- **Themes:** CSS variables generated from the Rust palettes (`[data-theme="<key>"]` blocks, keys = stored setting
  values); fonts (Geist, OFL) and Lucide icons bundled, no network; icons drawn as CSS masks in `currentColor`.
- **Board:** one `board.drag` per drag (after 4 px), then only the page works (moved by transforms, no re-render);
  Done's choice, Dev's fold and scroll kept per project while the app runs; drop hints for Unqueue / Park / Approve /
  Reopen worded in Rust where the frames show none; the chain data comes up front with the board so a hover never
  waits.
- **Search:** parsing and matching stay in Rust; the page takes Rust's chips back only when a token finished and the
  user hasn't typed on, so typing never lags.
- **Diff highlighting** is Rust's (syntect spans coloured by theme variables), so a theme switch recolours with no new
  request; large files load as they come near and render in 120-row chunks.
- **Panel:** buttons owned by other tickets open their overlays by id; the tab is kept when stepping between tickets;
  the epic panel is a section above the tabs; the PR box and the inbox run bar are panel sections.
- **Quick-create** floats over the top of the Backlog column (an overlay), not a card inside it.
- **Settings:** follows the frames' wording where it differs from the GPUI app; a set project value gets a "Use
  global" link (not in the frames; otherwise a project could never go back to the global value).
- **Validation:** `lands_on` (Validator while validating or waiting on you) decides both the side the Terminal tab opens
  on and where ⌘J / notification focus lands; a finished validator's chip copies the agent's attach command.
- **Recovery (T-0235):** a new after-restart summary sheet opens once per service start that interrupted sessions, and
  from the banner's Review & resume.

**Known differences from the frames, and things not drawn**
- Leave terminal focus is ⇧⌘` (the frames' hint says Esc; Esc goes to the CLI by design).
- Terminal tab: no "⌘C copy" tip; the background label has no "(2)" count; the token split on the end marker and the
  "Validated · round 2" line aren't drawn.
- Board: the edge chips naming columns scrolled out of view aren't drawn; the chain legend sits at a fixed place and can
  cover Done cards while open.
- Delete dialog title keeps the ticket's title, "Delete SHOP-21 Order confirmation email?" (Q42; the 05 frames say
  "Delete SHOP-21?").
- Diff: the frame-07 annotation keys (V, [ ], U, C) aren't built (no keymap actions; the GPUI app had none);
  uncommitted / dirty lists show bare paths without git status letters; the Merging steps don't tick one by one.
- PR: the success toast has no "↗" link; the Push intro has no "+18 −6" line counts; no cancel while pushing.
- Run: the card's `▶ :4020` chip opens the Run tab instead of an Open / Stop menu; the Run-stopped toast uses the
  service's wording.
- Inbox: Idle rows have no extra Open button (the row opens on click); the review row's "+412 −38 · 9 files" isn't
  drawn (the service doesn't send it); Project agent rows don't fade when resolved and aren't part of the ⌘J run; a
  Project agent notification opens the inbox (the core sends no target).
- Onboarding: the Remote / PR provider rows of *3 · Add project* aren't drawn (the folder check doesn't report the
  remote).
- Ticket tab: an Edit button in the footer (not in the frames); read mode shows Title only when an agent changed it.
- Terminals & agents: no resize grip on the drawer (380 px or maximized); after a restart the empty drawer doesn't name
  the closed terminals.
- Settings: the renderer row "Terminal renderer: WebGL (faster) · DOM (safer)" has no frame (needs a design check);
  "Reveal in Finder" opens the folder rather than selecting the file.
- Window: the traffic-light offset and the caption buttons' hover colours are guesses (no frame state).
- Cleanup (T-0235): the kept lines don't count commits or uncommitted files.

- Project agents (T-0243): the tab draws no answer box (a question is answered in the terminal or the inbox); Created
  and changed lists created and edited tickets, not dependency rows (the service sends only their count); Close
  mid-turn's dialog doesn't say what the agent is doing.
- After a restart (T-0244): a restart that ended only Project agents raises no banner or sheet (the service status
  has no agent count); the panel of a ticket that was validating keeps the usual Interrupted state (no frame of its own).

**Removing the GPUI app (T-0245)**
- Only the binary is renamed: the crate stays `crates/crewboard-desktop` (package `crewboard-desktop`), its binary is
  `crewboard-app`, so the CLI launcher, the service's notification opener and 0072's archive layout are unchanged (Q45).
- The npm part of `THIRD-PARTY-LICENSES.md` comes from `scripts/licenses/npm.mjs` (plain Node, no license tool): the
  page's production packages from `package-lock.json` with the license files of the installed packages, sorted; dev
  dependencies are left out (they don't ship). `scripts/licenses.sh --check` covers crates and npm packages.
- Removed with the GPUI app: `gpui`, `gpui_platform`, `alacritty_terminal`, the Zed git dependencies, the async-task
  patch, the GPUI crate's fonts copy, and `crewboard-view`'s `edit_buffer.rs` (the GPUI text input's buffer, 6 tests).
  The Metal toolchain is no longer needed. The Linux VM's package list in `scripts/README.md` now names Tauri's
  libraries (WebKitGTK 4.1, GTK 3…), not yet tried on the VM.

**TO COMPLETE:** anything T-0246 / T-0247 changed.

## 5. Open questions

Added to `tickets/questions.md` in M14 (all deferred and built as their recommended option unless noted):

| Q | Question | Built |
|---|---|---|
| Q37 | Which terminal renderer does Linux use by default? | Option 1: DOM on Linux, WebGL on macOS and Windows; revisit with T-0247's real-hardware run |
| Q38 | Where is the terminal-renderer setting shown? | Option 1: a global setting, one row under Settings → Appearance |
| Q39 | When does the app set `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux? | Option 1: always on Linux unless the environment sets it |
| Q40 | What does the app show when the service can't start? | Option 1: the "Background service stopped" banner with the reason |
| Q41 | When do the view models move into `crewboard-view`? | **Resolved 2026-10-12 (user): option 1**, all at once in T-0207; recorded in 0075 point 8 |
| Q42 | Does the delete confirmation's title name the ticket's title? | Option 1: key and title everywhere (open; not marked deferred in the file) |
| Q43 | Where does the panel's Park confirmation live? | Option 1: on the board's drop only; the panel has no Park item |
| Q44 | Does the after-restart sheet call its note "NOT IN RESUME ALL"? | Option 1: "NOT IN RESUME SELECTED" |
| Q45 | After T-0245, does the Tauri crate keep the name `crewboard-desktop`? | Option 1: only the binary is renamed |
| Q46 | Sentences in SDD.md, DESIGN.md and the runbook that T-0245 made stale | Option 1: listed for the user to update |
| Q47 | Does a restart that ended only Project agents raise the after-restart banner and summary? | Option 1: no banner when only Project agents ended (T-0244; recommendation is option 2) |

T-0243 added none; Q47 records T-0244's known gap. **TO COMPLETE:** questions added by T-0246 … T-0247.

Also noted, outside the questions file:
- A once-seen flake `crewboard-desktop api::terminal::tests::attaching_mid_stream_shows_every_line_once_in_order`
  ("line 107 missing") was not reproduced; the test now uses gates and reports the seam (86e1e26, 79f80ad), and the
  bug fixed in 1e10b4c is a likely cause.
- The flaky `crewboard-cli --test hook` ("sh: child setpgid … Operation not permitted") was fixed by running the test
  shell with `set +m` (86e1e26).
- The intro of `tickets/v0.1-tickets.md` still says the user reviews everything after M11.

## 6. Platforms checked

Test counts, 2026-10-13, on the tree with T-0246's and T-0206's changes (one run per machine covered both):

| Machine | Script | cargo test | Vitest | Result |
|---|---|---|---|---|
| macOS (the Mac) | `scripts/check.sh` | 1216 passed, 0 failed, 3 ignored | 282 | green |
| Windows 11 Arm VM | `scripts/check.ps1` (first run ever) | 1216 passed, 0 failed, 3 ignored | 282 | green |
| Ubuntu 26.04 Arm VM | `scripts/check.sh` | — | — | **not checked** |

**Ubuntu: not checked.** Its setup was done (T-0246: WebKitGTK / GTK packages with apt, Node v24.21.0 with nvm, steps
and undo in `scripts/README.md`), and its first run passed the frontend steps and failed clippy on a macOS-only constant
(fixed). The second run froze in `cargo test --workspace` at 03:37 VM time when the Mac's disk filled up (both VMs built
at once and their disk images grew); the VM then quit, and the user chose to skip Linux rather than restart it (0075's
VM skip rule). Not seen on Linux therefore: the app opening, the DMABUF repaint (Q39), WebGL in the VM, WebKitGTK's
clipboard read and the folder picker. `scripts/README.md` now says to check the Mac's free space first, run one VM at a
time and use `CARGO_BUILD_JOBS=4` on Ubuntu.

**Windows (T-0246):** the release app opens and draws (onboarding, theme, tools, Add project, board, Project terminal);
the WebView2 key switch compiles and works (Ctrl+F, Ctrl+P, F5, Ctrl+R, Ctrl+= / Ctrl+- / Ctrl+Shift+=, Ctrl+Shift+I and
F7 reach the page and do nothing; `CREWBOARD_KEY_LOG=1`); the folder picker and a folder dropped from Explorer both open
Add project with the folder; the caption buttons maximize and close; the tools step finds git and claude and offers
winget lines for gh and az. Fixed in T-0246: the first Ctrl+Shift+V in a terminal raised WebView2's "tauri.localhost
wants to see text and images copied to the clipboard" prompt and lost that paste (now `enable_clipboard_access()`,
which also lets WebKitGTK read it; checked on Windows: no prompt, the text arrives); Git for Windows' CRLF checkouts
failed Prettier on every file (now `.gitattributes`); three M12 tests that had never run on Windows (cargo's build
folder on the stand-in agent's PATH ran a typed message's `crewboard answer --stdin`; `pwd -P` prints MSYS paths; the
console doesn't echo what a sleeping program never reads). The winget Node LTS package is still v24 (v24.20.0).
Seen in passing, for T-0247: the Project terminal's welcome lines vanish when PowerShell draws its first prompt line;
the key log overlay showed overlapping lines once after maximizing.

**TO COMPLETE by T-0247:** the hand-check results in `tickets/handcheck-windows.md` / `handcheck-linux.md`, and the
real-Linux-hardware run (WebGL and DOM, DMABUF on and off) or why it wasn't done.
