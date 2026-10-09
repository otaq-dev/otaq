# v0.1 progress

| Ticket | Title | Milestone | Commit | Hand checks pending | Notes |
|---|---|---|---|---|---|
| T-0001 | Initialize the repository and Cargo workspace | M0 | deddef3 | none | gpui not added yet; no rust-toolchain.toml, workspace rust-version = 1.99 |
| T-0005 | Local check script: fmt, clippy, tests | M0 | 6bee9b9 | "unformatted file / clippy warning makes it fail" (agent ran both once; both failed as expected) | no .github/workflows |
| T-0002 | One name constant for product, command, plugin namespace and env prefix | M0 | f151604 | none | literal guard is tests/naming_literals.rs |
| T-0003 | Headless core baseline: data folder and daily logs | M0 | e71c2f0 | none | crewboard.db, crewboard.sock, daily logs keep 14 |
| T-0004 | SQLite schema and migrations (SDD §8) | M0 | def36e2 | none | TEXT UUIDv7 ids, BIGINT ms timestamps, board_column; see commit for §8 refinements |
| T-0006 | Protocol: versioned JSON messages, events and build ID | M1 | 83513ab | none | newline-delimited JSON; build ID = git hash[-dirty].build secs; ping/service.status/service.stop version-independent |
| T-0007 | CLI skeleton and global conventions | M1 | 267912f | none | clap; JSON errors on stdout; app = crewboard-app next to the CLI binary |
| T-0008 | `crewboard daemon start / stop / status` | M1 | 9406532 | none | flock on service.lock (PID inside); start runs hidden `daemon run` detached |
| T-0009 | Socket locked to the current OS user (macOS) | M1 | a9c7058 | none | folder 0700 + socket 0600 at every start; peer UID check (rustix getuid) |
| T-0010 | Event stream subscription | M1 | c67e1e8 | none | events.subscribe; laggard = full queue (256) or missed bus events (1024) → disconnected |
| T-0011 | Client library: connect, start the service, reconnect | M1 | 57c6f0e | none | backoff 100 ms → 2 s; launcher runs `crewboard daemon start` next to the program |
| T-0012 | Build-ID / protocol check and user-clicked service restart (core) | M1 | d55b058 | none | any other build ID = ServiceOlder; restart waits for the old lock file to vanish |
| T-0013 | Tools checklist and permission-mode check (core) | M1 | 6723bb8 | none | tools.check; defaultMode from managed then user settings; no mode set → unknown |
| T-0014 | GPUI app skeleton connected to the service | M1 | 6f9cf2f | launch/close by hand | gpui = Zed v1.23.2 (c0199504); async-task patch mirrored; unicode-properties 0.1.3 |
| T-0015 | Design foundations in GPUI: tokens, themes, fonts, icons | M1 | cabff05 | gallery vs 01 · Foundations (light, dark) | 39 tokens; Geist/Geist Mono OFL; 78 Lucide icons at 1.5 stroke; gallery via CREWBOARD_GALLERY=1 |
| T-0016 | Components: Button, Icon Button, Kbd, Banner, Toast | M1 | 7aad362 | gallery vs 01 · Components (light, dark) | focus ring as spread shadow; keymap.rs = d0058 table |
| T-0017 | Banner area, toasts and service banners | M1 | 2d28913 | banners/toasts vs 11 · Banners and 02 · App shell | max 2 rows; reconnecting folds in; "stopped" = no live PID in service.lock; Start service button (M1 interim) |
| T-0018 | Onboarding: welcome and tools checklist | M1 | af42f97 | onboarding vs 14 · First run, permission banner, copy/Re-check | onboarding.state / dismiss_permission_warning in settings table; Continue ends onboarding (Add project = T-0034) |
| T-0019 | Domain types: keys, kinds, columns, states, flags | M2 | 67d1174 | none | 20 statuses (Queued = waiting to start); orange names now come from `Status` (M1 agent counts updated) |
| T-0020 | Git runner and temp-repo test harness | M2 | 3be3d1a | none | async runner on tokio::process; harness in git::testing (tempfile now a core dependency) |
| T-0021 | Projects: add, validate, key, base branch, remove | M2 | b97052f | none | board::Board + Refusal codes; key suggestion SHOP → SW → SHOP2 |
| T-0022 | Tickets: create, edit, key allocation, fields, actor, events | M2 | 70c3f62 | none | numbers via UPDATE…RETURNING; base stored at creation; events created/updated with per-field changes |
| T-0023 | Agent-edit history (before/after) | M2 | d110b8c | none | changes in events payload; seen = agent_edits_seen event |
| T-0024 | Dependencies: DAG, cycle path, same project, same base, epic deps, tree | M2 | a1fd54f | none | loop path starts at the new parent; tree guards against loops |
| T-0025 | Epics: children, add to epic, move to epic + alias, close-epic plan, progress | M2 | 1780328 | none | Move to epic refuses tickets with an External ID; closed epic = Closed · not merged |
| T-0026 | Board state machine: transitions, drag rules, empty Done-when warning | M2 | b587108 | none | DropTarget splits Dev (Queued / Agent Working); later moves answer not_available_yet |
| T-0027 | Pinning and column sort order | M2 | 150d05c | none | ties: created_at then key; fold takes a card budget |
| T-0028 | Settings store: global and per-project values, validation | M2 | 0497591 | none | one settings row per field; font 13 (8–32); empty agent instructions by default |
| T-0029 | Protocol handlers and events: projects, tickets, deps, epics, pins, settings | M2 | f03a941 | none | events from a before/after diff of the project's tickets; errors refused/not_found with data.refusal |
| T-0030 | Components: Status Pill, Meta Chip, tabs, inputs, menus, dialogs | M2 | dd04a1f | gallery vs 01 · Components (Primitives, Badges & Inputs, Overlays), light/dark | TextInput on a tested EditBuffer; ↵ = Cancel on destructive dialogs; unicode-segmentation |
| T-0031 | Ticket Card component and flag priority | M2 | 1068bf3 | gallery grid vs 04 · Ticket card variants Light/Dark + Compact | quiet pill at 5 min (amber 3); rank 7 left to T-0154; CardExtras for later data |
| T-0032 | Epic Card component | M2 | 8aef09e | gallery epic rows vs 04 → Row · epics / delivery units, light/dark | progress segments per child; branch line from naming::branch until pushed |
| T-0033 | App shell: header and project switcher | M2 | f467ce1 | anatomy 1440/1280/1920 light+dark; rail states; ⌘P switcher | New ticket/Settings/Add project boxes open until T-0040/T-0045/T-0034 |
| T-0034 | Onboarding: add a project and the empty board | M2 | 2b74321 | 14 · 3 · Add project (3 states), 4 · empty board, second project dialog; 02 · No projects yet (light/dark) | refusal codes mapped to the design's copy; Remote/PR provider line waits for T-0141 |
| T-0035 | Board: columns, Dev sections, order, fold, Done rail, empty states | M2 | c41b232 | 03 · Board shop-web Light/Dark; empty column; 14-card column; 1280/1920 widths | Dev fold budget from window height; Done rail auto < 1440 px |
| T-0036 | Drag and drop, allowed/forbidden feedback, empty Done-when warning | M2 | d49ebf1 | drag allowed/forbidden, epic to Dev, empty Done-when dialog (SHOP-24), Esc cancels | Park/Approve/PR rework drops say not available yet; Add criteria box waits for T-0040 |
| T-0037 | Dependency badges and chain highlight | M2 | d788bb3 | chain highlight SHOP-20 light/dark (03 and 12 frames) | arrows only between visible members; no chips for hidden ones |
| T-0038 | Search and filter (⌘K) | M2 | f5d9b71 | 03 → Search & filter · ⌘K | Ticket.aliases added in core; is:needs-me includes review |
| T-0039 | Card right-click menu: Pin / Unpin | M2 | 4380c3f | 03 → Pinned tickets; 04 → Pinned | menu shows only working items; Delete… joins in T-0162 |
| T-0040 | Ticket editor: quick-create and full form | M2 | 652c747 | 08 → Quick-create, Full form · filled, Validation · empty form, Dark · full form | ⇧↵ is the full-form shortcut (frame) for the ticket's More…; read mode on double click |
| T-0041 | Editor: Depends-on picker, base branch, External ID | M2 | 1b2465b | 08 → picker, cycle prevention, base rows (SHOP-31, SHOP-19, cross-base), epic rows (SHOP-40, 40.2, epic dep) | links on existing tickets apply at once; remote branches stored without origin/ |
| T-0042 | Editor: agent-edit markers and before/after | M2 | ba0a1af | 08 → read mode with agent edits; Compare before/after | times shown as ages; Compare marks edits seen |
