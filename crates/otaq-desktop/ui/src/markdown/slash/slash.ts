// `/` for blocks (SHT-03, d0078; T-0465; *25 → Editing the description*: "Type / for blocks"): a `/` typed at the start
// of an empty paragraph opens a list of the blocks the toolbar makes (text, headings, lists, quote, code block, table,
// rule); typing after it filters the list, ↑ ↓ move, ↵ (or a click) turns the paragraph into that block, Esc closes
// the list and keeps the `/`. Nothing here goes beyond what the toolbar does, so the stored value stays Markdown.
// The list's names are the toolbar's own.

import type { Editor } from "@tiptap/core";
import type { EditorState } from "@tiptap/pm/state";

import type { IconName } from "../../generated/IconName";
import { TOOLBAR_BUTTONS } from "../toolbar/buttons";

/** One block the list offers. */
export interface SlashItem {
  id: string;
  label: string;
  icon: IconName;
  run: (editor: Editor) => void;
}

const heading = (level: 1 | 2 | 3): SlashItem => ({
  id: `heading${String(level)}`,
  label: `Heading ${String(level)}`,
  icon: "hash",
  run: (e) => e.chain().focus().setHeading({ level }).run(),
});

/** Text, three headings, then the toolbar's list and block buttons in its order. */
export const SLASH_ITEMS: readonly SlashItem[] = [
  {
    id: "paragraph",
    label: "Text",
    icon: "a-large-small",
    run: (e) => e.chain().focus().setParagraph().run(),
  },
  heading(1),
  heading(2),
  heading(3),
  ...TOOLBAR_BUTTONS.filter((b) => b.group === "lists" || b.group === "blocks").map((b) => ({
    id: b.id,
    label: b.label,
    icon: b.icon,
    run: b.run,
  })),
];

/** The `/` typed in a paragraph: where it starts and what follows it so far. */
export interface SlashQuery {
  /** The `/`'s position in the document. */
  from: number;
  /** The caret's position (the end of the query). */
  to: number;
  /** What was typed after the `/`. */
  query: string;
}

/**
 * The open query at the caret, or `null`: the caret is in a plain paragraph (not a list item's, a quote's or a code
 * block's) whose text up to the caret is `/` and letters, digits or spaces, with no text after the caret.
 */
export function slashAt(state: Pick<EditorState, "selection">): SlashQuery | null {
  const { selection } = state;
  if (!selection.empty) return null;
  const { $from } = selection;
  const parent = $from.parent;
  if (parent.type.name !== "paragraph" || $from.depth !== 1) return null;
  if ($from.parentOffset !== parent.content.size) return null;
  const text = parent.textBetween(0, $from.parentOffset, undefined, "￼");
  const match = /^\/([\p{L}\p{N} ]{0,24})$/u.exec(text);
  if (!match) return null;
  return { from: $from.start(), to: $from.pos, query: match[1] ?? "" };
}

/** The items whose name starts a word with the query (case ignored), in the list's order. */
export function filterItems(query: string, items: readonly SlashItem[] = SLASH_ITEMS): SlashItem[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [...items];
  return items.filter((item) =>
    item.label
      .toLowerCase()
      .split(/\s+/)
      .some((word, i, words) => word.startsWith(q) || words.slice(i).join(" ").startsWith(q)),
  );
}

/** The highlighted row after ↑ / ↓ (`delta` -1 / 1), round the ends. */
export function stepItem(at: number, delta: 1 | -1, count: number): number {
  if (count === 0) return 0;
  return (at + delta + count) % count;
}

/** Turns the paragraph into `item`'s block: the `/` and the query go, then the block's own command runs. */
export function applyItem(editor: Editor, slash: SlashQuery, item: SlashItem): void {
  editor.chain().focus().deleteRange({ from: slash.from, to: slash.to }).run();
  item.run(editor);
}
