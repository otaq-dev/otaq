# M2 · Board without agents — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0019 | Domain types: keys, kinds, columns, states, flags | 67d1174 |
| T-0020 | Git runner and temp-repo test harness | 3be3d1a |
| T-0021 | Projects: add, validate, key, base branch, remove | b97052f |
| T-0022 | Tickets: create, edit, key allocation, fields, actor, events | 70c3f62 |
| T-0023 | Agent-edit history (before/after) | d110b8c |
| T-0024 | Dependencies: DAG, cycle path, same project, same base, epic deps, tree | a1fd54f |
| T-0025 | Epics: children, add to epic, move to epic + alias, close-epic plan, progress | 1780328 |
| T-0026 | Board state machine: transitions, drag rules, empty Done-when warning | b587108 |
| T-0027 | Pinning and column sort order | 150d05c |
| T-0028 | Settings store: global and per-project values, validation | 0497591 |
| T-0029 | Protocol handlers and events: projects, tickets, deps, epics, pins, settings | f03a941 |
| T-0030 | Components: Status Pill, Meta Chip, tabs, inputs, menus, dialogs | dd04a1f |
| T-0031 | Ticket Card component and flag priority | 1068bf3 |
| T-0032 | Epic Card component | 8aef09e |
| T-0033 | App shell: header and project switcher | f467ce1 |
| T-0034 | Onboarding: add a project and the empty board | 2b74321 |
| T-0035 | Board: columns, Dev sections, order, fold, Done rail, empty states | c41b232 |
| T-0036 | Drag and drop, allowed/forbidden feedback, empty Done-when warning | d49ebf1 |
| T-0037 | Dependency badges and chain highlight | d788bb3 |
| T-0038 | Search and filter (⌘K) | f5d9b71 |
| T-0039 | Card right-click menu: Pin / Unpin | 4380c3f |
| T-0040 | Ticket editor: quick-create and full form | 652c747 |
| T-0041 | Editor: Depends-on picker, base branch, External ID | 1b2465b |
| T-0042 | Editor: agent-edit markers and before/after | ba0a1af |
| T-0043 | Epic actions: add ticket, move to epic, close epic, epic chip | a50aba6 |
| T-0044 | Stack list widget | ccee9e4 |
| T-0045 | Settings screens: global and project basics, remove project | 13ea175 |
| T-0161 | Delete never-started tickets (core) | b5d65ff |
| T-0162 | Delete from the card menu, with confirmation | 4fa643c |

## 2. How to try it

Use a throw-away data folder so nothing touches your real `~/.crewboard/` (keep its path short; `mktemp -d` does):

```sh
scripts/check.sh                                  # fmt, clippy -D warnings, tests
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)
target/debug/crewboard daemon start
```

**Empty start (onboarding).** Open `target/debug/crewboard-app` now, before seeding: with no projects the app shows
the add-project step. Point it at any git repository (or a temporary one: `mkdir /tmp/r && git -C /tmp/r init -b main
&& git -C /tmp/r commit --allow-empty -m init`).

**Sample data.** For the board checks, seed three temporary repositories and the shop-web tickets with this script
(save it as `seed.py`; it talks to the service over its socket, one JSON line per request):

