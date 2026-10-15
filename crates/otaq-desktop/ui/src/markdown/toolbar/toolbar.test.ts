// @vitest-environment happy-dom
import { Editor } from "@tiptap/core";
import { afterEach, describe, expect, it } from "vitest";

import { loadMarkdown, serializeMarkdown } from "../changes";
import { markdownExtensions } from "../extensions";
import { TOOLBAR_BUTTONS } from "./buttons";
import { openLink } from "./link";
import { overflowed, type OverflowItem } from "./overflow";
import { shortcutLabel } from "./shortcuts";

const editors: Editor[] = [];

function editorWith(markdown: string, mounted = false): Editor {
  const element = mounted ? document.createElement("div") : null;
  const editor = new Editor({ element, extensions: markdownExtensions() });
  loadMarkdown(editor, markdown);
  editors.push(editor);
  return editor;
}

afterEach(() => {
  for (const e of editors.splice(0)) e.destroy();
});

const button = (id: string) => {
  const found = TOOLBAR_BUTTONS.find((b) => b.id === id);
  if (!found) throw new Error(`no button ${id}`);
  return found;
};

describe("the buttons", () => {
  it("are the toolbar of the design, in its order", () => {
    expect(TOOLBAR_BUTTONS.map((b) => b.label)).toEqual([
      "Bold",
      "Italic",
      "Strikethrough",
      "Inline code",
      "Link",
      "Bullet list",
      "Numbered list",
      "Task list",
      "Quote",
      "Code block",
      "Table",
      "Horizontal rule",
    ]);
  });

  it.each([
    ["bold", "**hello**"],
    ["italic", "*hello*"],
    ["strike", "~~hello~~"],
    ["code", "`hello`"],
    ["bulletList", "- hello"],
    ["orderedList", "1. hello"],
    ["taskList", "- [ ] hello"],
    ["blockquote", "> hello"],
    ["codeBlock", "```\nhello\n```"],
  ])("%s applies its format and lights up", (id, saved) => {
    const editor = editorWith("hello");
    editor.commands.selectAll();
    const b = button(id);
    expect(b.isActive?.(editor)).toBe(false);
    b.run(editor);
    expect(serializeMarkdown(editor)).toBe(saved);
    expect(b.isActive?.(editor)).toBe(true);
  });

  it("lights up Task list, not Bullet list, with the caret in a task item", () => {
    const editor = editorWith("- [ ] one\n- [x] two");
    editor.commands.setTextSelection(4);
    expect(button("taskList").isActive?.(editor)).toBe(true);
    expect(button("bulletList").isActive?.(editor)).toBe(false);
  });

  it("Table inserts a 2 × 2 table, and lights up inside it", () => {
    const editor = editorWith("hello");
    editor.commands.setTextSelection(6);
    button("table").run(editor);
    expect(button("table").isActive?.(editor)).toBe(true);
    const table = serializeMarkdown(editor)
      .split("\n")
      .filter((line) => line.startsWith("|"));
    expect(table).toHaveLength(3); // the header row, its rule and one body row
    expect(table[0]?.split("|").length).toBe(4); // two columns
  });

  it("Horizontal rule adds a rule", () => {
    const editor = editorWith("hello");
    editor.commands.setTextSelection(6);
    button("horizontalRule").run(editor);
    expect(serializeMarkdown(editor)).toContain("---");
  });

  it("Link links the selection (to https:// until T-0252's popover) and removes a link the caret is in", () => {
    const editor = editorWith("see here");
    editor.commands.setTextSelection({ from: 5, to: 9 });
    openLink(editor);
    expect(editor.isActive("link")).toBe(true);
    expect(serializeMarkdown(editor)).toBe("see [here](https://)");
    editor.commands.setTextSelection(7);
    openLink(editor);
    expect(serializeMarkdown(editor)).toBe("see here");
  });

  it("Link does nothing without a selection", () => {
    const editor = editorWith("hello");
    editor.commands.setTextSelection(3);
    openLink(editor);
    expect(serializeMarkdown(editor)).toBe("hello");
  });

  it("the overflow order is Strikethrough, Quote, Table, Horizontal rule", () => {
    const order = TOOLBAR_BUTTONS.filter((b) => b.overflow !== undefined)
      .toSorted((a, b) => (a.overflow ?? 0) - (b.overflow ?? 0))
      .map((b) => b.label);
    expect(order).toEqual(["Strikethrough", "Quote", "Table", "Horizontal rule"]);
  });
});

