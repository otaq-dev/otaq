# 0078. Ticket description: a rich Markdown editor (Tiptap), MIT packages plus highlight.js

- **Status:** Accepted
- **Date:** 2026-10-15
- **Related:** 0019, 0054, 0067, 0071, 0075

## Context
The ticket description ("What and why", 0019, 0067) is a plain text area today. Read mode renders only paragraphs, headings,
code blocks and flat lists: links, bold and tables aren't shown, and task boxes are dropped. People paste descriptions from
web pages, Google Docs, Slack and Claude, and copy them back out. The formatting should survive both ways. Agents read and
write the same text with `otaq show` / `otaq update --stdin`, so the stored value has to stay Markdown.

## Decision
- **"What and why" is a rich (WYSIWYG) Markdown editor built on Tiptap v3** with its official Markdown extension. Read mode
  uses the same renderer with editing turned off.
- **The stored value stays plain Markdown** (GitHub-flavoured: tables and task lists). Nothing the editor offers goes beyond
  what Markdown can save: no colours, font sizes, underline, alignment, images or embeds.
- **Packages: MIT only, no Tiptap Pro extensions.** The packages are `@tiptap/core`, `@tiptap/react`, `@tiptap/starter-kit`,
  `@tiptap/markdown`, `@tiptap/extension-list`, `@tiptap/extension-table`, `@tiptap/extensions` and
  `@tiptap/extension-code-block-lowlight`, plus `lowlight`.
- **One exception: `highlight.js` (BSD-3-Clause)** colours the code in code blocks. It is used through `lowlight`. BSD-3 is
  permissive like MIT. Its notice ships in the third-party notices (0071), and its authors' names aren't used to promote
  Otaq. Any other non-MIT package needs its own decision.
- **The design is update P** (`design-briefs/CHANGES-2026-10-15-p.md`), with its audit in
  `design-briefs/AUDIT-2026-10-15-p.md`. It covers:
  - the toolbar and the "Markdown" source toggle;
  - links, tables and code blocks with a language label;
  - "Copy as Markdown" in read mode;
  - paste that keeps formatting, and ⌘⇧V for plain text;
  - copying a selection as both rich text and Markdown;
  - the rendered / Markdown compare for "Updated by agent".
  Its open questions (D1–D10) and the full-window view (D11) are all decided below.
- **⌘K / Ctrl+K inserts a link while the editor has focus**; outside the editor it stays Search / filter (0058, refined).
- **Tab and ⇧Tab indent and outdent inside lists and code blocks**; everywhere else in the editor Tab moves focus out of
  it. From inside a list or code block, Esc then Tab leaves.
- **Esc steps back one layer at a time:** first it closes an open popover or menu, then it moves focus out of the editor,
  and only then does it discard the form, as Esc does today.
- **Height:** in the ⌘N form the editor grows with its content up to 480 px, then scrolls inside, with the toolbar pinned.
  When the window is too short, the form body scrolls and the Save / Cancel footer stays pinned. In the Ticket tab the
  editor may grow until it fills the tab.
- **Dropped paste content is reported:** when a paste leaves out images or embeds, a muted line under the content says how
  many ("1 image left out: descriptions are Markdown only") until the next edit. Dropped colours and fonts aren't reported.
- **Full-window view:** an Expand button (Lucide "maximize") opens the description over the whole Otaq window.
  - In edit mode it sits at the right end of the toolbar, next to the Markdown toggle. The full view has a small header
    ("SHOP-47 · What and why"), the same toolbar and toggle, Save, and Collapse. It's the same field, so the text carries over.
  - In read mode it sits next to "Copy as Markdown" and shows the rendered description.
- **Where read mode's buttons go:** "Copy as Markdown" and Expand sit at the right end of the "What and why" label row in the
  Ticket tab, in every ticket state. They are not in the action bar, which keeps only ticket actions (Start, Edit, Approve…).
- **Narrow toolbar:** when the toolbar doesn't fit, Strikethrough, Quote, Table and Horizontal rule move into "…" first.
  Expand and the Markdown toggle never move.
- **The Markdown toggle is remembered per user**, in the app's settings (0023): every editor opens in the mode last chosen,
  rich or source.
  - Esc steps back through it as well: popover, then the full view, then the rest as before.
  - It has no shortcut. Expand panel (⇧⌘E, 0058) stays as it is.

## Alternatives considered
- **Milkdown** (MIT): built entirely around Markdown and the strongest at keeping text unchanged after a save, but it has a
  smaller ecosystem and is harder to restyle. It's the fallback if Tiptap changes agent-written text in the round-trip test
  below.
- **MDXEditor:** heavy, built around MDX, and comes with its own look.
- **BlockNote:** its Markdown conversion loses information.
- **CodeMirror 6:** perfect copy and paste, but it shows the raw source, not "what you see is what you get".
- **Code blocks without colours (MIT only), or Shiki (MIT, no official Tiptap extension):** rejected in favour of
  highlight.js as a BSD-3 exception.

## Consequences
- **Round-trip test before building it out:** load real agent-written descriptions, save them, and diff the result. If the
  editor rewrites Markdown (list markers, table layout, escaping), switch to Milkdown.
- The npm notices (0071, 0075) gain the Tiptap packages, `lowlight` and `highlight.js`.
- The Rust Markdown view model used by read mode today (`otaq-view` `view_models::markdown`) is no longer needed for the
  description once read mode uses the editor.

## Changelog
- 2026-10-15: created (user decision: Tiptap v3, MIT packages only, highlight.js accepted as a BSD-3-Clause exception)
- 2026-10-15: ⌘K means Link inside the editor (audit D2, user decision; 0058 refined).
- 2026-10-15: Tab / ⇧Tab indent and outdent inside lists as well as code blocks (audit D3, user decision).
- 2026-10-15: Esc steps back one layer at a time (popover, then editor, then the form) (audit D4, user decision).
- 2026-10-15: editor max height 480 px in the ⌘N form; the form body scrolls with the footer pinned (audit D5, user decision).
- 2026-10-15: a muted note reports images and embeds left out of a paste (audit D6, user decision).
- 2026-10-15: full-window view of the description from an Expand button, in edit and read mode (user decision).
- 2026-10-15: Copy as Markdown and Expand sit on the "What and why" label row in every state (audit D7, user decision).
- 2026-10-15: overflow order for a narrow toolbar; Expand and the Markdown toggle always visible (audit D8, user decision).
- 2026-10-15: the Markdown toggle is remembered per user (audit D9, user decision).
- 2026-10-15: older design frames that show the description switch to the new editor (audit D10, user decision); all of update P's open questions are decided.
