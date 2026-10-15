# M15 · Markdown description editor — tickets

The first milestone after v0.1. It turns the ticket description ("What and why") into a rich Markdown editor. The sources
are:
- `decisions/0078` (the editor, D1–D11) and `decisions/0079` (this milestone and how it is built);
- `SDD.md` TKT-10, TKT-11, TKT-12 and UI-05;
- the design: design update P in `crewboard.pen`, checked in `design-briefs/AUDIT-2026-10-15-p.md` (all of it applied on
  2026-10-15).

**14 tickets, T-0248–T-0261, one epic: M15 · Markdown description editor.**

- **Format:** the same as `tickets/v0.1-tickets.md` (the shape for `otaq create -f`, 0032). The IDs continue that file's.
- **Definition of done** (0056): `scripts/check.sh` is green, new behaviour has tests, the "Done when" items are met, and the
  commit names the ticket, its requirement IDs and decisions 0078 / 0079. **A conflict with a decision → defer or stop as
  `prompts/implementer.md` says.**
- **Hand checks** compare the running app with the named frames. They're never ticked by agents. Run the app with
  `OTAQ_HOME=/tmp/m15 npm run tauri dev` in `crates/otaq-desktop`, light (Otaq Light) and dark (Otaq Dark). Use a test
  project whose ticket SHOP-47 has the update P example description (§ "The example" below).
- **Where the code goes.** This list is what makes parallel work possible, so keep to it. If a ticket really needs another
  place, name it in its commit and in `tickets/review-M15.md`.
  - `crates/otaq-desktop/ui/src/markdown/`: the editor, shared by every screen.
    - `Editor.tsx`, `extensions.ts` (**the extension list: one line per feature**), `markdown.css` and `index.ts`: T-0249.
    - `toolbar/`: T-0250 and T-0251.
    - `link/`: T-0252.
    - `table/`: T-0253.
    - `code/`: T-0254.
    - `paste/`: T-0255.
    - `fullwindow/`: T-0260.
  - Screens: `ui/src/screens/editor/` (the form, the Ticket tab, read mode, Compare) and Rust `src/api/editor.rs`.
  - Rust view models stay in `otaq-view`. Settings live in `otaq-core` (`board/settings.rs`).
- **Shared files** are named per ticket. **The registration files** are `markdown/extensions.ts` and
  `markdown/toolbar/buttons.ts`; a ticket may add **one line** to each. Conflicts there are one-line merges.
- **No new npm packages after T-0248.** If one is really needed, it is a blocker (it changes `package.json`, which runs
  alone, and its licence must be MIT, 0078).

## The example
SHOP-47 · Checkout address autocomplete, Backlog (the update P sheets):

