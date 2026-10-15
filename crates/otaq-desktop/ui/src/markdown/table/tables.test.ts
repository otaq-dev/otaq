// The table bar's actions (T-0253) on a headless editor: each one saves a valid Markdown table (read back, it is the
// table that was drawn), one header row stays one, the last row or column can't be deleted on its own, and the bar's
// place next to the table.

import { Editor, type JSONContent } from "@tiptap/core";
import { afterEach, describe, expect, test } from "vitest";

import { serializeMarkdown } from "../changes";
import { markdownExtensions } from "../extensions";
import { BAR_EDGE, BAR_GAP, placeTableBar, TABLE_ACTIONS, tableAt } from "./tables";

const TABLE = `| Field | Source |
|---|---|
| Street | Places API |
| Postcode | Places API, editable |
`;

const DOC = `Where each address field comes from:

${TABLE}
After the table.
`;

let editor: Editor | null = null;

afterEach(() => {
  editor?.destroy();
  editor = null;
});

/** A headless editor holding `markdown`, with the caret at the start of the cell whose text is `cell`. */
function editorAt(markdown: string, cell: string): Editor {
  editor = new Editor({
    element: null,
    extensions: markdownExtensions(),
    content: markdown,
    contentType: "markdown",
  });
  let at = -1;
  editor.state.doc.descendants((node, pos) => {
    if (at < 0 && node.isText && node.text === cell) at = pos;
  });
  if (at < 0) throw new Error(`no cell ${cell}`);
  editor.commands.setTextSelection(at);
  return editor;
}

function run(target: Editor, id: string): boolean {
  const action = TABLE_ACTIONS.find((a) => a.id === id);
  if (!action) throw new Error(`no action ${id}`);
  return action.run(target);
}

function can(target: Editor, id: string): boolean {
  return TABLE_ACTIONS.find((a) => a.id === id)?.can(target) ?? false;
}

/** The tables a Markdown text reads back as: each row's cell texts, `#` before a header cell. */
function readTables(target: Editor, markdown: string): string[][][] {
  const text = (node: JSONContent): string =>
    node.text ?? (node.content ?? []).map((child) => text(child)).join("");
  const doc = target.markdown?.parse(markdown);
  return (doc?.content ?? [])
    .filter((node) => node.type === "table")
    .map((table) =>
      (table.content ?? []).map((row) =>
        (row.content ?? []).map((c) => (c.type === "tableHeader" ? `#${text(c)}` : text(c))),
      ),
    );
}

/** The editor's tables as drawn, in the same shape. */
function drawnTables(target: Editor): string[][][] {
  const tables: string[][][] = [];
  target.state.doc.descendants((node) => {
    if (node.type.name !== "table") return true;
    const rows: string[][] = [];
    node.forEach((row) => {
      const cells: string[] = [];
      row.forEach((c) => cells.push(c.type.name === "tableHeader" ? `#${c.textContent}` : c.textContent));
      rows.push(cells);
    });
    tables.push(rows);
    return false;
  });
  return tables;
}

/** Runs an action and returns the saved Markdown, checking that it reads back as the table that was drawn. */
function saveAfter(target: Editor, id: string): string {
  expect(can(target, id)).toBe(true);
  expect(run(target, id)).toBe(true);
  const saved = serializeMarkdown(target);
  expect(readTables(target, saved)).toEqual(drawnTables(target));
  return saved;
}

describe("the table bar's actions save valid Markdown tables", () => {
  test("loading and moving the caret into a table changes nothing", () => {
    const e = editorAt(DOC, "Street");
    expect(serializeMarkdown(e)).toContain("| Street   | Places API           |");
    expect(readTables(e, serializeMarkdown(e))).toEqual([
      [
        ["#Field", "#Source"],
        ["Street", "Places API"],
        ["Postcode", "Places API, editable"],
      ],
    ]);
  });

  test("add row above", () => {
    expect(saveAfter(editorAt(DOC, "Street"), "addRowAbove")).toContain(
      [
        "| Field    | Source               |",
        "| -------- | -------------------- |",
        "|          |                      |",
        "| Street   | Places API           |",
        "| Postcode | Places API, editable |",
      ].join("\n"),
    );
  });

  test("add row below", () => {
    expect(saveAfter(editorAt(DOC, "Street"), "addRowBelow")).toContain(
      [
        "| Street   | Places API           |",
        "|          |                      |",
        "| Postcode | Places API, editable |",
      ].join("\n"),
    );
  });

  test("add column left", () => {
    expect(saveAfter(editorAt(DOC, "Street"), "addColumnLeft")).toContain(
      [
        "|     | Field    | Source               |",
        "| --- | -------- | -------------------- |",
        "|     | Street   | Places API           |",
        "|     | Postcode | Places API, editable |",
      ].join("\n"),
    );
  });

  test("add column right", () => {
    expect(saveAfter(editorAt(DOC, "Street"), "addColumnRight")).toContain(
      [
        "| Field    |     | Source               |",
        "| -------- | --- | -------------------- |",
        "| Street   |     | Places API           |",
        "| Postcode |     | Places API, editable |",
      ].join("\n"),
    );
  });

  test("delete row", () => {
    const saved = saveAfter(editorAt(DOC, "Street"), "deleteRow");
    expect(saved).toContain(
      [
        "| Field    | Source               |",
        "| -------- | -------------------- |",
        "| Postcode | Places API, editable |",
      ].join("\n"),
    );
    expect(saved).not.toContain("Street");
  });

  test("delete column", () => {
    expect(saveAfter(editorAt(DOC, "Street"), "deleteColumn")).toContain(
      [
        "| Source               |",
        "| -------------------- |",
        "| Places API           |",
        "| Places API, editable |",
      ].join("\n"),
    );
  });

  test("delete table", () => {
    const e = editorAt(DOC, "Street");
    expect(run(e, "deleteTable")).toBe(true);
    expect(serializeMarkdown(e)).toBe("Where each address field comes from:\n\nAfter the table.");
    expect(tableAt(e.state)).toBeNull();
  });
});

