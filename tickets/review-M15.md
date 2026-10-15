# M15 review: Markdown description editor

M15 makes a ticket's "What and why" a formatted Markdown editor (decision 0078): a toolbar, links, tables, code blocks,
paste and copy that keep the formatting, a Markdown source toggle, a rendered read mode, Compare rendered by default and
a full-window view. What is stored is still the same plain Markdown string that agents read and write; an untouched
description is never re-saved. It was built by parallel teammates in worktrees with fast-forward landings (decision
0079), on 2026-10-15.

## 1. What was built

Oldest first, all on `main`:

| Commit | What |
|---|---|
| 5cd637d | M15 planning: decisions 0078 and 0079, tickets T-0248–T-0261, the orchestrator prompt, design-brief changes, DESIGN.md / SDD.md updates |
| 5e04cc4 | Decision 0078 refined after the round-trip gate (Q48): keep Tiptap, accept the listed differences, T-0254 fixes the code fence |
| 3fa0433 | **T-0248** Packages, licences and the round-trip gate: Tiptap 3.32.0 packages, lowlight 3.3.0, highlight.js 11.11.2 (the BSD exception), happy-dom 21.0.1 (dev); 90 round-trip samples, report in `tickets/m15-roundtrip.md` |
| 777bbbe | **T-0249** The editor core: `useMarkdownEditor` + `MarkdownSurface` + `MarkdownEditor`, one component for edit and read; the untouched rule (`markdown/changes.ts`); gallery section |
| 5c383b4 | **T-0257** Read mode: the rendered description, Copy as Markdown (the stored string, text/plain only), the label row with Expand's slot |
| 3fc1742 | **T-0254** Code blocks: language picker, colours from the Diff tab's tokens, Copy, sideways scroll; the fence fits the code (Q48) |
| 3caff5e | **T-0250** Toolbar: 12 buttons, Heading menu, active states, tooltips with shortcuts, the "…" overflow; ⌘⌥1–6 blocked (0058) |
| 481b723 | **T-0258** Compare: rendered by default, a Rendered / Markdown toggle per description entry |
| 36283a7 | **T-0253** Tables: the table bar (rows, columns, delete), one header row kept; Tab leaves tables |
| bf0b165 | **T-0256** The ⌘N form and the Ticket tab's edit mode: the field, hint, 480 px height with the pinned toolbar, fill height in the tab |
| f66b8dc | **T-0252** Links: the ⌘K popover, the hover bubble, read mode's tooltip, the system browser; ⌘K scoped to the editor |
| 68fd4bf | **T-0255** Copy and paste: cleaned HTML paste, Markdown-looking text parsed, ⌘⇧V plain paste, Markdown on the clipboard's text/plain, the "left out" note |
| 1a85a0d | **T-0259** Keys and focus: Tab in lists and code, Esc in layers (popover → editor → form), ⌘↵ from anywhere |
| f9b34ec | **T-0251** The Markdown source toggle, remembered per user (`GlobalSettings.description_source_mode`) |
| 65ab766 | **T-0260** The full-window view, edit and read (the `full_view` overlay, same editor instance and undo history) |
| (this branch) | **T-0261** This review and the Windows check |

## 2. How to try it

Node 24 comes from nvm on the Mac; nothing is installed globally. The app starts the service from the `otaq` binary
next to it, so build the workspace once first. Keep `OTAQ_HOME` short (macOS's 104-byte socket path limit).

```sh
export PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"
cargo build --workspace                       # the otaq CLI / service the app starts
cd crates/otaq-desktop
npm ci
rm -rf /tmp/m15 && OTAQ_HOME=/tmp/m15 npm run tauri dev
```

- **Sample project and SHOP-47.** In onboarding, add a small temporary repository as the project `shop-web` with the key
  `SHOP` (`git init /tmp/shop-web && git -C /tmp/shop-web commit --allow-empty -m init`). Save the example of
  `tickets/m15-tickets.md` (§ "The example") as `/tmp/m15-example.md` and create the ticket:
  `OTAQ_HOME=/tmp/m15 ../../target/debug/otaq create "Checkout address autocomplete" --project SHOP --stdin < /tmp/m15-example.md`.
  Its number won't be 47; any SHOP ticket with the example is "SHOP-47" in the checks below. Make a few more with
  `otaq create` so it isn't alone (and one for "a working ticket", moved to Dev with a stand-in agent as in
  `review-M14.md`).
