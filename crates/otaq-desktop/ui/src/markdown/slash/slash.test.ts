// @vitest-environment happy-dom
// `/` for blocks (T-0465, SHT-03): when the list opens, what it offers for a query, and what a pick turns the paragraph
// into, on a headless editor with the description's own extensions.
import { Editor } from "@tiptap/core";
import { afterEach, describe, expect, it } from "vitest";

import { loadMarkdown, serializeMarkdown } from "../changes";
import { markdownExtensions } from "../extensions";
import { applyItem, filterItems, SLASH_ITEMS, slashAt, stepItem } from "./slash";

let editor: Editor | null = null;

afterEach(() => {
  editor?.destroy();
  editor = null;
});

/** An editor holding `markdown`, the caret at the end of the block holding `text`. */
function at(markdown: string, text: string): Editor {
  const e = new Editor({ element: null, extensions: markdownExtensions() });
  editor = e;
  loadMarkdown(e, markdown);
  let pos = -1;
  e.state.doc.descendants((node, p) => {
    if (pos < 0 && node.isText && node.text?.includes(text)) pos = p + node.text.length;
  });
  e.commands.setTextSelection(pos);
  return e;
}

describe("the / list", () => {
  it("opens for a / at the start of a plain paragraph, with what follows it as the query", () => {
    expect(slashAt(at("Users land on /login.\n\n/hea", "/hea").state)).toMatchObject({ query: "hea" });
    expect(slashAt(at("/", "/").state)).toMatchObject({ query: "" });
  });

  it("stays shut mid-sentence, in a list item, a quote or a code block", () => {
    expect(slashAt(at("Users land on /login", "/login").state)).toBeNull();
    expect(slashAt(at("- /bul", "/bul").state)).toBeNull();
    expect(slashAt(at("> /quo", "/quo").state)).toBeNull();
    expect(slashAt(at("```\n/code\n```", "/code").state)).toBeNull();
  });

  it("offers text, three headings and the toolbar's blocks, filtered by the start of a word", () => {
    expect(SLASH_ITEMS.map((i) => i.label)).toEqual([
      "Text",
      "Heading 1",
      "Heading 2",
      "Heading 3",
      "Bullet list",
      "Numbered list",
      "Task list",
      "Quote",
      "Code block",
      "Table",
      "Horizontal rule",
    ]);
    expect(filterItems("hea").map((i) => i.label)).toEqual(["Heading 1", "Heading 2", "Heading 3"]);
    expect(filterItems("LIST").map((i) => i.label)).toEqual(["Bullet list", "Numbered list", "Task list"]);
    expect(filterItems("heading 2").map((i) => i.label)).toEqual(["Heading 2"]);
    expect(filterItems("zz")).toEqual([]);
  });

  it("↑ ↓ go round the ends", () => {
    expect(stepItem(0, -1, 3)).toBe(2);
    expect(stepItem(2, 1, 3)).toBe(0);
    expect(stepItem(0, 1, 0)).toBe(0);
  });

  it("a pick drops the / and its query and turns the paragraph into the block", () => {
    const e = at("Users land on /login.\n\n/task", "/task");
    const slash = slashAt(e.state);
    const item = filterItems("task")[0];
    if (!slash || !item) throw new Error("no list");
    applyItem(e, slash, item);
    e.commands.insertContent("no redirect loop");
    expect(serializeMarkdown(e)).toBe("Users land on /login.\n\n- [ ] no redirect loop");

    const h = at("/he", "/he");
    const heading = filterItems("heading 2")[0];
    const open = slashAt(h.state);
    if (!open || !heading) throw new Error("no list");
    applyItem(h, open, heading);
    h.commands.insertContent("Expected");
    expect(serializeMarkdown(h)).toBe("## Expected");
  });
});
