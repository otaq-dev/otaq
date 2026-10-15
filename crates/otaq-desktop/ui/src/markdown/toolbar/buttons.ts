// The toolbar's buttons (d0078, *MD Toolbar · component*): one entry per button, in the order they're drawn, so each
// ticket adds or changes its own line. T-0250 fills the list and draws it; Expand (T-0260) and the Markdown toggle
// (T-0251) are slots at the toolbar's right end, not entries here. The Heading menu is drawn before the list.

import type { Editor } from "@tiptap/core";

import type { IconName } from "../../generated/IconName";
import { openLink } from "./link";

/**
 * A shortcut with ⌘ (Ctrl on Windows and Linux), as the tooltip and the "…" menu show it. The editor's own key
 * bindings do the work (Tiptap's, and `toolbarShortcuts` for the ones it lacks); this only names them.
 */
export interface Shortcut {
  /** The key as printed: `"B"`, `"8"`. */
  key: string;
  shift?: boolean;
  alt?: boolean;
}

/** The buttons between two separators. */
export type ToolbarGroup = "inline" | "link" | "lists" | "blocks";

/** One toolbar button. */
export interface ToolbarButton {
  /** A stable name, for tests and the overflow order. */
  id: string;
  /** The tooltip's name ("Bold"). */
  label: string;
  icon: IconName;
  group: ToolbarGroup;
  /** Applies the format at the caret or the selection. */
  run: (editor: Editor) => void;
  /** Whether the caret is in this format (the button's active state). */
  isActive?: (editor: Editor) => boolean;
  keys?: Shortcut;
  /** Where it goes when the toolbar is too narrow: into "…", lowest first (d0078 D8). Without it, it never moves. */
  overflow?: number;
}

export const TOOLBAR_BUTTONS: readonly ToolbarButton[] = [
  {
    id: "bold",
    label: "Bold",
    icon: "bold",
    group: "inline",
    run: (e) => e.chain().focus().toggleBold().run(),
    isActive: (e) => e.isActive("bold"),
    keys: { key: "B" },
  },
  {
    id: "italic",
    label: "Italic",
    icon: "italic",
    group: "inline",
    run: (e) => e.chain().focus().toggleItalic().run(),
    isActive: (e) => e.isActive("italic"),
    keys: { key: "I" },
  },
  {
    id: "strike",
    label: "Strikethrough",
    icon: "strikethrough",
    group: "inline",
    run: (e) => e.chain().focus().toggleStrike().run(),
    isActive: (e) => e.isActive("strike"),
    keys: { key: "X", shift: true },
    overflow: 1,
  },
  {
    id: "code",
    label: "Inline code",
    icon: "code",
    group: "inline",
    run: (e) => e.chain().focus().toggleCode().run(),
    isActive: (e) => e.isActive("code"),
    keys: { key: "E" },
  },
  {
    id: "link",
    label: "Link",
    icon: "link",
    group: "link",
    run: openLink,
    isActive: (e) => e.isActive("link"),
    keys: { key: "K" },
  },
  {
    id: "bulletList",
    label: "Bullet list",
    icon: "list",
    group: "lists",
    run: (e) => e.chain().focus().toggleBulletList().run(),
    isActive: (e) => e.isActive("bulletList"),
    keys: { key: "8", shift: true },
  },
  {
    id: "orderedList",
    label: "Numbered list",
    icon: "list-ordered",
    group: "lists",
    run: (e) => e.chain().focus().toggleOrderedList().run(),
    isActive: (e) => e.isActive("orderedList"),
    keys: { key: "7", shift: true },
  },
  {
    id: "taskList",
    label: "Task list",
    icon: "list-todo",
    group: "lists",
    run: (e) => e.chain().focus().toggleTaskList().run(),
    isActive: (e) => e.isActive("taskList"),
    keys: { key: "9", shift: true },
  },
  {
    id: "blockquote",
    label: "Quote",
    icon: "text-quote",
    group: "blocks",
    run: (e) => e.chain().focus().toggleBlockquote().run(),
    isActive: (e) => e.isActive("blockquote"),
    keys: { key: "B", shift: true },
    overflow: 2,
  },
  {
    id: "codeBlock",
    label: "Code block",
    icon: "square-code",
    group: "blocks",
    run: (e) => e.chain().focus().toggleCodeBlock().run(),
    isActive: (e) => e.isActive("codeBlock"),
  },
  {
    id: "table",
    label: "Table",
    icon: "table",
    group: "blocks",
    run: (e) => e.chain().focus().insertTable({ rows: 2, cols: 2, withHeaderRow: true }).run(),
    isActive: (e) => e.isActive("table"),
    overflow: 3,
  },
  {
    id: "horizontalRule",
    label: "Horizontal rule",
    icon: "minus",
    group: "blocks",
    run: (e) => e.chain().focus().setHorizontalRule().run(),
    overflow: 4,
  },
];