- **Gallery** (the editor samples, sections "08 · Ticket editor · …"): `OTAQ_GALLERY=1 OTAQ_HOME=/tmp/m15 npm run tauri dev`.
  The palette switch in the gallery covers Otaq Light, Otaq Dark, Dracula and Solarized Light.
- **Open a ticket at start:** `OTAQ_OPEN=SHOP-47 OTAQ_HOME=/tmp/m15 npm run tauri dev`.
- **Key log:** `OTAQ_KEY_LOG=1` shows the last keys and where each went (useful for the keyboard checks).
- **Window size:** the frames are 1440×900; the 480 px panel is the side panel dragged to its narrowest.
- **Round-trip report:** `tickets/m15-roundtrip.md` (what Tiptap changes on a save; the accepted outputs are the
  `saved` texts in `ui/src/markdown/roundtrip/samples.json`).

## 3. Hand checks

None of these is ticked by agents. Light = Otaq Light, Dark = Otaq Dark. Every frame is in `crewboard.pen`; "08 → …"
means *08 · Ticket editor → …*, "05 → …" means *05 · Ticket side panel → …*.

### Gallery (`OTAQ_GALLERY=1`)
- [ ] **T-0249** the editor sample against *08 → Row · Markdown editor parts (update P) → MD Content · component* and
  *08 → Row · Markdown editor · edit mode (update P) → b · Filled · edit mode · Light / Dark*, without the toolbar:
  headings H1 17 / H2 15 / H3 13, lists, task boxes, quote, inline code, table, code block; edit and read mode; the empty
  field with its placeholder. In Otaq Light, Otaq Dark and Dracula.
- [ ] **T-0250** the toolbar section ("08 · Ticket editor · MD Toolbar · component") against *MD Toolbar · component*,
  *a · Empty field* and *b · Filled · edit mode* (light and dark): button order, separators, active states, tooltips with
  shortcuts (⌘B, ⌘I, ⌘⇧X, ⌘E, ⌘K, ⌘⇧8, ⌘⇧7, ⌘⇧9, ⌘⇧B, ⌘⌥C). At 480 px against *08 → Row · Markdown editor · height,
  narrow panel, agent (update P) → h · Narrow Ticket tab · toolbar overflow* with "…" open: Strikethrough, Quote, Table,
  Horizontal rule move into "…" in that order; Expand and the Markdown toggle stay at the right end.
- [ ] **T-0251** the source sample against *08 → Row · Markdown editor · source, links, tables, code (update P) →
  d · Markdown source on*, light and dark: Geist Mono text, the toolbar's buttons faint, the switch on.
- [ ] **T-0252** the link sample against *08 → … source, links, tables, code → e · Link popover and link bubble*, light
  and dark.
- [ ] **T-0253** the table sample against *08 → … source, links, tables, code → f · Caret in a table*, light and dark:
  the bar sits just above the table, left edges aligned, Delete table in the error colour.
- [ ] **T-0254** the code section ("08 · Ticket editor · g · Code block · language picker") against *g*: label, picker
  with filter and the current row checked, colours. Colours in Otaq Light, Otaq Dark and **Solarized Light** use the Diff
  tab's tokens (keyword purple, string magenta, number / type amber, function blue, comment muted); the frame draws other
  colours, see Q50.