````markdown
## Why
Users mistype addresses at checkout; **12%** of failed deliveries trace back to it (see [the support report](https://example.com/report)).

## What
- Suggest addresses as the user types in *Street*
- Keyboard: ↑ ↓ to move, Enter to pick, Esc to close

## Done when
- [x] Suggestions appear after 3 characters
- [ ] Arrow keys move the highlight
- [ ] Works with the existing `AddressForm` validation

| Field | Source |
|---|---|
| Street | Places API |
| Postcode | Places API, editable |

```ts
const results = await places.autocomplete(query, { country: "TR" });
```
````

| Epic | Tickets |
|---|---|
| M15 · Markdown description editor | T-0248 – T-0261 (14) |

## M15 · Markdown description editor
Goal: the description is written and read as formatted Markdown. Paste and copy keep the formatting, and agents still read
and write the same plain Markdown.

### T-0248 · Packages, licences and the round-trip gate
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078, 0079, 0071, 0075
- **Design:** none (no UI)
- **Depends on:** none
- **Size:** M
- **Shared files:** `package.json`, `package-lock.json`, `THIRD-PARTY-LICENSES.md` (**runs alone**)

**What and why**
Adds every npm package M15 needs, in one go, so no later ticket touches `package.json`:
- `@tiptap/core`, `@tiptap/react`, `@tiptap/pm`, `@tiptap/starter-kit`, `@tiptap/markdown`, `@tiptap/extension-list`,
  `@tiptap/extension-table`, `@tiptap/extensions` and `@tiptap/extension-code-block-lowlight`, all MIT;
- `lowlight` (MIT) and `highlight.js` (BSD-3-Clause, the one exception in 0078);
- if the tests need a DOM, `happy-dom` (MIT) as a dev dependency.

Pin exact versions, as the other packages are. Check each licence with `npm view <pkg> license`. Anything that isn't MIT,
other than highlight.js, is a blocker. Regenerate `THIRD-PARTY-LICENSES.md` with `scripts/licenses.sh`; the About dialog's
list reads it.

Then the **round-trip gate** of 0078. A Vitest test builds the editor's Markdown setup **headless**: the same extensions
T-0249 will use, defined here in a first `markdown/extensions.ts` that T-0249 takes over. It loads each sample, serialises
it without edits, and compares the result with the input.

The samples are:
- the update P example;
- the "What and why" sections of 40 tickets taken from `tickets/v0.1-tickets.md`, which were written by an agent;
- edge cases: `*` and `+` bullets; `1)` lists; nested lists, two levels; task lists; escaped characters (`\*`, `\_`);
  inline HTML; hard line breaks; tables with alignment colons; `~~~` fences and fences with a language; setext headings;
  autolinks; trailing spaces; a file without a final newline; CRLF line endings.

**Every difference is written to `tickets/m15-roundtrip.md`:** the sample, the input and the output, and whether the meaning
changed (lost text, different nesting, a broken table) or only the spelling (`*` → `-`).
- **No differences:** the gate passes.
- **Any difference:** **stop as a blocker**. 0078 says to switch to Milkdown if Tiptap rewrites Markdown, so the user decides
  between keeping Tiptap with the listed differences and switching to Milkdown. Recommend keeping Tiptap if only the
  spelling changes, because T-0249 never re-saves an untouched description.

**Done when**
- [ ] the packages are in `package.json` at exact versions; the lock file is updated with `npm install`, and `npm ci` passes
- [ ] every licence is MIT except highlight.js (BSD-3-Clause), and the commit body lists each package and why
- [ ] `THIRD-PARTY-LICENSES.md` is regenerated, and `scripts/licenses.sh --check` passes
- [ ] the round-trip test runs in `npm run test` and `tickets/m15-roundtrip.md` exists
- [ ] no difference, or the user has decided (the answer recorded in `tickets/questions.md` and, if it changes 0078, in the
  decision)
- [ ] checks green (`scripts/check.sh`); the commit names TKT-10 and decisions 0078, 0079

### T-0249 · The editor core: one component for edit and read
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078, 0075
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor parts (update P) → MD Content · component;
  → Row · Markdown editor · edit mode (update P) → b · Filled · edit mode · Light / Dark
- **Depends on:** T-0248
- **Size:** M
- **Shared files:** creates `markdown/extensions.ts` and `markdown/toolbar/buttons.ts` (empty list) as the registration
  files; adds the editor sample to `screens/components/Gallery.tsx`

**What and why**
One React component, `<MarkdownEditor>` in `ui/src/markdown/`, used for every description in the app.
- **Props:** `value` (Markdown), `onChange(markdown)`, `editable`, `placeholder`, `label` (accessibility) and `onKeyDown`
  pass-through for the form.
- **Extensions:**
  - starter-kit (headings limited to H1–H3 in the UI; deeper pasted levels render like H3);
  - task lists;
  - tables (simple: no merged cells, no alignment);
  - code blocks through lowlight;
  - links (`openOnClick` off; T-0252 opens them);
  - placeholder;
  - markdown (GitHub-flavoured).
- **Untouched text is saved as it is.** If the user didn't change the document, `onChange` is never called, and the
  original string is what gets saved, byte for byte. A save never rewrites a description nobody edited.
- **Styles** come from MD Content · component: H2 15/600, body 13/1.45, inline code and code blocks in Geist Mono, links in
  the accent colour, task boxes as drawn, table header in the app's table style. Everything is CSS variables from the
  theme tokens, so all 15 themes work (0069, 0070).
- **Read mode** (`editable={false}`): task boxes are drawn the same way but don't react (no hover, default cursor), and
  nothing can be typed.

**Done when**
- [ ] the component renders the example in both modes, and `onChange` returns Markdown
- [ ] an untouched description saves its original string (a Vitest test)
- [ ] read-only task boxes don't toggle (a Vitest test)
- [ ] it matches *MD Content · component* and *b · Filled · edit mode* (without the toolbar), light and dark, and one dark
  palette (Dracula)
  - *Hand check:* start the app with `OTAQ_GALLERY=1` (the design gallery, `screens/components/Gallery.tsx`); add the
    editor there with the example in both modes and compare with the frames.
- [ ] checks green; the commit names TKT-10 and decisions 0078, 0079

### T-0250 · Toolbar: buttons, active states, tooltips and the "…" overflow
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078 (D8), 0058
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor parts (update P) → MD Toolbar · component;
  → Row · Markdown editor · edit mode (update P) → a · Empty field, b · Filled · edit mode (Light / Dark), Annotations;
  → Row · Markdown editor · height, narrow panel, agent (update P) → h · Narrow Ticket tab · toolbar overflow
- **Depends on:** T-0249
- **Size:** M
- **Shared files:** `markdown/toolbar/buttons.ts` (creates the full button list)

**What and why**
The toolbar from MD Toolbar · component, in `markdown/toolbar/`.
- **Buttons:** Heading menu (Text / H1–H3), Bold, Italic, Strikethrough, Inline code, Link, Bullet list, Numbered list, Task
  list, Quote, Code block, Table, Horizontal rule. **Expand** and the **Markdown** toggle sit at the right end; this ticket
  draws them as placeholder slots, which T-0260 and T-0251 fill.
- Lucide icons at 26 px. A button is **active** when the caret is in that format; for example, the caret in a task item
  lights up Task list.
- **Tooltips** show the name and the shortcut. Windows and Linux show Ctrl and Alt.
- **Shortcuts**, while the editor has focus: ⌘B, ⌘I, ⌘⇧X, ⌘E, ⌘K, ⌘⇧8, ⌘⇧7, ⌘⇧9, ⌘⇧B. **No heading shortcut** (⌘⌥1–6
  switches panel tabs, 0058).
- The Link button calls a hook that T-0252 fills; until then it adds or removes a link without a popover. Table inserts a
  2 × 2 table.
- **Overflow:** when the toolbar doesn't fit (a ResizeObserver), Strikethrough, Quote, Table and Horizontal rule move into
  "…" in that order. The menu lists them with their shortcuts. Expand and the Markdown toggle never move.
- The buttons aren't in the tab order (Annotations).

**Done when**
- [ ] every button applies its format and shows the active state; a Vitest test covers the overflow order
- [ ] matches *MD Toolbar · component*, *a* and *b*, light and dark; at 480 px matches *h* (the overflow menu open)
  - *Hand check:* the gallery's editor at full width and at 480 px.
- [ ] checks green; the commit names TKT-10 and decisions 0078, 0058

### T-0251 · The Markdown source toggle, remembered per user
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078 (D9), 0023
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · source, links, tables, code (update P) → d · Markdown source on
- **Depends on:** T-0250
- **Size:** S
- **Shared files:** `otaq-core/src/board/settings.rs` (one field), `src/api/editor.rs` (the open view and one request),
  the toolbar's toggle slot

**What and why**
- The "Markdown" switch at the toolbar's right end shows the raw source in Geist Mono, at the same size, in a plain text
  area, and turns it back into rendered text. The formatting buttons are disabled while the source shows.
- Edits made in either mode carry over: switching serialises or parses the Markdown.
- **Remembered per user (D9):**
  - a new `GlobalSettings` field (`description_source_mode`, default off, serde default for old files);
  - `editor.open` / the read view report it;
  - a new request `editor.source_mode` saves it.
  - Every editor opens in the last chosen mode.
- It's not a visible Settings row.

**Done when**
- [ ] the toggle switches both ways without losing edits (a Vitest test on the switching logic)
- [ ] the choice survives an app restart (a Rust test on the setting's default and its round trip)
- [ ] matches *d · Markdown source on*
  - *Hand check:* turn it on in SHOP-47, close the app, reopen it, and edit SHOP-47 again: source mode.
- [ ] checks green; the commit names TKT-10 and decisions 0078, 0023

### T-0252 · Links: the ⌘K popover, the hover bubble, the system browser
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078 (D2), 0058
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · source, links, tables, code (update P) → e · Link popover and link bubble;
  05 · Ticket side panel → Row · Ticket tab · rich Markdown read mode (update P) → c · Read mode · Dark (the link tooltip)
- **Depends on:** T-0250
- **Size:** M
- **Shared files:** `markdown/extensions.ts` (one line), the Link button's hook in `toolbar/`

**What and why**
In `markdown/link/`.
- **The popover.** ⌘K / Ctrl+K, or the Link button on a selection, opens it. It has a URL field (focused), Apply (↵) and
  Remove. If the clipboard holds a URL, the field is pre-filled. If reading the clipboard needs a new Tauri permission or
  plugin, leave the pre-fill out and record it as a deferred question; don't add a plugin.
- **The bubble.** Hovering a link, or the caret in one, shows the URL (truncated), Open, Edit and Remove.
- **Opening links:** in **read mode a click opens the link**; in edit mode only the bubble's Open does. Links always open in
  the system browser, through the existing `terminal.open_link` path or a new `editor.open_link` request with the same
  safety rules. Hovering in read mode shows the URL as a tooltip.
- **⌘K scope (D2):** only while the editor has focus. Everywhere else ⌘K stays Search / filter. The app's keyboard map
  (`registry/shortcuts.ts` and the keyboard-map view) gets a note row: "⌘K in the description: Link".

**Done when**
- [ ] ⌘K in the editor opens the popover; ⌘K on the board still opens Search
- [ ] Apply, Remove and the bubble's actions work, and a link never opens inside Otaq
- [ ] a Vitest test for the URL check (`https:`, `http:` and `mailto:` only; other schemes are refused with the popover's
  inline error)
- [ ] matches *e*, light and dark
  - *Hand check:* select "the support report" in SHOP-47, press ⌘K, change the URL, Apply; hover it; Open opens the
    browser. ⌘K with the caret on the board opens Search.
- [ ] checks green; the commit names TKT-10 and decisions 0078, 0058

### T-0253 · Tables: the table bar
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · source, links, tables, code (update P) → f · Caret in a table
- **Depends on:** T-0249
- **Size:** S
- **Shared files:** `markdown/extensions.ts` (one line)

**What and why**
In `markdown/table/`. When the caret is in a table, a small bar floats above it with: add row above / below, add column
left / right, delete row, delete column and delete table (in red). Each icon has a tooltip. Arrow keys move between cells.
Simple tables only, which is what Markdown can store.

**Done when**
- [ ] every action works and the result saves as a valid Markdown table (a Vitest test per action on the serialised text)
- [ ] matches *f*, light and dark
- [ ] checks green; the commit names TKT-10 and decision 0078

### T-0254 · Code blocks: language picker, coloured code, Copy, sideways scroll
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078 (D1)
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · source, links, tables, code (update P) → g · Code block · language picker;
  → Row · Markdown editor · height, narrow panel, agent (update P) → h · Narrow Ticket tab · toolbar overflow (the scrolled
  code line); 05 · Ticket side panel → Row · Ticket tab · rich Markdown read mode (update P) → c · Read mode · Light (Copy on hover)
- **Depends on:** T-0249
- **Size:** M
- **Shared files:** `markdown/extensions.ts` (one line)

**What and why**
In `markdown/code/`.
- **Colours:** lowlight's highlight.js classes map to the **same six token colours as the Diff tab**: keyword, string,
  number / type, comment, function (`screens/diff/diff.css`, `CodeToken`). Code looks the same in both places and in every
  theme. Load only the languages in the picker.
- **The language label** at the top right opens a short list with a filter: Plain text, bash, diff, json, rust, ts, tsx,
  yaml, toml. The choice is stored after the opening fence (```` ```ts ````).
- **Leaving a block:** ```` ``` ```` on an empty line, or ↓ at the end, leaves it. (Tab and ⇧Tab inside blocks are
  T-0259's.)
- **Read mode:** "Copy" appears on hover and copies just that block's code.
- **Long lines** scroll sideways inside the block, with no wrapping, a right-edge fade and a thin scrollbar on hover.
- **The fence fits the code (Q48):** when a block's code contains a line of backticks (a Markdown example with its own
  ```` ``` ````), the saved fence is longer than the longest run of backticks inside (```` ```` ````), so the block doesn't
  close early. Override the code block's Markdown output in `markdown/code/`; no new package. Update the round-trip
  test's expectations for the `fence-containing-fence` and `four-backtick-fence` samples (T-0248) to the fixed output.

**Done when**
- [ ] the picker changes the fence's language, and the text round-trips (a Vitest test)
- [ ] a block containing ```` ``` ```` saves with a longer fence and survives two saves unchanged (the round-trip samples)
- [ ] the colours use the diff's token variables (shown in Otaq Light, Otaq Dark and Solarized Light)
- [ ] matches *g*, the code line in *h*, and the hover Copy in *c*
- [ ] checks green; the commit names TKT-10 and decision 0078

### T-0255 · Copy and paste
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-11
- **Decisions:** 0078 (D6)
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · paste and compare (update P) → 6 · Copy and paste
- **Depends on:** T-0249
- **Size:** M
- **Shared files:** `markdown/extensions.ts` (one line)

**What and why**
In `markdown/paste/`.
- **Pasting HTML** (from a web page or Google Docs) keeps headings, lists, bold, italic, links, inline code, code blocks and
  tables. It drops colours, fonts, highlights, underline and alignment, and merged table cells are split.
- **Pasting plain text** that looks like Markdown (headings, `- [ ]`, fences, `**bold**`, links, tables) becomes
  formatting.
- **⌘⇧V / Ctrl+Shift+V** pastes plain text, with no formatting and no Markdown parsing.
- **Images and embeds** are left out. A muted line under the content says how many ("1 image left out: descriptions are
  Markdown only"), until the next edit (D6).
- **Copying a selection** puts both HTML and Markdown (`text/plain`) on the clipboard.
- **WebView2:** on Windows, paste must work the first time (T-0246 enabled clipboard access).

**Done when**
- [ ] Vitest tests: a Google-Docs-shaped HTML sample becomes the expected Markdown; a Markdown text sample becomes
  formatting; an image is counted and dropped; the copy serialiser gives Markdown
- [ ] matches *6 · Copy and paste*
  - *Hand check:* copy the "Address autocomplete — notes" part of a Google Doc or web page with an image into SHOP-47:
    compare with the frame. Copy a Claude reply with a table and paste it. Paste with ⌘⇧V. Copy a selection into TextEdit
    (rich) and into Terminal (Markdown).
- [ ] checks green; the commit names TKT-11 and decision 0078

### T-0256 · The form and the Ticket tab's edit mode: the field, hint, height and pinned footer
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10, UI-05
- **Decisions:** 0078 (D5), 0019, 0048, 0063
- **Design:** crewboard.pen → 08 · Ticket editor:
  - → Row · Markdown editor · edit mode (update P) → a · Empty field, b · Filled · edit mode (Light / Dark);
  - → Row · ⌘N form at 1440×900 (D5) → D5 · ⌘N form · long description · 1440×900;
  - → Row · Markdown editor · height, narrow panel, agent (update P) → i · At max height, scrolled; h · Narrow Ticket tab ·
    toolbar overflow; j · a · Editing while the agent works; j · b · After Save;
  - the older frames that now show the editor: Full form · filled, Validation · empty form, Editing while the agent works,
    Editing while it validates (SHOP-29) and its Dark version, Dark · full form, and the epic editors.
- **Depends on:** T-0250
- **Size:** M
- **Shared files:** `screens/editor/Fields.tsx`, `Modal.tsx`, `TicketTab.tsx`, `editor.css`; `src/api/editor.rs`
  (`body_hint`)

**What and why**
- Replace the description's `TextArea` in `Fields.tsx` with `<MarkdownEditor>` plus the toolbar, in the full form (⌘N, edit
  ticket) and in the Ticket tab's edit mode. The hint becomes "Markdown · paste keeps formatting" (`editor.rs`). The
  placeholder is unchanged.
- **Height (D5):**
  - In the full form the field grows with its content up to 480 px, then scrolls inside. The toolbar stays pinned, with a
    hairline shadow once content scrolls under it.
  - When the form is taller than the window, the form body scrolls and Save / Cancel stays pinned.
  - In the Ticket tab the field may grow until it fills the tab.
- **Keep what works today:** ⌘↵ saves from inside the editor; the save notes for a working (0048) or validating (0063)
  ticket; the "unsaved warning".
- Quick-create (title only) doesn't change.

**Done when**
- [ ] create, edit and save work with the editor, and the stored description is Markdown (a Vitest test on the form state;
  the Rust editor tests kept)
- [ ] matches the listed frames, light and dark, at 1440×900 and in the 480 px panel
  - *Hand check:* ⌘N with the example pasted against *D5*; edit SHOP-47 in the Ticket tab against *b* and *i*; edit a
    working ticket against *j · a* and *j · b*.
- [ ] checks green; the commit names TKT-10, UI-05 and decisions 0078, 0019, 0048, 0063

### T-0257 · Read mode: the rendered description, Copy as Markdown, the label row
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10, TKT-11, UI-05
- **Decisions:** 0078 (D7)
- **Design:** crewboard.pen → 05 · Ticket side panel → Row · Ticket tab · rich Markdown read mode (update P) → c · Read mode
  · Light, c · Copied state, c · Read mode · Dark, Annotations; 08 · Ticket editor → Ticket tab · read mode, with agent edits
- **Depends on:** T-0249
- **Size:** M
- **Shared files:** `screens/editor/ReadMode.tsx`, `editor.css`; `src/api/editor.rs` (the read view);
  `otaq-view/src/view_models/markdown.rs` (removed)

**What and why**
- Read mode shows the description with `<MarkdownEditor editable={false}>`. `EditorReadView.body` becomes the raw Markdown
  string (`body_markdown`) instead of `MarkdownBlock[]`. Remove `markdown.rs` and `MarkdownBlock` if nothing else uses them
  (0078's consequence), and regenerate the ts-rs types.
- **The label row (D7):** "WHAT AND WHY" gets two icon buttons at its right end, **Copy as Markdown** and **Expand**, in
  every ticket state. Expand is a placeholder that T-0260 fills. They are not in the action bar.
- **Copy as Markdown** copies the stored Markdown byte for byte (no HTML). The icon turns into a check, and a "Copied ✓"
  tooltip shows for 1.5 s.
- The "Updated by agent" marker and Compare link stay on the same row, before the icons.
- Empty descriptions keep the existing "nothing written" text.

**Done when**
- [ ] the read view sends Markdown (a Rust test), and the old block model is gone (or the commit says why it stays)
- [ ] Copy as Markdown puts exactly the stored text on the clipboard (a Vitest test on the copy function)
- [ ] matches *c* (light, Copied, dark), including in a state without Edit (Agent Working)
  - *Hand check:* SHOP-47's Ticket tab; Copy as Markdown, then paste into a terminal; a working ticket shows the same two
    icons.
- [ ] checks green; the commit names TKT-10, TKT-11, UI-05 and decision 0078

### T-0258 · Compare: rendered by default, with a Rendered / Markdown toggle
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10, TKT-05
- **Decisions:** 0078, 0019
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · paste and compare (update P) → 7 · Updated by agent · compare;
  → Compare · before / after
- **Depends on:** T-0257
- **Size:** S
- **Shared files:** `screens/editor/ReadMode.tsx` (`Compare`), `src/api/editor.rs` (`EditorCompareEntry` gains its field)

**What and why**
- The before / after view shows the description **rendered** on both sides, using the read-only editor, on the
  `diff-del-bg` and `diff-add-bg` backgrounds.
- A **Rendered / Markdown** segmented toggle in the header shows the source instead. Rendered is the default.
- Only the description entry renders. Other fields (title, priority…) stay plain text. `EditorCompareEntry` gains `field`
  so the page knows which entry is the description.

**Done when**
- [ ] both modes show the agent's change; a Rust test covers the new `field`
- [ ] matches *7* (both states)
  - *Hand check (real `claude`):* let an agent change SHOP-12's description; Compare from the marker, then toggle.
- [ ] checks green; the commit names TKT-10, TKT-05 and decisions 0078, 0019

### T-0259 · Keys and focus: Tab in lists and code, Esc in layers, ⌘↵
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10
- **Decisions:** 0078 (D3, D4), 0058
- **Design:** crewboard.pen → 08 · Ticket editor → Row · Markdown editor · edit mode (update P) → Annotations (keyboard and
  focus notes); → Row · Open questions (update P) → Note · Open questions (now "Decided (0078)")
- **Depends on:** T-0252, T-0254, T-0256
- **Size:** M
- **Shared files:** `screens/editor/Modal.tsx` (`useFormKeys`), `TicketTab.tsx`, `markdown/Editor.tsx` (the key handling)

**What and why**
- **Tab (D3):**
  - In a list item, Tab / ⇧Tab indents and outdents.
  - In a code block, they indent and outdent the line.
  - Anywhere else, Tab moves focus out of the editor: forward to Depends on, back to Priority.
  - From inside a list or code block, Esc then Tab leaves.
- **Esc (D4)** steps back one layer at a time:
  1. it closes an open popover or menu (link popover, language list, "…", heading menu);
  2. otherwise it moves focus out of the editor;
  3. otherwise the form discards, as today: no confirmation is added.
  - `useFormKeys` must not discard while the editor or a popover has the key.
  - In the Ticket tab the panel's own Esc waits the same way. T-0260 adds the full-window layer.
- ⌘↵ / Ctrl+↵ saves from anywhere in the form, including inside the editor and the source view.
- Typing with an input method (`isComposing`) is never interrupted.

**Done when**
- [ ] Vitest tests for the Esc layer order and the Tab rules (a small key-routing function, tested without the DOM)
- [ ] every key behaves as above in the ⌘N form and the Ticket tab
  - *Hand check:* in SHOP-47: nest "Keyboard: …" under "Suggest addresses…" with Tab; open the link popover and press Esc
    three times (popover, editor, form); Tab out of a paragraph to Depends on.
- [ ] checks green; the commit names TKT-10 and decisions 0078, 0058

### T-0260 · The full-window view, edit and read
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-12
- **Decisions:** 0078 (D11, D4), 0058
- **Design:** crewboard.pen → 08 · Ticket editor:
  - → Row · Full-window view · edit (D11) → Full-window edit · Light / Dark;
  - → Row · Full-window view · read (D11) → Full-window read · Light / Dark;
  - → Row · Full-window view · notes (D11) → Note · Full-window view;
  - the Expand button in → Row · Markdown editor · height, narrow panel, agent (update P) → h.
- **Depends on:** T-0256, T-0257, T-0259
- **Size:** M
- **Shared files:** `markdown/fullwindow/`, the toolbar's Expand slot, `ReadMode.tsx`'s Expand slot, the overlay registry
  (one line)

**What and why**
- **Expand** (Lucide `maximize-2`) opens the description over the whole Otaq window, as a registered overlay. Collapse
  (`minimize-2`) returns.
- **Edit:**
  - a header ("SHOP-47 · What and why", the ticket title in muted text, "Editing", Collapse);
  - the same toolbar and Markdown toggle, pinned;
  - the content in a centred 760 px column;
  - Save / Cancel at the bottom right, with "⌘↵ save · Esc collapse (after closing a popover)".
  - It is **the same editor instance**: the text, selection, caret, undo history and toggle carry over both ways.
  - Save here saves the form; Cancel collapses without saving (the form keeps the edits).
- **Read:** the rendered description in the same column, with Copy as Markdown and Collapse in the header. There is no
  toolbar; Edit stays in the Ticket tab.
- **Esc:** an open popover closes first, then the full view collapses, then the T-0259 layers apply.
- There is no shortcut. Expand panel (⇧⌘E) doesn't change.

**Done when**
- [ ] Expand and Collapse keep the text, caret and undo (a Vitest test on the shared state, or the commit says how it's kept)
- [ ] matches the four full-window frames, light and dark, at 1440×900
  - *Hand check:* expand from the ⌘N form, from the Ticket tab's edit mode and from read mode; undo after collapsing still
    undoes what was typed in the full view; Esc with the link popover open closes only the popover.
- [ ] checks green; the commit names TKT-12 and decisions 0078, 0058

### T-0261 · Hand-check pass and the Windows check
- **Epic:** M15 · Markdown description editor
- **Requirements:** TKT-10, TKT-11, TKT-12
- **Decisions:** 0078, 0079, 0062
- **Design:** every frame named in T-0249–T-0260
- **Depends on:** T-0248 – T-0260
- **Size:** M
- **Shared files:** `tickets/review-M15.md`, `scripts/README.md` (only if setup notes change)

**What and why**
- Prepare every hand check of M15 in `tickets/review-M15.md`, with steps and frame names, grouped by screen. Agents don't
  tick hand checks.
- Run `scripts/check.ps1` in the **Windows VM** (VMPal, `Z:\software-factory`), and try paste and copy there with
  WebView2. The same goes for the Ubuntu VM if it's reachable.
- A VM that can't be reached is **skipped** and recorded as "not checked", not a blocker (the M14 rule).
- Fix what fails in new commits that name the ticket they fix.

**Done when**
- [ ] `tickets/review-M15.md` lists every hand check, with "How it was run" (0079) and "Platforms checked"
- [ ] `check.ps1` green in the Windows VM, or the VM is recorded as skipped
- [ ] checks green on the final tree; the commit names TKT-10, TKT-11, TKT-12 and decisions 0078, 0079

## Parallel lanes (M15)
**The rules are the Depends on lists and the shared files.** This table is only the earliest grouping. The orchestrator
decides the actual order (0079): at most 3 teammates at once; never two tickets in the same folder or file at once, except
the one-line registration files; tickets that change `package.json` or `Cargo.toml` run alone.

| Phase | Tickets (side by side within a phase) | Watch out for |
|---|---|---|
| 0 | T-0248 (alone; may stop the milestone) | `package.json`, the lock file, licences |
| 1 | T-0249 (alone) | creates the registration files |
| 2 | T-0250 ∥ T-0253 ∥ T-0254 ∥ T-0255 ∥ T-0257 | each adds one line to `extensions.ts`; T-0257 is the only one changing Rust |
| 3 | T-0252 ∥ T-0256 ∥ T-0258; T-0251 after T-0256 | T-0251, T-0256 and T-0258 all touch `src/api/editor.rs`: at most one Rust ticket at a time; T-0252 and T-0256 are frontend folders apart |
| 4 | T-0259 (alone) | `Modal.tsx`, `TicketTab.tsx`, `Editor.tsx` |
| 5 | T-0260 (alone) | the toolbar and read-mode slots |
| 6 | T-0261 (alone) | the VMs |

With at most 3 at once, phase 2 runs in two waves. A good first wave is T-0250, T-0254 and T-0257: the toolbar unblocks the
most tickets, and T-0257 is the only Rust ticket in the phase.
