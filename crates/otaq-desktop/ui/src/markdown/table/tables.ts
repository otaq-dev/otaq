// Tables in the description editor (T-0253, d0078): simple tables only, which is what Markdown can store: one header
// row, no merged cells, no alignment controls. The table bar's actions live here, apart from the bar that draws them,
// so each one can be tested on the Markdown it saves.

import { type Editor, Extension } from "@tiptap/core";
import { Table, TableCell, TableHeader, TableRow } from "@tiptap/extension-table";
import type { EditorState, Selection, Transaction } from "@tiptap/pm/state";
import { isInTable, selectedRect } from "@tiptap/pm/tables";

import type { IconName } from "../../generated/IconName";

/**
 * Tiptap's table, without its Tab and ⇧Tab: in a table Tab leaves the editor like anywhere outside a list or code block
 * (d0078: "everywhere else in the editor Tab moves focus out of it"), and arrow keys move between cells.
 */
const MarkdownTable = Table.extend({
  addKeyboardShortcuts() {
    const keys = { ...this.parent?.() };
    delete keys.Tab;
    delete keys["Shift-Tab"];
    return keys;
  },
});

/** The extension list's table line: Tiptap's TableKit, with the table above. */
export const Tables = Extension.create({
  name: "tables",
  addExtensions: () => [MarkdownTable, TableHeader, TableRow, TableCell],
});

/** The table around the caret, as its position in the document, or `null` outside a table. */
export function tableAt({ selection }: { selection: Selection }): number | null {
  const { $from } = selection;
  for (let depth = $from.depth; depth > 0; depth--) {
    if ($from.node(depth).type.name === "table") return $from.before(depth);
  }
  return null;
}

/** Whether the table around the caret has a header row, as Markdown's tables do. */
function hasHeaderRow(state: EditorState): boolean {
  const pos = tableAt(state);
  return pos !== null && state.doc.nodeAt(pos)?.firstChild?.firstChild?.type.name === "tableHeader";
}

/**
 * Keeps the header in the first row only: a row added above the header becomes the header and the old header a body
 * row; a deleted header row hands the header to the next row. Markdown has room for one header row, so the table
 * looks the way it will read back after a save.
 */
function keepOneHeaderRow(tr: Transaction): void {
  const pos = tableAt(tr);
  const table = pos === null ? null : tr.doc.nodeAt(pos);
  if (pos === null || !table) return;
  const { tableHeader, tableCell } = tr.doc.type.schema.nodes;
  if (!tableHeader || !tableCell) return;
  table.forEach((row, rowOffset, index) => {
    row.forEach((cell, cellOffset) => {
      const type = index === 0 ? tableHeader : tableCell;
      if (cell.type !== type) tr.setNodeMarkup(pos + 1 + rowOffset + 1 + cellOffset, type, cell.attrs);
    });
  });
}

type TableCommand =
  "addRowBefore" | "addRowAfter" | "addColumnBefore" | "addColumnAfter" | "deleteRow" | "deleteColumn";

/** Runs a table command and, if the table had a header row, keeps exactly one, in the same undo step. */
/**
 * Whether the selected rows (`"row"`) or columns can be deleted: not when they are all the table has. (Tiptap's
 * `can().deleteRow()` says yes even then, and the command then does nothing.)
 */
function canDelete(state: EditorState, what: "row" | "column"): boolean {
  if (!isInTable(state)) return false;
  const rect = selectedRect(state);
  return what === "row"
    ? !(rect.top === 0 && rect.bottom === rect.map.height)
    : !(rect.left === 0 && rect.right === rect.map.width);
}

function runKeepingHeader(editor: Editor, command: TableCommand): boolean {
  const header = hasHeaderRow(editor.state);
  const chain = editor.chain();
  return chain[command]()
    .command(({ tr }) => {
      if (header) keepOneHeaderRow(tr);
      return true;
    })
    .run();
}

/** One button of the table bar. */
export interface TableAction {
  /** A stable name, for tests. */
  id: string;
  /** The tooltip of an icon button, or the text of a text button. */
  label: string;
  /** Icon buttons show only the icon (the label is their tooltip); text buttons show the label. */
  icon?: IconName;
  /** Drawn in the danger colour (Delete table). */
  danger?: boolean;
  /** The bar draws a thin rule before this button. */
  separated?: boolean;
  run: (editor: Editor) => boolean;
  /** Whether the action can run at the caret: the last row or column can't be deleted on its own. */
  can: (editor: Editor) => boolean;
}

/** The table bar's buttons, in the order they're drawn (*f · Caret in a table*). */
export const TABLE_ACTIONS: readonly TableAction[] = [
  {
    id: "addRowAbove",
    label: "Add row above",
    icon: "between-horizontal-start",
    run: (editor) => runKeepingHeader(editor, "addRowBefore"),
    can: (editor) => editor.can().addRowBefore(),
  },
  {
    id: "addRowBelow",
    label: "Add row below",
    icon: "between-horizontal-end",
    run: (editor) => runKeepingHeader(editor, "addRowAfter"),
    can: (editor) => editor.can().addRowAfter(),
  },
  {
    id: "addColumnLeft",
    label: "Add column left",
    icon: "between-vertical-start",
    run: (editor) => runKeepingHeader(editor, "addColumnBefore"),
    can: (editor) => editor.can().addColumnBefore(),
  },
  {
    id: "addColumnRight",
    label: "Add column right",
    icon: "between-vertical-end",
    run: (editor) => runKeepingHeader(editor, "addColumnAfter"),
    can: (editor) => editor.can().addColumnAfter(),
  },
  {
    id: "deleteRow",
    label: "Delete row",
    separated: true,
    run: (editor) => runKeepingHeader(editor, "deleteRow"),
    can: (editor) => canDelete(editor.state, "row"),
  },
  {
    id: "deleteColumn",
    label: "Delete column",
    run: (editor) => runKeepingHeader(editor, "deleteColumn"),
    can: (editor) => canDelete(editor.state, "column"),
  },
  {
    id: "deleteTable",
    label: "Delete table",
    icon: "trash-2",
    danger: true,
    separated: true,
    run: (editor) => editor.commands.deleteTable(),
    can: (editor) => editor.can().deleteTable(),
  },
];

/** The gap between the bar and the table's top edge, and the least room kept from the window's edge, in px. */
export const BAR_GAP = 6;
export const BAR_EDGE = 8;

export interface BarRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/**
 * Where the bar goes for a table at `table` (window coordinates): above the table, left edges aligned; when the table's
 * top is too near the window's top, just inside the table's top edge instead, so it stays on screen. Always inside the
 * window.
 */
export function placeTableBar(
  table: BarRect,
  bar: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number } {
  const above = table.top - BAR_GAP - bar.height;
  const top = above >= BAR_EDGE ? above : Math.max(BAR_EDGE, table.top + BAR_GAP);
  const left = Math.max(BAR_EDGE, Math.min(table.left, viewport.width - BAR_EDGE - bar.width));
  return { left, top: Math.min(top, viewport.height - BAR_EDGE - bar.height) };
}