```python
# usage: CREWBOARD_HOME=... python3 seed.py <repos-dir>  -- adds shop-web/billing-api/crewboard sample projects and tickets
import json, os, socket, subprocess, sys
home = os.environ['CREWBOARD_HOME']; repos = sys.argv[1]
def call(method, params=None, caller=None):
    s = socket.socket(socket.AF_UNIX); s.connect(os.path.join(home, 'crewboard.sock'))
    msg = {"protocol_version": 1, "id": 1, "request": {"method": method}}
    if params is not None: msg["request"]["params"] = params
    if caller: msg["caller"] = caller
    s.sendall((json.dumps(msg) + "\n").encode()); data = b""
    while not data.endswith(b"\n"): data += s.recv(65536)
    s.close(); return json.loads(data)
def git(*a, cwd): subprocess.run(["git", *a], cwd=cwd, check=True, capture_output=True,
    env={**os.environ, "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@x", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@x"})
for name in ["shop-web", "billing-api", "crewboard"]:
    p = os.path.join(repos, name); os.makedirs(p, exist_ok=True)
    if not os.path.exists(os.path.join(p, ".git")):
        git("init", "-q", "-b", "main", cwd=p); open(os.path.join(p, "README"), "w").write(name); git("add", ".", cwd=p); git("commit", "-qm", "init", cwd=p)
    print(call("project.add", {"path": p}).get("error") or name)
def t(title, **kw):
    r = call("ticket.create", {"project": "SHOP", "title": title, **kw}); return r["reply"]["ticket"]["key"] if "reply" in r else print(r)
for i in range(1, 17): t(f"filler {i}")
keys = {}
for title, dw in [("Cart totals", ["Totals add up"]), ("Checkout API", ["Returns 200 with a session id"]), ("Payment form", ["Card form validates"]), ("Checkout page", ["Pays"]), ("Order confirmation email", ["Email sent"]), ("Fix flaky login redirect", []), ("Split user full name", ["Names split"]), ("Update footer links", [])]:
    keys[title] = t(title, done_when=dw)
for child, parent in [("Checkout API", "Cart totals"), ("Checkout page", "Checkout API"), ("Checkout page", "Payment form"), ("Order confirmation email", "Checkout page")]:
    call("dep.add", {"child": keys[child], "parent": keys[parent]})
e = call("ticket.create", {"project": "SHOP", "kind": "epic", "title": "Wishlist", "external_id": "PAY-123"})["reply"]["ticket"]["key"]
for title in ["Wishlist API", "Wishlist button", "Wishlist page"]: t(title, epic=e, done_when=["works"])
call("ticket.move", {"key": keys["Cart totals"], "target": "dev_queued", "confirmed": True})
call("ticket.pin", {"key": keys["Split user full name"]})
call("ticket.update", {"key": keys["Payment form"], "body": "Use the new form", "done_when": [{"op": "add", "text": "Agent criterion"}]}, caller="agent:SHOP-19")
print("seeded", keys, e)
```

```sh
REPOS=$(mktemp -d); python3 seed.py $REPOS
target/debug/crewboard-app                        # the board
CREWBOARD_GALLERY=1 target/debug/crewboard-app    # also the gallery window (component sheets, card grid, epic rows, stack list)
```

