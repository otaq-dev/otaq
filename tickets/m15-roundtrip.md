# M15 · The round-trip gate (T-0248)

**Decision:** on 2026-10-15 the user chose option 2 (Q48, 0078 refined): keep Tiptap, accept the spelling changes and
M1, M2 and M4 as known; M3 (a fence inside a fence) is fixed in T-0254. The round-trip test records the accepted
outputs, so it passes, and fails on any other difference.

Decision 0078: "Round-trip test before building it out: load real agent-written descriptions, save them, and diff the
result. If the editor rewrites Markdown (list markers, table layout, escaping), switch to Milkdown."

**Result: the editor rewrites Markdown, so the gate stopped for a decision.** 35 of 90 samples come back different. In the
40 agent-written samples, 34 are unchanged and the other 6 change only their spelling. The meaning changes only in 6
edge cases: HTML tags, an image, and a code block that contains a fence (which breaks), plus a space at the edge of
inline code.

## How it was tested
- `crates/otaq-desktop/ui/src/markdown/roundtrip/roundtrip.test.ts` (in `npm run test`) builds the editor headless from
  `markdown/extensions.ts`, the list T-0249 takes over: StarterKit (underline and its own code block off, links with
  `openOnClick` off), task lists (nested), TableKit, CodeBlockLowlight (highlight.js common languages), Placeholder and
  `@tiptap/markdown`. All at 3.32.0.
- Each sample is loaded as Markdown (`contentType: "markdown"`) and saved with `editor.getMarkdown()`, with no edit.
  The test expects the saved text to be the sample, character for character, or, for the 35 accepted differences, the
  recorded `saved` text.
- It runs with a DOM (happy-dom), as the app's WebView has one. This matters for HTML: without a DOM, `@tiptap/markdown`
  keeps HTML tags as escaped text (`&lt;kbd&gt;Cmd&lt;/kbd&gt;`); with one, it reads them and drops what the schema
  doesn't know.
- The samples (`roundtrip/samples.json`):
  - **example (1):** the SHOP-47 description of update P, as in `tickets/m15-tickets.md`;
  - **agent (40):** every sixth ticket of `tickets/v0.1-tickets.md` (T-0001, T-0007, … T-0241), assembled exactly as
    `otaq create -f` stores them (`otaq-core` `board/batch.rs`: "What and why", then the "## Done when" list, then the notes);
  - **edge (49):** every edge case T-0248 lists, plus a few found while checking the agent text (a code span with a space
    at its edge, a wrapped list item, `&` and `<placeholder>` text, a fence inside a fence, indented code, an empty text).
- "Meaning" was checked by rendering the input and the output with GitHub-flavoured Markdown (marked, the parser
  `@tiptap/markdown` uses) and comparing the HTML. "Spelling" means the same HTML (apart from the noted spacing).
- **Second save:** every output was loaded and saved again. All of them come back unchanged, except the two broken
  fences (M3), which change again on every save.

## Counts

| Samples | Unchanged | Spelling only | Meaning changed |
|---|---|---|---|
| example (update P) | 0 of 1 | 1 | 0 |
| agent (v0.1 tickets) | **34 of 40** | 6 | 0 |
| edge cases | 21 of 49 | 22 | 6 |
| **all** | **55 of 90** | **29** | **6** |

Edge cases that come back unchanged: `1.` lists, nested lists (two levels, 2-space indent), task lists (flat and
nested), two-space hard breaks, fences with and without a language, trailing spaces, a text without a final newline,
`*italic*` / `**bold**`, `~~strike~~`, block quotes, `---` rules, inline code with backticks inside, a list followed by a
paragraph, a link with a title, a list starting at 3, a "2." that isn't a list, an empty text, heading levels 1–4.

