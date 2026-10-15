// The description editor's extension list (d0078): one line per feature, so each M15 ticket adds its own line. The
// rich editor, read mode and the round-trip test all build from this list, so they parse and save Markdown the same
// way. Nothing here offers more than Markdown can save: underline is off (d0078: no underline). Headings keep all six
// levels so a pasted `####` survives a save; the toolbar offers H1–H3 and markdown.css draws H4–H6 like H3 (T-0249).

import type { Extensions } from "@tiptap/core";
import { TaskItem, TaskList } from "@tiptap/extension-list";
import { Placeholder } from "@tiptap/extensions";
import { Markdown } from "@tiptap/markdown";
import { StarterKit } from "@tiptap/starter-kit";

import { CodeBlock } from "./code/CodeBlock";
import { Tables } from "./table/tables";
import { ToolbarShortcuts } from "./toolbar/shortcuts";

export interface MarkdownExtensionsOptions {
  /** The muted text shown while the description is empty; a function is read again on each redraw. */
  placeholder?: string | (() => string);
}

export function markdownExtensions({ placeholder = "" }: MarkdownExtensionsOptions = {}): Extensions {
  return [
    StarterKit.configure({ codeBlock: false, underline: false, link: { openOnClick: false } }),
    TaskList,
    TaskItem.configure({ nested: true }),
    Tables,
    CodeBlock,
    Placeholder.configure({ placeholder }),
    Markdown,
    ToolbarShortcuts,
  ];
}