describe("one header row, as Markdown stores it", () => {
  test("a row added above the header becomes the header; the old header becomes a body row", () => {
    const e = editorAt(DOC, "Field");
    const saved = saveAfter(e, "addRowAbove");
    expect(readTables(e, saved)).toEqual([
      [
        ["#", "#"],
        ["Field", "Source"],
        ["Street", "Places API"],
        ["Postcode", "Places API, editable"],
      ],
    ]);
  });

  test("a row added below the header is a body row", () => {
    const e = editorAt(DOC, "Field");
    expect(readTables(e, saveAfter(e, "addRowBelow"))[0]?.[1]).toEqual(["", ""]);
  });

  test("deleting the header row makes the next row the header", () => {
    const e = editorAt(DOC, "Field");
    expect(readTables(e, saveAfter(e, "deleteRow"))).toEqual([
      [
        ["#Street", "#Places API"],
        ["Postcode", "Places API, editable"],
      ],
    ]);
  });

  test("a column added from the header has a header cell", () => {
    const e = editorAt(DOC, "Field");
    expect(readTables(e, saveAfter(e, "addColumnRight"))[0]?.[0]).toEqual(["#Field", "#", "#Source"]);
  });
});

describe("what the bar offers", () => {
  test("the caret outside a table: no table", () => {
    const e = editorAt(DOC, "After the table.");
    expect(tableAt(e.state)).toBeNull();
  });

  test("a header-only table can't lose its last row; a one-column table can't lose its last column", () => {
    const rowOnly = editorAt("| A | B |\n|---|---|\n", "A");
    expect(can(rowOnly, "deleteRow")).toBe(false);
    expect(can(rowOnly, "deleteColumn")).toBe(true);
    expect(can(rowOnly, "deleteTable")).toBe(true);
    rowOnly.destroy();
    const columnOnly = editorAt("| A |\n|---|\n| 1 |\n", "1");
    expect(can(columnOnly, "deleteColumn")).toBe(false);
    expect(can(columnOnly, "deleteRow")).toBe(true);
  });

  test("the buttons, in the design's order", () => {
    expect(TABLE_ACTIONS.map((a) => a.label)).toEqual([
      "Add row above",
      "Add row below",
      "Add column left",
      "Add column right",
      "Delete row",
      "Delete column",
      "Delete table",
    ]);
    expect(TABLE_ACTIONS.filter((a) => a.danger === true).map((a) => a.id)).toEqual(["deleteTable"]);
  });
});

describe("where the bar goes", () => {
  const bar = { width: 410, height: 32 };
  const viewport = { width: 1440, height: 900 };

  test("above the table, left edges aligned", () => {
    expect(placeTableBar({ left: 300, top: 400, width: 350, height: 84 }, bar, viewport)).toEqual({
      left: 300,
      top: 400 - BAR_GAP - 32,
    });
  });

  test("inside the table's top edge when the table is at the window's top", () => {
    expect(placeTableBar({ left: 300, top: 20, width: 350, height: 84 }, bar, viewport).top).toBe(
      20 + BAR_GAP,
    );
    expect(placeTableBar({ left: 300, top: -200, width: 350, height: 400 }, bar, viewport).top).toBe(
      BAR_EDGE,
    );
  });

  test("always inside the window", () => {
    expect(placeTableBar({ left: 1300, top: 400, width: 350, height: 84 }, bar, viewport).left).toBe(
      1440 - BAR_EDGE - 410,
    );
  });
});