### ⌘N form (full form)
- [ ] **T-0256** ⌘N, paste the example, at 1440×900 against *08 → Row · ⌘N form at 1440×900 (D5) → D5 · ⌘N form · long
  description · 1440×900*: hint "Markdown · paste keeps formatting", the field grows to 480 px then scrolls under the
  pinned toolbar (hairline shadow once text is under it), the form body scrolls between the title and Save / Cancel.
  Known difference: D5's upward shadow on the Save / Cancel footer isn't built (the footer is the shared Dialog's). Also
  the older frames that now show the editor: *Full form · filled*, *Validation · empty form*, *Dark · full form* and the
  epic editors. Light and dark.
- [ ] **T-0251** turn the Markdown toggle on in SHOP-47's form, quit the app, start it again, edit SHOP-47: it opens in
  source mode. Type in the source, switch back: the rich text shows what was typed. Open, switch, switch back, Save
  without typing: the stored text is unchanged (`otaq show SHOP-47`).
- [ ] **T-0253** in SHOP-47's form put the caret in the table: the bar shows (against *f*); arrow keys move between
  cells; add a row above the header: the new row becomes the header; delete the table.
- [ ] **T-0255** (see "Copy and paste" below) in the form as well as the Ticket tab.

### Ticket tab, edit mode
- [ ] **T-0256** edit SHOP-47 in the Ticket tab against *08 → … edit mode → b · Filled · edit mode* and
  *08 → … height, narrow panel, agent → i · At max height, scrolled*; then in the 480 px panel against *h*. Edit a
  working ticket (Agent Working) and save against *j · a · Editing while the agent works* and *j · b · After Save*; also
  *Editing while it validates (SHOP-29)* and its Dark version. Light and dark.
- [ ] **T-0252** select "the support report", press ⌘K: the popover opens with the URL field focused (no clipboard
  pre-fill, Q53); change the URL, Apply: the caret sits after the link. Hover the link: the bubble (shortened URL, Open,
  Edit, Remove); Open opens the system browser. ⌘K with the focus on the board opens Search. Against *e*, light and dark.
- [ ] **T-0254** in the 480 px panel, the long `places.autocomplete` line scrolls sideways with the right-edge fade,
  against the code line in *h*. Typing ```` ``` ```` on the block's last empty line leaves it.

### Ticket tab, read mode
- [ ] **T-0257** SHOP-47's Ticket tab against *05 → Row · Ticket tab · rich Markdown read mode (update P) → c · Read
  mode · Light*, *c · Copied state* and *c · Read mode · Dark*: Copy as Markdown and Expand at the right end of the label
  row; Copy is Lucide `clipboard-copy`, turns green with a check for 1.5 s, and its dark tooltip reads a green check
  then "Copied" (Q49, option 2); the full-window header's Copy (T-0260) looks the same. Paste
  into Terminal: the stored Markdown exactly. A working ticket (Agent Working) shows the same two icons after the
  marker and Compare, against *08 → Ticket tab · read mode, with agent edits*.
- [ ] **T-0252** read mode's link tooltip (full URL, dark) against *05 → c · Read mode · Dark*; a click opens the
  browser. In the Ticket tab's edit mode, select text, ⌘K, type `mailto:a@b.c`, ↵: the popover shows the inline error
  "Only https: and http: links." (Q52).
- [ ] **T-0254** hover a code block in read mode: Copy shows, against the hover Copy in *05 → c · Read mode · Light*.

### Compare
- [ ] **T-0258** (real `claude`) let an agent change SHOP-12's description; press Compare on the description's
  "Updated by agent" marker, against *08 → Row · Markdown editor · paste and compare (update P) → 7 · Updated by agent ·
  compare* and *Compare · before / after*: both sides rendered on the diff-del / diff-add backgrounds; the toggle to
  Markdown shows the stored source in Geist Mono 11.5 px. Light and dark. Known difference: the Rendered / Markdown
  toggle sits at the right end of each description entry's heading row, not in the view's header as drawn.

### Full-window view
- [ ] **T-0260** at 1440×900, light and dark, against *08 → Row · Full-window view · edit (D11) → Full-window edit ·
  Light / Dark*, *→ Row · Full-window view · read (D11) → Full-window read · Light / Dark* and *Note · Full-window view*:
  - expand SHOP-47 from the ⌘N form, from the Ticket tab's edit mode and from read mode (the label row's Expand);
  - the header keeps room for the traffic lights, is a drag region, shows "What and why" and the title (the typed title
    for a new ticket); the 760 px column;
  - type in the full view, Collapse, ⌘Z: it undoes what was typed in the full view; the caret is back in the text after
    each Expand / Collapse;
  - open the link popover in the full view, Esc: only the popover closes; Esc again: the view collapses;
  - Save collapses and saves; Cancel collapses and keeps the edits; ⌘↵ saves;
  - the link popover, table bar and code language picker sit in the right places inside the view;
  - the source toggle works inside the view and matches the field afterwards;
  - Expand's icon is Lucide `maximize`, Collapse `minimize` (Q54).

### Keyboard
- [ ] **T-0259** in SHOP-47, in the ⌘N form and in the Ticket tab (notes: *08 → … edit mode → Annotations*):
  - Tab on "Keyboard: ↑ ↓ …" nests it under "Suggest addresses…"; ⇧Tab lifts it back; Tab in a list never leaves it;
  - Tab in a paragraph goes to Depends on; ⇧Tab to Priority; Tab in a table leaves the editor;
  - Tab in the code block indents two spaces; ⇧Tab removes them;
  - open the link popover, press Esc three times: popover closes, then the text loses its caret (focus border on the
    field), then the form discards (the ⌘N form closes; the Ticket tab returns to read mode);
  - Esc then Tab from a list and from a code block leaves the editor; Esc then ↵ goes back into the text;
  - Esc with the "…" menu or the Heading menu open closes only the menu;
  - ⌘↵ saves from the text, from the source view and with a popover open;
  - no heading shortcut: ⌘⌥1 does nothing (0058);
  - ⌘⌥C in the text toggles a code block (Code block's tooltip and its "…" entry read ⌘⌥C; Ctrl+Alt+C on Windows
    and Linux); with the focus outside the editor (the board, a filter field, read mode, the link popover's URL)
    ⌘⌥C does nothing and no inspector opens (Q51).
  - Known: the Markdown toggle has no shortcut and is out of the Tab order (0078: no shortcut; toolbar buttons out of
    the Tab order), so a keyboard-only user can't switch modes.

### Copy and paste
- [ ] **T-0255** against *08 → Row · Markdown editor · paste and compare (update P) → 6 · Copy and paste*, light and
  dark:
  - copy the "Address autocomplete — notes" part of a Google Doc or a web page with an image and paste it into SHOP-47:
    headings, bold, lists and links kept; colours and fonts gone; the note "1 image left out: descriptions are Markdown
    only" under the content, gone after the next edit;
  - copy a Claude reply with a table and paste it: a table with a header row;
  - paste Markdown text from Terminal (`cat /tmp/m15-example.md | pbcopy`): it becomes formatted;
  - ⌘⇧V pastes plain text, one paragraph per line, no formatting, and no WebKit "Paste" bubble on macOS;
  - copy a selection into TextEdit (rich: formatted) and into Terminal (Markdown); a selection inside one list item
    copies its text without the list marker.

### Windows
Agent-run checks in the Windows VM are in §6: `check.ps1` is green, the release app opens, and the editor checks
were **skipped** (a forced Windows Update restart). So these stay for the user, on Windows with Ctrl in place of ⌘:
- [ ] **T-0255** Ctrl+V of formatted text (from Edge or Word) into SHOP-47's description works the first time: no lost
  paste, no "wants to see text and images copied to the clipboard" prompt; an image in it gives the note.
- [ ] **T-0255** Ctrl+Shift+V pastes plain text, first time, no prompt; a selection copied out arrives rich in WordPad
  and as Markdown in Notepad.
- [ ] **T-0257 / T-0260** Copy as Markdown in read mode and in the full view's header, pasted into Notepad: the stored
  text exactly, no prompt.
- [ ] **T-0260** the full view's header shows the caption buttons (– □ ×) at the right, from `title_bar_end`; it drags
  the window; its buttons keep their clicks.
- [ ] **T-0252 / T-0259 / T-0260** Ctrl+K in the editor and in the full view is Link (Search on the board); Esc steps
  popover → full view → editor → form; Ctrl+↵ collapses the full view, then saves.

## 4. Implementation choices (SDD §26)

- **Library and round trip (T-0248, Q48):** Tiptap 3.32.0 with its Markdown extension. A save without an edit was
  checked on 90 samples; agent-written text came back unchanged in 34 of 40 and with spelling-only changes in 6. The
  user kept Tiptap (Q48). Untouched descriptions are never re-saved, so these changes only happen when a person edits.
- **One editor, two layers (T-0249):** `useMarkdownEditor` owns the instance; `MarkdownSurface` draws it wherever the
  description shows. The full-window view moves the same view (T-0260), so text, caret and undo carry over.
- **The untouched rule (T-0249, T-0251, T-0256):** only a transaction that changed the document counts as an edit;
  typing then undoing reports the original string byte for byte; loading uses no undo step; switching to source and
  back never counts.
- **Registration files:** `markdown/extensions.ts` (one line per feature) and `markdown/toolbar/buttons.ts`; the icon
  list (`otaq-view` `icons.rs` + assets + generated `IconName.ts`) became a third shared file in practice (18 Lucide icons
  added from lucide-static 1.54.0).
- **Code (T-0254):** lowlight with only the picker's languages, colours mapped onto the Diff tab's tokens (Q50); no
  language guessing; the fence is one backtick longer than the longest backtick run that starts a line.
- **Tables (T-0253):** Tiptap's Tab / ⇧Tab in tables off (0078: Tab leaves except in lists and code); one header row
  kept after every action; Delete row / column disabled at the last one.
- **Links (T-0252):** clicks never follow a link inside the webview; web links open through `terminal.open_link`;
  only `https:` / `http:` accepted, an existing non-web link (`mailto:`) kept and shown but not opened (Q52); no clipboard pre-fill (Q53); ⌘K is Link inside the editor and Search elsewhere
  (`keysKeptBy` in `app/keys.ts`).
- **Paste (T-0255):** Tiptap's paste rules off (pasted `*…*` stays text; bare URLs link after a reload); HTML cleaned
  before the schema reads it; images and embeds left out and counted; ⌘⇧V reads `navigator.clipboard.readText()`.
- **Keys (T-0259):** `routeKey` in `markdown/keys.ts`, a plain tested function; Esc layers popover → full view → editor
  → form; Esc moves focus to the field (focus border, no caret) instead of blurring.
- **Source mode (T-0251):** a global setting, no Settings row; the latest choice in this run wins over an older view.
- **Full-window view (T-0260):** a modal overlay drawn by its owner through a portal, so the form's own editor, source
  mode and callbacks are used; the field keeps its height while expanded.
- **Files outside the planned lists**, named in each commit: `components/Dialog.tsx` (T-0256, T-0259), `app/keys.ts`
  (T-0252, T-0259), `otaq-view` `icons.rs` / `keyboard_map.rs` / `agent_edits.rs` (T-0250, T-0252, T-0253, T-0258,
  T-0260), `markdown/Editor.tsx` mount lines (T-0251, T-0253, T-0255, T-0260).

## 5. Open questions

From `tickets/questions.md`:
- **Q48** (resolved, user, option 2): keep Tiptap after the round-trip gate; accept the spelling changes and the HTML,
  image and code-space losses; the broken fence fixed in T-0254. Decision 0078 refined (5e04cc4).
- **Q49** (resolved, user, option 2): Copy as Markdown uses `clipboard-copy`, and the Copied tooltip a green check
  icon followed by "Copied", as the design draws (Tooltip takes an optional leading icon).
- **Q50** (deferred, option 1): code colours are the Diff tab's tokens (the ticket), not the frames' accent / green.
- **Q51** (resolved, user, option 2): Code block's shortcut is the design's ⌘⌥C / Ctrl+Alt+C; the dispatcher lets ⌥⌘C
  through only in the description editor's text.
- **Q52** (resolved 2026-10-15, web links only, option 4): `mailto:` is refused like other schemes; existing non-web
  links are kept and shown but don't open (`terminal.open_link` takes web links only).
- **Q53** (deferred, option 1): no clipboard pre-fill in the link popover (needs a clipboard plugin).
- **Q54** (deferred, option 1): Expand / Collapse use Lucide `maximize` / `minimize` (0078 and the frames), not the
  ticket's `-2` variants.

### Notes
- **"flushSync was called from inside a lifecycle method"** (seen by T-0260 in `npm run tauri dev` at load, React
  StrictMode, before any Expand). It is M15's, not older: before 3fa0433 the app had no Tiptap, and the only
  `flushSync` in the page's code path is Tiptap's own `ReactRenderer` constructor (`@tiptap/react`), which renders a
  React node view synchronously when the editor's content is already mounted. M15's one React node view is the code
  block (T-0254, `markdown/code/CodeBlock.ts`); `useMarkdownEditor` loads the description in a `useLayoutEffect`
  (T-0249, `markdown/Editor.tsx`) after `EditorContent` has mounted, so a description with a code block (SHOP-47, the
  gallery sample) creates that node view inside a React lifecycle and React warns. Effect: React postpones that one
  render to the end of the commit instead of running it at once, and the code block still draws (T-0254's tests and
  the gallery show it). The same warning prints in Vitest (stderr of `Editor.test.ts`, `CodeBlock.test.ts`,
  `source.test.ts`, `editor.test.ts`, all passing), always in tests whose text has a code block. A dev-only warning
  with no user-visible effect seen, so not fixed here; a later clean-up could load the first value through the
  editor's `content` option instead of the layout effect.
- The Windows copy shows `scripts/check.sh` and `scripts/licenses.sh` as modified after robocopy: the executable bit
  doesn't survive the shared folder (`git diff` is a mode change only). Harmless; the copy is never committed from.

## 6. Platforms checked

| Machine | Script | Rust tests | Vitest | Result |
|---|---|---|---|---|
| macOS (the Mac) | `scripts/check.sh`, by the orchestrator on every landing head (§7) | all passed | 809 (the same suite) | green on every landing |
| Windows 11 24H2 Arm VM | `scripts/check.ps1` on 65ab766 (robocopy of the main checkout) | all passed, 0 failed (1234 tests listed by `cargo test -- --list`) | 809, 50 files | **green**, 28.4 min |
| Ubuntu Arm VM | — | — | — | **not checked** |

**Windows (T-0261).** VMPal VM "Windows" (Windows 11 Pro 24H2, Arm64), resumed from its saved session. As
`scripts/README.md` says: `robocopy Z:\software-factory %USERPROFILE%\cb-src /MIR /XD target .claude node_modules dist`
(the main checkout at 65ab766, clean), `CARGO_TARGET_DIR=%USERPROFILE%\cb-target`, then
`powershell -ExecutionPolicy Bypass -File scripts\check.ps1` in the copy: every step passed (node, npm-ci, types, tsc,
eslint, prettier, vitest, page, fmt, clippy, test), 18:50–19:18 VM time. Nothing ran inside `Z:\`. The VM's disk had room
(the guest reported 890 GB free; VMPal 166 of 228 GB used), so the old `cb-target` was kept. T-0261 changes only
`tickets/`, so this run covers the final tree. The app checks in the VM are below.

**Windows app checks (agent-run, not the user's hand checks).** The release build was made as the README says
(`cargo build --release -p otaq-cli`, then `npm run tauri build` in `crates\otaq-desktop`, 2.2 min) and started on the
signed-in desktop through a scheduled task (`OTAQ_HOME=%USERPROFILE%\m15\h1`; the task was deleted afterwards). Then
Windows Update put up "We'll restart and install this update at 19:30" (VM time 19:19), ten minutes away. Postponing
the restart means changing the VM's update settings, which this session's permission system refused, and screenshots
of the VM were refused from then on too; after the restart Windows waits at the sign-in screen. So the app checks
stop here:

| Check | Result | Evidence |
|---|---|---|
| The release app opens (onboarding) | **Pass** | the screen text read "Welcome · 2 Theme · 3 Tools · 4 Project · 5 Done", "One inbox … Ctrl+J opens the next" |
| Ctrl+V of formatted text into a description works the first time (no lost paste, no clipboard prompt) | **Skipped** | forced Windows Update restart; VM control refused |
| Ctrl+Shift+V pastes plain text (`navigator.clipboard.readText()`, no prompt) | **Skipped** | same |
| Copy a selection out (rich into WordPad, Markdown into Notepad) | **Skipped** | same |
| Copy as Markdown (read mode and the full view's header) | **Skipped** | same |
| Full view's header: caption buttons from `title_bar_end`, a drag region, buttons keep clicks | **Skipped** | same |
| Ctrl+K in the editor (and the full view) is Link, not Search | **Skipped** | same |
| Esc layers (popover → full view → editor → form) | **Skipped** | same |
| Ctrl+↵ saves (collapses the full view first) | **Skipped** | same |

These rows are worth repeating by hand on Windows, together with the frame comparisons of §3 (Ctrl in place of ⌘):
WebView2's clipboard is where M14 found a lost first paste (T-0246, fixed with `enable_clipboard_access()`), and the
⌘⇧V path (T-0255) and the link popover (Q53) depend on the same clipboard access. To rebuild: the README's steps,
`%USERPROFILE%\m15\example.md` holds the example and `%USERPROFILE%\m15\shop-web` is a ready repository for the
project (both left in the VM).

**Linux: not checked.** There is no Ubuntu VM in VMPal any more (the orchestrator's VM list on 2026-10-15 had only
"Windows"). By the M14 rule, a VM that can't be reached is skipped and recorded, not a blocker. Not seen on Linux
therefore: the editor in WebKitGTK, its clipboard (paste, ⌘⇧V's `navigator.clipboard.readText()`), and the full-window
view's header there.

## 7. How it was run (decision 0079)

M15 was the first milestone built by parallel teammates (decision 0079): an orchestrator on `main` started each ticket
as a teammate in its own worktree and branch (`m15/T-xxxx`), at most three at once, and landed each finished branch on
`main` by **fast-forward only**, after `scripts/check.sh` was green on that exact head. A branch that fell behind was
rebased onto the new `main` and checked again. Times are 2026-10-15, Mac local time.

**Order and timeline**

| Time | What | Why / notes |
|---|---|---|
| 16:02–16:07 | planning 5cd637d landed (check.sh 5 min, cached build) | T-0248 had already started from it: it changes `package.json`, so it runs alone |
| ~16:10 | T-0248 stopped at the round-trip gate (a248eba): 35 of 90 samples differed | user chose option 2 (Q48) |
| 16:23 | 5e04cc4 (0078 refined, Q48) landed after check.sh | T-0248 waited for it |
| 16:23–16:35 | T-0248 squashed to 3fa0433; landed 16:35 (check 16:33–16:35) | see "Process change" below |
| 16:36–16:57 | T-0249 alone; 777bbbe landed (check 16:52–16:57, 578 Vitest) | it creates the registration files everything else uses |
| 16:57 | **Wave A:** T-0250 ∥ T-0254 ∥ T-0257 | different folders (toolbar/, code/, ReadMode.tsx + Rust); overlaps only one-line `extensions.ts` edits and their own gallery sections; T-0257 the only Rust ticket. T-0250 picked first because it unblocks T-0251, T-0252 and T-0256 |
| 17:07 | T-0257 ready; T-0253 started in its slot | table/ plus one `extensions.ts` line |
| 17:12 | T-0257 5c383b4 landed (check 17:07–17:11), Q49 | T-0254's question was also Q49 → rebased and renumbered |
| 17:13–17:18 | T-0254 rebased → 3fc1742 (question renumbered Q50), landed 17:18 | |
| 17:16 | T-0250 bfa51c4 ready, but its question collided with Q50 too | waited, then rebased and renumbered (Q51) |
| 17:18 | T-0258 started | chosen before T-0255 because T-0256 also edits `editor.rs`: finishing T-0258 first avoids that overlap |
| 17:22 | T-0253 75e47c2 ready, but it also added icons (icons.rs, IconName.ts) like T-0250 | waited for T-0250, then rebased |
| 17:24 | T-0250 3caff5e landed (check 17:19–17:24), Q51; T-0252 started | T-0252 unblocks T-0259 |
| 17:26–17:30 | T-0258 481b723 landed (check 17:26–17:30) | before T-0253, to free `editor.rs` for T-0256 / T-0251 |
| 17:30 | T-0256 started | T-0251 waits for it (both `editor.rs`) |
| 17:36 | T-0253 36283a7 landed (check 17:31–17:36) after two rebases (T-0250's icons, T-0258's progress row); T-0255 started | |
| 17:47 | T-0256 bf0b165 landed (check 17:42–17:46); T-0251 started | |
| 17:53 | T-0252 f66b8dc landed (check 17:48–17:53) after a rebase onto T-0256, Q52, Q53; T-0259 started | |
| 18:00 | T-0255 68fd4bf landed (check 17:55–18:00) after a rebase onto T-0252 | |
| 18:04–18:14 | T-0251 2f75c41 and T-0259 59e2043 both ready; **order changed: T-0259 first** | T-0259 unblocks T-0260; T-0251 unblocks nothing. T-0259 amended to 1a85a0d (a stand-in source-view test), landed 18:14 (check 18:09–18:14) |
| 18:22 | T-0251 f9b34ec landed (check 18:18–18:22) after three rebases | the last rebase also fixed ↵-after-Esc going to the hidden rich text |
| 18:22–18:48 | T-0260 alone (no rebase needed); 65ab766 landed (check 18:44–18:48), Q54 | it touches Fields.tsx, Toolbar.tsx, ReadMode.tsx, TicketTab.tsx and keys.ts |
| 18:48 | T-0261 started alone | review file, Windows VM |

T-0261 ran 18:48 to about 19:30 (the Windows `check.ps1` took 18:50–19:18) and lands by fast-forward after the
orchestrator's `check.sh` on its head, which closes M15: about three and a half hours from 16:00, with 13 code
tickets landed in 2 h 13 min (16:35–18:48).

**Rebases and re-checks:** T-0254 once (Q49 → Q50), T-0250 once (Q50 → Q51), T-0253 twice (icons, progress row), T-0252
once, T-0255 once, T-0251 three times. Every rebased head got its own green `check.sh` before landing; conflicts were
only in `tickets/progress.md`, `tickets/questions.md` and the icon list, resolved by keeping both sides.

**Process change after T-0248.** T-0248 started `check.sh` in its own background and went idle; a background command
dies when the teammate goes idle, so no result ever came. From then on teammates stopped after squashing and reporting,
and the **orchestrator ran `check.sh` on the exact head** before each fast-forward. Each check took 4–5 minutes with
the shared build cache.

**What collided, and the lessons:**
- **Question numbers:** parallel teammates each took "the next free Q number" from their own branch, so T-0254 and
  T-0257 both wrote Q49 and T-0250 then collided with Q50. Each later lander renumbered on rebase.
- **The icon list:** `otaq-view` `icons.rs`, its SVG assets and the generated `IconName.ts` turned out to be a shared
  registration file (T-0250, T-0253, T-0252 and T-0260 all added icons); it wasn't in the planned shared-file list.
  Later briefs named it.
- **Files outside the lists:** each commit names them (§4). The pen.dev screenshot tool is shared too: "A capture is
  already running" meant another teammate was using it (T-0258 couldn't screenshot).
- **Ordering by what a ticket unblocks** (T-0250 first in wave A, T-0252 over T-0255, T-0259 before T-0251) kept the
  long chain T-0256 → T-0259 → T-0260 moving.