**Beyond the gate, all 247 v0.1 tickets:** 188 of 247 (76 %) come back unchanged. The others change only spelling, with
4 exceptions: the space at the edge of inline code moves (M4). In T-0221 this is a real change (`` `is:needs-me ` `` loses
the space the text asks you to type). In T-0058, T-0215 and T-0224 the source already had a stray backtick (the key
⇧⌘\`), so the original didn't render as meant either. None of the 247 has HTML tags, images or a fence inside a fence.

## Differences, grouped by kind
Each kind lists the samples it appears in. A sample can show several kinds. The exact input and output of every
sample are under "Every difference" below.

### Meaning changed

| # | Kind | Samples | Example |
|---|---|---|---|
| M1 | **HTML tags are dropped; their text stays.** Inline tags and HTML blocks. | edge: inline-html, html-block | `Press <kbd>Cmd</kbd>+<kbd>K</kbd>` → `Press Cmd+K`; a `<details><summary>More</summary>` block becomes two plain paragraphs |
| M2 | **An image becomes its alt text; the URL is lost.** (0078 has no images, so there is no image node.) | edge: image | `![logo](https://example.com/logo.png)` → `logo` |
| M3 | **A code block that contains a ` ``` ` line breaks.** The fence is always written as ` ``` `, so a `~~~` or ` ```` ` fence around a Markdown example closes early; the rest leaks out as text, and every later save breaks it further. | edge: fence-containing-fence, four-backtick-fence | ``~~~md`` / ` ```ts ` / … / `~~~` → ` ```md ` / ` ```ts ` / … : the inner fence ends the block |
| M4 | **A space at the edge of inline code moves outside it.** | edge: code-span-edge-space (all tickets: T-0221; T-0058, T-0215, T-0224 with a stray backtick) | `` `is:needs-me ` `` → `` `is:needs-me` `` followed by two spaces |

### Spelling only (same meaning)

| # | Kind | Samples | Example |
|---|---|---|---|
| S1 | A blank line is added after a heading | example | `## Why`↵`Users…` → `## Why`↵↵`Users…` |
| S2 | Tables are re-padded into aligned columns; the separator row is rewritten (alignment colons kept); two blank lines before and after the table, and a leading newline when the text starts with one | example, table-alignment, table-padded | `\|---\|---\|` → `\| -------- \| ------- \|` |
| S3 | `*` and `+` bullets become `-` | star-bullets, plus-bullets | `* one` → `- one` |
| S4 | `1)` becomes `1.` | paren-ordered | `1) first` → `1. first` |
| S5 | Escaping: `_` inside a word gains a backslash; an unneeded `\#` loses it | T-0019, T-0241, escaped-characters | `needs_restack` → `needs\_restack` |
| S6 | `&`, `<`, `>` in text become `&amp;`, `&lt;`, `&gt;` (not inside code). The rendered text is the same in Otaq. Strict GFM reads the input's `<agent name>` as an HTML tag and hides it, so the output shows what the author meant. | T-0204, T-0229, T-0235, ampersand-and-angle-text | `Review & resume` → `Review &amp; resume`; `by <agent name>` → `by &lt;agent name&gt;` |
| S7 | Inline code wrapped over two lines is joined onto one line (the line break inside code is a space either way) | T-0160, T-0241 | `` `cargo clippy --workspace``↵``--all-targets …` `` → one line |
| S8 | List indentation is normalised: wrapped lines inside a list item lose their hanging indent (4 → 2 in a nested item, 2 → 0 at the top level), and 4-space nesting becomes 2 | T-0229, T-0241, list-item-wrapped, nested-list-four-spaces | `- a long item…`↵`  onto a second line` → `…`↵`onto a second line` |
| S9 | A blank line is inserted before a task list that follows a paragraph or plain bullets (the editor keeps `- [ ]` items as their own list). In strict GFM, T-0241's single list becomes a "loose" list: same items, a little more space between them. | T-0229, T-0241 | `- the header…`↵`- [x] the header…` → a blank line between |
| S10 | A loose list becomes tight (the blank lines between items go) | loose-list | `- one`↵↵`- two` → `- one`↵`- two` |
| S11 | `~~~` fences and indented code become ` ``` ` fences | tilde-fence, indented-code | `~~~` → ` ``` ` |
| S12 | Setext headings become `#` headings | setext-h1, setext-h2 | `Title`↵`=====` → `# Title` |
| S13 | Autolinks become explicit links | autolink-angle, autolink-bare | `<https://example.com/page>` → `[https://example.com/page](https://example.com/page)` |
| S14 | `_italic_` / `__bold__` become `*italic*` / `**bold**` | emphasis-underscore | |
| S15 | A `***` rule becomes `---` | horizontal-rule-stars | |
| S16 | A reference link becomes an inline link; its definition line goes | reference-link | `[the docs][docs]` … `[docs]: https://example.com` → `[the docs](https://example.com)` |
| S17 | A backslash hard break becomes two trailing spaces | hard-break-backslash | `line one\`↵ → `line one␣␣`↵ |
| S18 | The final newline goes; CRLF becomes LF | final-newline, crlf | |

In the 40 agent samples only S5–S9 occur. Among all 247 tickets, the most common are S6 (`&` and `<…>`, 32 tickets), S7
(18), S5 (16) and S8/S9 (5).

## Options tried before calling these differences
- `@tiptap/markdown` `indentation`: `size: 2` (the default) and `size: 4`. With 4, nested-list-four-spaces round-trips,
  but every 2-space nested list changes (nested-two-levels, nested-task-list, T-0229, T-0241), and agents write 2.
  Kept 2. `style: "tab"` doesn't apply.
- `markedOptions`: GFM is already on. `breaks` and `pedantic` don't touch any of these kinds.
- A DOM or none: see "How it was tested" (M1 is "tags escaped as text" without a DOM, "tags dropped" with one).
- There is **no option** in 3.32.0 for the rest. These spellings are fixed in each extension's `renderMarkdown`:
  - the bullet `- ` and the ordered `. ` (`@tiptap/extension-list` `item/list-item.ts`);
  - the ` ``` ` fence (`@tiptap/extension-code-block` `code-block.ts`);
  - the table padding and its surrounding blank lines (`@tiptap/extension-table` `utilities/markdown.ts`);
  - the escaping and `&amp;` encoding (`@tiptap/markdown` `MarkdownManager.ts`, `encodeTextForMarkdown`);
  - the blank line between blocks.

  Each could change only by overriding that extension's `renderMarkdown` in our own code. That needs no new package.

## What it means for Otaq
- T-0249 never re-saves an untouched description (its "Done when": "an untouched description saves its original
  string"). Agent text is rewritten only when a person edits that description in the rich editor, and then once: a
  second save gives the same text (except M3).
- Agents still read plain Markdown after such a save. The changes they would notice are `&amp;` / `&lt;…&gt;` (S6) and
  `\_` (S5).
- M3 is the only change that gets worse on each save. It can be fixed in our own code without a package: a code block
  `renderMarkdown` that picks a fence longer than any backtick run inside. This belongs in `markdown/code/` (T-0254).
- M1, M2 and M4 don't occur in any of the 247 agent tickets except T-0221 (M4).

## Recommendation
**Keep Tiptap**, accept the spelling changes S1–S18, and fix M3 in T-0254 (the fence length, our own code). Accept
M1, M2 and M4 as known: none of them is in the 40 agent samples, and they are rare in the 247 tickets. The alternative,
Milkdown (0078), was not measured here, because no package beyond T-0248's list may be added. Its serialiser (remark)
also normalises list markers, emphasis and escaping by default, with options to choose them.

The test now records each accepted difference as the sample's expected output (`saved` in `samples.json`), so it passes
and flags any new difference after a package update. The two M3 samples record today's broken output until T-0254
fixes the fence.

## Every difference
Edge cases and the example show the exact strings (JSON-escaped, so newlines, CRLF and trailing spaces are visible).
Agent samples show only the changed lines, as a unified diff of the stored body.

#### example · update-p

````text
in:  "## Why\nUsers mistype addresses at checkout; **12%** of failed deliveries trace back to it (see [the support report](https://example.com/report)).\n\n## What\n- Suggest addresses as the user types in *Street*\n- Keyboard: ↑ ↓ to move, Enter to pick, Esc to close\n\n## Done when\n- [x] Suggestions appear after 3 characters\n- [ ] Arrow keys move the highlight\n- [ ] Works with the existing `AddressForm` validation\n\n| Field | Source |\n|---|---|\n| Street | Places API |\n| Postcode | Places API, editable |\n\n```ts\nconst results = await places.autocomplete(query, { country: \"TR\" });\n```"
out: "## Why\n\nUsers mistype addresses at checkout; **12%** of failed deliveries trace back to it (see [the support report](https://example.com/report)).\n\n## What\n\n- Suggest addresses as the user types in *Street*\n- Keyboard: ↑ ↓ to move, Enter to pick, Esc to close\n\n## Done when\n\n- [x] Suggestions appear after 3 characters\n- [ ] Arrow keys move the highlight\n- [ ] Works with the existing `AddressForm` validation\n\n\n| Field    | Source               |\n| -------- | -------------------- |\n| Street   | Places API           |\n| Postcode | Places API, editable |\n\n\n```ts\nconst results = await places.autocomplete(query, { country: \"TR\" });\n```"
````

#### agent · T-0019

Changed lines (`-` input, `+` output):

````diff
@@ -13,3 +13,3 @@
 - a `Flags` set covers every flag in §7.2 (pinned, sent back · N · round K, conflicts with `<base>`, changed since review, N messages waiting, Run running, updated by agent, answering, last signal, tokens, PR #N, PR provider not detected)
-- no stacking states exist (no Paused, no needs_restack) (d0049)
+- no stacking states exist (no Paused, no needs\_restack) (d0049)
 - tests: unit tests for key parsing/formatting and round-tripping every enum through its stored string form
````

#### agent · T-0160

Changed lines (`-` input, `+` output):

````diff
@@ -2,4 +2,3 @@
 machines** that the user sets up as the last step of v0.1. Make the check script work there: `scripts/check.sh` runs unchanged
-on Linux, and a `scripts/check.ps1` runs the same three steps (`cargo fmt --all --check`, `cargo clippy --workspace
---all-targets -- -D warnings`, `cargo test --workspace`) on Windows. Write down how to set up each VM (Rust, git, the GPUI system
+on Linux, and a `scripts/check.ps1` runs the same three steps (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`) on Windows. Write down how to set up each VM (Rust, git, the GPUI system
 dependencies on Linux, the Visual Studio build tools on Windows) so the run can be repeated.
````

#### agent · T-0204

Changed lines (`-` input, `+` output):

````diff
@@ -5,3 +5,3 @@
 agent closes without asking; Ended normally; **Ended by restart · Resume**; **Recently ended** (with entries and empty).
-Tickets a Project agent creates appear in Backlog live with a subtle highlight, and cards say "by <agent name>".
+Tickets a Project agent creates appear in Backlog live with a subtle highlight, and cards say "by &lt;agent name&gt;".
````

#### agent · T-0229

Changed lines (`-` input, `+` output):

````diff
@@ -12,3 +12,3 @@
 in Dev, SHOP-20 and SHOP-21 Queued) at 1440 × 900, Done folded to its rail, hover SHOP-20, then click it and move the
-    pointer off the board: compare with *03 · Board & columns → Chain highlight · hover or select SHOP-20* and, in
+    pointer off the board: compare with *03 · Board &amp; columns → Chain highlight · hover or select SHOP-20* and, in
     Crewboard Dark, *Dark · chain highlight* (SHOP-20 2 px accent, the other members 1.5 px accent, the rest at 35 %,
@@ -18,14 +18,15 @@
     dims. Start a drag: the highlight goes.
+
 - [ ] each storyboard 1–6 plays out on the new UI as drawn (by hand, with the sample tickets of `design-briefs/00-README.md`)
-  - Hand check, against *12 · Stack & dependencies*: 1 · create SHOP-18…21 with their links: ⛓ badges, the Stack list of
-    SHOP-20 (*Stack list · detail*); 2 · drag all four to Ready for Dev: SHOP-18/19 start, SHOP-20 "waits for SHOP-18,
-    SHOP-19 to merge", then "(SHOP-18 in Review)" once SHOP-18 submits (parentheses, Q19); 3 · Local merge SHOP-18:
-    "waits for SHOP-19 to merge (in Review)"; 4 · merge SHOP-19: SHOP-20 starts, the toast, and the card's callout "from
-    main @ <sha> · all dependencies merged"; 5 · redo with Open PR on SHOP-18: "waits for SHOP-18 · PR #N open", then Mark
-    merged starts SHOP-20; 6 · Mark closed on SHOP-19: SHOP-20 "Parent closed" with the 04 variant's longer callout
-    (the storyboard abbreviates it to "parent SHOP-19 closed") and the inbox's Reopen parent / Remove dependency.
+  - Hand check, against *12 · Stack &amp; dependencies*: 1 · create SHOP-18…21 with their links: ⛓ badges, the Stack list of
+  SHOP-20 (*Stack list · detail*); 2 · drag all four to Ready for Dev: SHOP-18/19 start, SHOP-20 "waits for SHOP-18,
+  SHOP-19 to merge", then "(SHOP-18 in Review)" once SHOP-18 submits (parentheses, Q19); 3 · Local merge SHOP-18:
+  "waits for SHOP-19 to merge (in Review)"; 4 · merge SHOP-19: SHOP-20 starts, the toast, and the card's callout "from
+  main @ &lt;sha&gt; · all dependencies merged"; 5 · redo with Open PR on SHOP-18: "waits for SHOP-18 · PR #N open", then Mark
+  merged starts SHOP-20; 6 · Mark closed on SHOP-19: SHOP-20 "Parent closed" with the 04 variant's longer callout
+  (the storyboard abbreviates it to "parent SHOP-19 closed") and the inbox's Reopen parent / Remove dependency.
 - [ ] tests: chain view-model tests kept; by hand against the named frames
   - Automated part done: the chain tests in `chain.rs` are kept, with the legend and per-card chains added; the six
-    storyboards reach the page in `board_page.rs` (`the_storyboards_reach_the_page`); `chain.test.ts` covers the hover
-    rules, the looks and the arrows. The frame comparison is the two hand checks above.
+  storyboards reach the page in `board_page.rs` (`the_storyboards_reach_the_page`); `chain.test.ts` covers the hover
+  rules, the looks and the arrows. The frame comparison is the two hand checks above.
 - [x] checks green (`scripts/check.sh`); commit names DEP-03, DEP-05, CLS-04, UI-02 and decisions 0006, 0049, 0044, 0046
````

#### agent · T-0235

Changed lines (`-` input, `+` output):

````diff
@@ -12,3 +12,3 @@
 - after a simulated restart the summary lists the interrupted tickets with Resume; nothing resumes by itself
-- *Hand check:* start two tickets whose agent is an ordinary long command, then `crewboard service stop` while they run (or kill the service) and reopen the app: the red banner "2 sessions were interrupted" and, beside it, the summary sheet vs *16 → Opening after a reboot* (every row ticked, project chip, key, title, "was Agent Working · 311k", Resume; "Resume selected (2)"; Later). Nothing resumes until a click. Untick one → "Resume selected (1)". Resume one → "SHOP-25 · Resuming…" then "SHOP-19 · Resumed" (*col → Resume · in progress → resumed → failed*); a resume whose conversation is gone shows "Couldn’t resume" with Retry and Start from scratch. Later (or Esc) closes it; the banner's Review & resume opens it again. With 7+ interrupted tickets the rest fold into "+N" (click the line to list them). An orange ticket (Asks you) and a Ready for Review ticket show in the note box (Q44: headed "NOT IN RESUME SELECTED"). A normal reopen while the service kept running shows only the "Reconnected" toast (*Reopening the app normally*).
+- *Hand check:* start two tickets whose agent is an ordinary long command, then `crewboard service stop` while they run (or kill the service) and reopen the app: the red banner "2 sessions were interrupted" and, beside it, the summary sheet vs *16 → Opening after a reboot* (every row ticked, project chip, key, title, "was Agent Working · 311k", Resume; "Resume selected (2)"; Later). Nothing resumes until a click. Untick one → "Resume selected (1)". Resume one → "SHOP-25 · Resuming…" then "SHOP-19 · Resumed" (*col → Resume · in progress → resumed → failed*); a resume whose conversation is gone shows "Couldn’t resume" with Retry and Start from scratch. Later (or Esc) closes it; the banner's Review &amp; resume opens it again. With 7+ interrupted tickets the rest fold into "+N" (click the line to list them). An orange ticket (Asks you) and a Ready for Review ticket show in the note box (Q44: headed "NOT IN RESUME SELECTED"). A normal reopen while the service kept running shows only the "Reconnected" toast (*Reopening the app normally*).
 - Cleanup lists leftovers and deletes only after the confirmation
````

#### agent · T-0241

Changed lines (`-` input, `+` output):

````diff
@@ -3,4 +3,3 @@
 confirmation); they count in the header and the project switcher. The two notifications ("shop-web · Project agent asks a
-question" / "needs permission") are checked by hand. **Calls:** `inbox.list` → `project_agents`, `project_agent.answer {id,
-choice, clear}` (`needs_confirmation` with unsent text), `session.focus`; events `inbox_changed`, `project_agent_changed`.
+question" / "needs permission") are checked by hand. **Calls:** `inbox.list` → `project_agents`, `project_agent.answer {id, choice, clear}` (`needs_confirmation` with unsent text), `session.focus`; events `inbox_changed`, `project_agent_changed`.
 
@@ -15,8 +14,9 @@
     clicking the row opens the drawer on that agent's tab
+
 - [x] the header and switcher counts include waiting Project agents
 - [ ] tests: view-model test for the item (label, answer, confirmation); by hand against the named frames, including the two notifications
-  - view-model and Vitest tests done (`inbox_page.rs` project_agents_wait_in_group_1, a_project_agents_unsent_text_question_names_the_agent; `switcher.rs`; `inbox.test.ts` "Project agent items")
+  - view-model and Vitest tests done (`inbox_page.rs` project\_agents\_wait\_in\_group\_1, a\_project\_agents\_unsent\_text\_question\_names\_the\_agent; `switcher.rs`; `inbox.test.ts` "Project agent items")
   - hand check: with the window in the background, a Project agent question and a permission prompt (e.g. ask it to run
-    `gh issue list`) show *11 → OS notifications → macOS · M12* ("shop-web · Project agent asks a question" / "needs
-    permission"); a click brings the app forward on the inbox
+  `gh issue list`) show *11 → OS notifications → macOS · M12* ("shop-web · Project agent asks a question" / "needs
+  permission"); a click brings the app forward on the inbox
 - [x] checks green (`scripts/check.sh`); commit names PAG-07, ATT-06, ATT-10, ATT-11 and decisions 0068, 0016, 0042
````

#### edge · star-bullets

````text
in:  "* one\n* two\n* three"
out: "- one\n- two\n- three"
````

#### edge · plus-bullets

````text
in:  "+ one\n+ two\n+ three"
out: "- one\n- two\n- three"
````

#### edge · paren-ordered

````text
in:  "1) first\n2) second\n3) third"
out: "1. first\n2. second\n3. third"
````

#### edge · escaped-characters

````text
in:  "Literal \\*stars\\* and \\_underscores\\_ and a \\# hash."
out: "Literal \\*stars\\* and \\_underscores\\_ and a # hash."
````

#### edge · inline-html

````text
in:  "Press <kbd>Cmd</kbd>+<kbd>K</kbd> to search, or see <span>this</span>."
out: "Press Cmd+K to search, or see this."
````

#### edge · html-block

````text
in:  "<details>\n<summary>More</summary>\n\nHidden text.\n\n</details>"
out: "More\n\nHidden text.\n\n"
````

#### edge · hard-break-backslash

````text
in:  "line one\\\nline two"
out: "line one  \nline two"
````

#### edge · table-alignment

````text
in:  "| Left | Centre | Right |\n|:---|:---:|---:|\n| a | b | c |\n| dd | ee | ff |"
out: "\n| Left | Centre | Right |\n| :---- | :------: | -----: |\n| a    | b      | c     |\n| dd   | ee     | ff    |\n"
````

#### edge · table-padded

````text
in:  "| Field    | Source     |\n| -------- | ---------- |\n| Street   | Places API |"
out: "\n| Field  | Source     |\n| ------ | ---------- |\n| Street | Places API |\n"
````

#### edge · tilde-fence

````text
in:  "~~~\nplain code\n~~~"
out: "```\nplain code\n```"
````

#### edge · setext-h1

````text
in:  "Title\n=====\n\nText."
out: "# Title\n\nText."
````

#### edge · setext-h2

````text
in:  "Subtitle\n--------\n\nText."
out: "## Subtitle\n\nText."
````

#### edge · autolink-angle

````text
in:  "See <https://example.com/page> for details."
out: "See [https://example.com/page](https://example.com/page) for details."
````

#### edge · autolink-bare

````text
in:  "See https://example.com/page for details."
out: "See [https://example.com/page](https://example.com/page) for details."
````

#### edge · final-newline

````text
in:  "Text with a final newline.\n"
out: "Text with a final newline."
````

#### edge · crlf

````text
in:  "## Heading\r\n\r\n- one\r\n- two\r\n\r\nText.\r\n"
out: "## Heading\n\n- one\n- two\n\nText."
````

#### edge · emphasis-underscore

````text
in:  "Some _italic_ and __bold__ text."
out: "Some *italic* and **bold** text."
````

#### edge · horizontal-rule-stars

````text
in:  "Above\n\n***\n\nBelow"
out: "Above\n\n---\n\nBelow"
````

#### edge · loose-list

````text
in:  "- one\n\n- two\n\n- three"
out: "- one\n- two\n- three"
````

#### edge · reference-link

````text
in:  "See [the docs][docs].\n\n[docs]: https://example.com"
out: "See [the docs](https://example.com)."
````

#### edge · image

````text
in:  "![logo](https://example.com/logo.png)"
out: "logo"
````

#### edge · fence-containing-fence

````text
in:  "~~~md\n```ts\nconst a = 1;\n```\n~~~"
out: "```md\n```ts\nconst a = 1;\n```\n```"
````

#### edge · four-backtick-fence

````text
in:  "````\n```\ninner\n```\n````"
out: "```\n```\ninner\n```\n```"
````

#### edge · indented-code

````text
in:  "Text.\n\n    indented code\n    second line"
out: "Text.\n\n```\nindented code\nsecond line\n```"
````

#### edge · nested-list-four-spaces

````text
in:  "- parent\n    - child\n    - child two"
out: "- parent\n  - child\n  - child two"
````

#### edge · code-span-edge-space

````text
in:  "Type `is:needs-me ` (with the space)."
out: "Type `is:needs-me`  (with the space)."
````

#### edge · list-item-wrapped

````text
in:  "- a long item that the agent wrapped\n  onto a second line\n- next"
out: "- a long item that the agent wrapped\nonto a second line\n- next"
````

#### edge · ampersand-and-angle-text

````text
in:  "Search & filter, and cards say \"by <agent name>\"."
out: "Search &amp; filter, and cards say \"by &lt;agent name&gt;\"."
````