describe("shortcuts", () => {
  it("print ⌘ on macOS and Ctrl / Alt on Windows and Linux", () => {
    expect(shortcutLabel({ key: "B" }, true)).toBe("⌘B");
    expect(shortcutLabel({ key: "X", shift: true }, true)).toBe("⌘⇧X");
    expect(shortcutLabel({ key: "C", alt: true }, true)).toBe("⌘⌥C");
    expect(shortcutLabel({ key: "B" }, false)).toBe("Ctrl+B");
    expect(shortcutLabel({ key: "8", shift: true }, false)).toBe("Ctrl+Shift+8");
    expect(shortcutLabel({ key: "C", alt: true }, false)).toBe("Ctrl+Alt+C");
  });

  it("are the ticket's: ⌘B ⌘I ⌘⇧X ⌘E ⌘K ⌘⇧8 ⌘⇧7 ⌘⇧9 ⌘⇧B", () => {
    expect(TOOLBAR_BUTTONS.flatMap((b) => (b.keys ? [shortcutLabel(b.keys, true)] : []))).toEqual([
      "⌘B",
      "⌘I",
      "⌘⇧X",
      "⌘E",
      "⌘K",
      "⌘⇧8",
      "⌘⇧7",
      "⌘⇧9",
      "⌘⇧B",
    ]);
  });

  it("⌘⇧X strikes through, and ⌘⌥1–6 make no heading (d0058)", () => {
    const editor = editorWith("hello", true);
    editor.commands.selectAll();
    // ProseMirror's Mod: ⌘ where the platform says Mac, else Ctrl.
    const mac = /Mac|iP(hone|[oa]d)/.test(navigator.platform);
    const press = (key: string, mods: KeyboardEventInit) =>
      editor.view.someProp("handleKeyDown", (f) =>
        f(editor.view, new KeyboardEvent("keydown", { key, ...mods, metaKey: mac, ctrlKey: !mac })),
      );
    expect(editor.isActive("strike")).toBe(false);
    press("x", { shiftKey: true });
    expect(editor.isActive("strike")).toBe(true);
    for (const n of [1, 2, 3, 4, 5, 6]) press(String(n), { altKey: true });
    expect(editor.getHTML()).not.toMatch(/<h\d/);
  });
});

describe("overflow", () => {
  // The design's widths: Heading 50, buttons 24, separators 7, each plus a 1 px gap.
  const items: OverflowItem[] = [
    { id: "heading", width: 51 },
    { id: "sep-inline", width: 8 },
    ...TOOLBAR_BUTTONS.map((b) => ({ id: b.id, width: 25, overflow: b.overflow })),
    { id: "sep-link", width: 8 },
    { id: "sep-lists", width: 8 },
    { id: "sep-blocks", width: 8 },
  ];
  const all = items.reduce((sum, i) => sum + i.width, 0);

  it("moves nothing, and shows no …, while everything fits", () => {
    expect(overflowed(items, all, 25)).toEqual([]);
    expect(overflowed(items, all + 100, 25)).toEqual([]);
  });

  it("moves Strikethrough, Quote, Table and Horizontal rule, in that order, until the rest and … fit", () => {
    expect(overflowed(items, all - 1, 25)).toEqual(["strike", "blockquote"]);
    expect(overflowed(items, all - 25, 25)).toEqual(["strike", "blockquote"]);
    expect(overflowed(items, all - 26, 25)).toEqual(["strike", "blockquote", "table"]);
    expect(overflowed(items, all - 50, 25)).toEqual(["strike", "blockquote", "table"]);
    expect(overflowed(items, all - 51, 25)).toEqual(["strike", "blockquote", "table", "horizontalRule"]);
  });

  it("moves only those four, however narrow", () => {
    expect(overflowed(items, 0, 25)).toEqual(["strike", "blockquote", "table", "horizontalRule"]);
  });
});