What the seed creates in shop-web (key `SHOP`; the numbers differ from the design's, which uses SHOP-40 for the epic):
SHOP-1…16 fillers; SHOP-17 Cart totals (moved to Dev, so it sits in Queued); SHOP-18 Checkout API → depends on 17;
SHOP-19 Payment form (edited by `agent:SHOP-19`, so it carries agent-edit markers); SHOP-20 Checkout page → 18, 19;
SHOP-21 Order confirmation email → 20; SHOP-22 Fix flaky login redirect and SHOP-24 Update footer links (no Done-when
items, for the empty-Done-when warning); SHOP-23 Split user full name (pinned); epic SHOP-25 Wishlist (External ID
PAY-123) with SHOP-25.1–25.3. Projects billing-api (`BILL`) and crewboard (`CREW`) are empty.

States that need agents (Working, Needs you, review, conflicts, "N agents working", needs-you counts in the switcher)
can't be reached on the real board before M3; the gallery shows them from fixtures.

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark" means: compare once with macOS in light mode and once in dark mode (or
the gallery's Light / Dark buttons).

**T-0030 · Components** (gallery)
- [ ] `01 · Components → Primitives → Badges & Inputs → Badges & pills` against the gallery's "Badges & Inputs ·
  Badges & pills" (every status pill of SDD §7.2, dependency and meta chips), light/dark.
- [ ] `→ Tabs & segmented` and `→ Fields` (text field states: default, focused, error, disabled; textarea; select).
  Type into the live fields: caret, selection (⇧←/→, ⌘A), ⌘←/→, backspace/delete, multi-line.
- [ ] `01 · Components → Overlays & feedback`: tooltip, context menu and confirm dialog. In the dialog Esc and Cancel
  close it; a destructive dialog has Cancel as the default (↵ cancels).

**T-0031 · Ticket card** (gallery)
- [ ] `04 · Ticket card variants · Light → Variant grid` and `· Dark` against "04 · Ticket card variants · Variant
  grid (fixtures)": one card per flag, the flag priority (only the strongest flag shows as a pill; others as icons),
  quiet pill, conflicts pill plus callout, pinned, dependency chips, agent-edit dot.
- [ ] `04 → Compact density`: the gallery's "Compact density" row (smaller padding, one-line titles).

**T-0032 · Epic card** (gallery)
- [ ] `04 → Row · epics` and `Row · epics as delivery units` against "Row · epics (fixtures)": "Epic · 1 / 3 merged"
  pill, progress bar, External ID chip, closed epic "Closed · not merged".

**T-0033 · App shell** (board, seeded)
- [ ] `02 · App shell & project switcher → Window anatomy`, `Title bar macOS`, `1280×800`, `1920×1080 sidebar` (resize
  the window), `Dark 1920`; and `03 · Board → Header / Project rail`.
- [ ] `02 → Header states` and `One project` (a fresh home with one project), `Ten projects` (add seven more repos:
  `for i in 1 2 3 4 5 6 7; do mkdir $REPOS/p$i && git -C $REPOS/p$i init -qb main && git -C $REPOS/p$i commit -q
  --allow-empty -m i; done`, then add them with ⌘P → Add project or by dropping the folders on the window).
- [ ] `02 → Quick switcher ⌘P`: ⌘P opens it; typing filters; ↑/↓/↵ switch; ⌘1…⌘9 switch directly; ⌘N inside the
  switcher adds a project. `Another project needs you`: the layout only — the counts stay 0 until agents run (M3).

**T-0034 · Add project and empty board** (fresh `CREWBOARD_HOME`, before seeding)
- [ ] `14 · First run & onboarding → 3 · Add project valid`, `Not a git repo` (pick a plain folder), `Already added`
  (pick the same repo twice): name, key suggestion (shop-web → SHOP, a taken key → SW, then SHOP2), base branch picker.
  Known difference: the frame's "Remote / PR provider" line is left out (it comes with T-0141).
- [ ] `14 → 4 · Done empty board` and `Adding a second project later` (⌘P → Add project).
- [ ] `02 → No projects yet` and `Dark · no projects` (remove all projects in Settings, or a fresh home after
  onboarding was finished once).

**T-0035 · Board** (seeded, shop-web)
- [ ] `03 · Board → Board shop-web · Light` and `· Dark`: five columns, Dev sections (Queued with SHOP-17), card
  order (pinned first, then by created time), Done rail.
- [ ] `03 → Empty column` (the empty Dev and Review columns), `Very full column` (Backlog with 20+ cards: scrolls, the
  header stays), and `Width strategy`: below 1440 px the Done column folds to a rail; Dev folds sections when the window
  is too short for them.

**T-0036 · Drag and drop** (seeded)
- [ ] `03 → Drag over · allowed` and `Drag over · forbidden`: drag SHOP-18 over Dev (allowed, accent outline) and a
  Backlog card over Review (forbidden, red outline with the reason). Esc during a drag cancels it; the card goes back.
- [ ] `03 → Epic can't be dragged to Dev`: drag SHOP-25 over Dev.
- [ ] `08 · Ticket editor → Starting with empty Done when`: drag SHOP-22 (no Done-when items) to Dev; the warning
  dialog ("Start anyway" moves it to Queued, "Add criteria" opens it instead; nothing is blocked). Also ⌘⇧D on a
  selected card. Drops whose effects come later (Park, Approve, PR rework) say "not available yet".

**T-0037 · Dependency badges and chain highlight** (seeded)
- [ ] `03 → Chain highlight SHOP-20` and its Dark frame: hover or select SHOP-20 — SHOP-17, 18, 19 and 21 highlighted,
  the rest dimmed; the "blocked by" / "blocks" chips on the cards.
- [ ] `12 · Stack & dependencies → chain highlight`.

**T-0038 · Search and filter** (seeded)
- [ ] `03 → Search & filter ⌘K`: ⌘K focuses the field; free text matches key, title and epic title (and old keys
  after Move to epic); tokens `is:queued`, `is:backlog`, `is:needs-me` (includes review), `dep:SHOP-18`,
  `epic:Wishlist` become removable chips once followed by a space; the suggestion list shows counts; columns show
  "shown/total"; Esc clears the filter and returns focus to the board.

**T-0039 · Pin / Unpin** (seeded)
- [ ] `03 → Pinned tickets` and `04 → Pinned`: right-click a card → Pin; it moves to the top of its column with the
  pin icon; Unpin puts it back in order. SHOP-23 is pinned by the seed.

**T-0040 · Ticket editor** (seeded)
- [ ] `08 · Ticket editor → Quick-create`: ⌘N; type a title, ↵ creates; ⇧↵ opens the full form (the frame's "More…").
- [ ] `08 → Full form filled`, `Validation empty` (⌘↵ with an empty title), `Dark full form`. Double-click a card to
  open it read mode; Edit; ⌘↵ saves; Esc closes.

**T-0041 · Depends-on picker, base branch, External ID** (seeded)
- [ ] `08 → Depends-on picker open`: in SHOP-17's editor search "check"; `Cycle prevention`: SHOP-17 can't depend on
  SHOP-18 (greyed row with the loop path).
- [ ] `08 → base rows`: SHOP-17 (started) shows the base locked; a new ticket's base picker lists the repo's branches;
  a cross-base dependency is refused with the reason. `→ epic rows`: a child of SHOP-25 shows the epic's base, and a
  dependency on the epic itself.
- [ ] External ID field: saved and shown as a chip on the card.

**T-0042 · Agent-edit markers** (seeded)
- [ ] `08 → read mode with agent edits`: open SHOP-19 (double-click): the agent-edit marker on the body and the
  "Agent criterion" Done-when item, with "agent · 4m ago" style ages.
- [ ] `08 → Compare`: Compare shows before/after for each edit; after closing, the card's agent-edit dot is gone
  (seen).

**T-0043 · Epic actions** (seeded)
- [ ] `05 · Ticket side panel → Epic dialogs → Move to epic…`: right-click SHOP-24 → Move to epic… → pick SHOP-25:
  the new key SHOP-25.4, "· the old key SHOP-24 still works", Move to SHOP-25. Afterwards searching "SHOP-24" finds it.
- [ ] `08 → Move to epic · base changes`: give a ticket another base first (full form), then move it: the base note.
- [ ] `05 → Epic dialogs → Close epic…` on SHOP-25: "Also closes …" lists the children; confirm; the epic card shows
  "Closed · not merged". `04 → Row · epics` epic chip on the child cards.

**T-0044 · Stack list** (gallery and seeded)
- [ ] `12 · Stack & dependencies → Stack list · detail` against "12 · Stack & dependencies · Stack list (fixtures)" and
  SHOP-20's read mode (until the side panel arrives, T-0062).

**T-0045 · Settings** (seeded; ⌘, opens it)
- [ ] `15 · Settings → Global` (Agent instructions, Data with the database and logs paths, Appearance with theme and
  terminal font size, `→ Global · Tools`).
- [ ] `15 → Project fresh` (billing-api) and `Project configured + invalid`: in shop-web set Open URL to `localhost`
  and add env file `.env.local` (doesn't exist): both in red with the reason, the nav marks shop-web. Fields save on
  blur or ↵; restart the app and they are still there.
- [ ] `15 → Remove project · confirm` (crewboard): what goes, what is never touched; Cancel is the default.
  `→ Dark project`.

**T-0162 · Delete** (seeded)
- [ ] `05 · Ticket side panel → Delete never-started` (menu item) and `Delete ticket · confirm`: right-click SHOP-21 →
  Delete…: "Delete SHOP-21 Order confirmation email?" with "SHOP-20 is a dependency of SHOP-21; that link will be
  removed"; Cancel is the default. SHOP-17 (been to Dev) has no Delete item.
- [ ] `05 → Delete epic · confirm`: SHOP-25 (with its three children; none started). `→ Dark options & delete`.
- [ ] `03` card menu and `17 · Undo a created ticket`: create a ticket, then delete it from its menu; the number is
  not handed out again (the next ticket gets the next number).

## 4. Implementation choices (SDD §26)

- **Domain:** 20 ticket states; `queued` shows as plain "Queued". Enums stored as text through one `stored_enum!`
  macro. Project keys are 1–10 characters, uppercase letters and digits.
- **Git:** one runner on `tokio::process`; the temp-repo harness (`git::testing`) is public so the app crate's tests
  use it, which made `tempfile` a normal core dependency. Repo roots are canonicalised (so `/tmp` and `/private/tmp`
  are the same project). Remote branches are stored without `origin/`.
- **Keys:** suggestion SHOP → SW → SHOP2 when taken; numbers allocated with `UPDATE … RETURNING` (never reused, also
  after a delete).
- **Base and "started":** the base is stored when the ticket is created. "Started" = the first move to Dev
  (`started_at`); it locks the base, and a child locks the epic's base.
- **Events and agent edits:** ticket events `created` / `updated` with per-field changes and the actor. "Agent edits
  seen" is an `agent_edits_seen` event; edits are unseen when their UUIDv7 id is newer.
- **Dependencies:** the reported loop path starts at the new parent. Move to epic refuses tickets with an External
  ID (the epic carries it). A closed epic always shows "Closed · not merged".
- **Moves:** dropping on Dev = Queued. Park, Approve, PR rework and reopening a started ticket answer "not available
  yet" until their milestones. Esc cancels a drag.
- **Order and layout:** ties by created time, then key. Dev fold budget = (window height − 120) / 96, at least 4
  cards. Done becomes a rail below 1440 px.
- **Settings:** one row per field in the `settings` table; terminal font 13 (8–32); default agent instructions empty;
  Open URL must be http(s) with a host. Text fields save on blur or ↵. Sections for later features (limits, usage,
  notifications, PR provider, …) are left to their tickets.
- **Protocol:** board events computed from a before/after diff of the project's tickets; errors `refused` (with
  `data.refusal`) and `not_found`. New requests beyond the ticket: `project.branches`, `ticket.delete_preview`,
  `ticket.delete`. Delete is refused for agent callers (`not_allowed`) until T-0065 decides agent permissions. Core
  `Ticket` gained `aliases` and `next_child_number`.
- **UI:** GPUI has no text input, so `TextInput` is built on a tested `EditBuffer` (new crate `unicode-segmentation`).
  Destructive dialogs default to Cancel. The quiet pill appears after 5 min (amber at rank 3); rank 7 "asked" is left
  to T-0154. Conflicts show the pill and a callout. `is:needs-me` includes Ready for review. ⇧↵ is the quick-create's
  "More…". Agent-edit times are shown as ages ("4m ago"). Menus show only actions that work now (Close ticket… comes
  with T-0106). Move to epic lists epics as a pick list. The stack list sits in the editor's read mode until the side
  panel (T-0062).
- **Icons added:** archive, bot, message-circle-reply, alarm-clock, chevron-right, ban, eye-off, pin-off, lock (Lucide,
  lucide-static 1.54.0, stroke 1.5, as in M1).

## 5. Open questions and notes

No questions were added to `tickets/questions.md`. Things worth your attention:

1. **Visuals were not checked by the agent.** Screen capture isn't permitted in this environment, so the agent ran the
   app only as smoke runs (it starts and stays up with the seeded data). Every frame comparison above is still open.
2. **The add-project step leaves out "Remote / PR provider"** from its frame; it arrives with T-0141.
3. **Editing an existing ticket starts from read mode** (double-click a card), because the side panel is M3.
4. **Agent-dependent states** (switcher counts, Working / Needs you cards on the real board, "N agents running" in the
   Remove and Close-epic dialogs) are wired but show only in the gallery until M3.
