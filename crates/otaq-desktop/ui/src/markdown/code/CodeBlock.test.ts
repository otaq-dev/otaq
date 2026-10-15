// @vitest-environment happy-dom
// The code block in the editor (T-0254): the picker's language is saved after the fence and round-trips, ``` on an
// empty last line leaves the block, and the drawn block has its label, the Diff tab's colours, and read mode's Copy.

import { Editor } from "@tiptap/core";
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, test, vi } from "vitest";

import { loadMarkdown, serializeMarkdown } from "../changes";
import { MarkdownEditor } from "../Editor";
import { markdownExtensions } from "../extensions";
import { leaveOnClosingFence } from "./CodeBlock";
import { setCodeLanguage } from "./languages";

function headless(markdown: string): Editor {
  const editor = new Editor({ element: null, extensions: markdownExtensions() });
  loadMarkdown(editor, markdown);
  return editor;
}

/** The position of the first code block. */
function codeBlockAt(editor: Editor): number {
  let at = -1;
  editor.state.doc.descendants((node, pos) => {
    if (at === -1 && node.type.name === "codeBlock") at = pos;
    return at === -1;
  });
  return at;
}

function roundTrip(markdown: string): string {
  return serializeMarkdown(headless(markdown));
}

describe("the picker changes the fence's language", () => {
  test.each([
    ["rust", "```\nfn main() {}\n```", "```rust\nfn main() {}\n```"],
    ["Plain text", "```ts\nconst a = 1;\n```", "```\nconst a = 1;\n```"],
    ["tsx", "Intro\n\n```ts\n<A />\n```", "Intro\n\n```tsx\n<A />\n```"],
  ])("to %s, and the text round-trips", (label, before, after) => {
    const editor = headless(before);
    const id = label === "Plain text" ? null : label;
    expect(setCodeLanguage(editor, codeBlockAt(editor), id)).toBe(true);
    const saved = serializeMarkdown(editor);
    expect(saved).toBe(after);
    expect(roundTrip(saved)).toBe(saved);
    editor.destroy();
  });

  test("only a code block takes a language", () => {
    const editor = headless("text");
    expect(setCodeLanguage(editor, 0, "ts")).toBe(false);
    editor.destroy();
  });
});

describe("``` on an empty last line leaves the block", () => {
  function typeBacktick(editor: Editor): boolean {
    const { from, to } = editor.state.selection;
    return leaveOnClosingFence(editor, from, to, "`");
  }

  test("the line goes and the caret is in a paragraph below", () => {
    const editor = headless("```ts\nconst a = 1;\n```");
    editor.commands.focus("end");
    editor.commands.insertContent("\n``");
    expect(typeBacktick(editor)).toBe(true);
    expect(editor.state.selection.$from.parent.type.name).toBe("paragraph");
    expect(serializeMarkdown(editor)).toBe("```ts\nconst a = 1;\n```");
    expect(editor.state.doc.lastChild?.type.name).toBe("paragraph");
    editor.destroy();
  });

  test.each([
    ["not at the end of the block", "```\nab\n```", 2],
    ["other text on the line", "```\nx ``\n```", null],
  ])("nothing happens: %s", (_name, markdown, caret) => {
    const editor = headless(markdown);
    editor.commands.focus("end");
    if (caret !== null) editor.commands.setTextSelection(caret);
    expect(typeBacktick(editor)).toBe(false);
    editor.destroy();
  });

  test("outside a code block, a backtick is a backtick", () => {
    const editor = headless("``");
    editor.commands.focus("end");
    expect(typeBacktick(editor)).toBe(false);
    editor.destroy();
  });
});

describe("the drawn block", () => {
  let root: Root | null = null;
  let host: HTMLElement | null = null;

  beforeAll(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterEach(() => {
    act(() => root?.unmount());
    host?.remove();
    root = null;
    host = null;
  });

  function render(value: string, editable: boolean, onChange?: (markdown: string) => void): HTMLElement {
    host = document.body.appendChild(document.createElement("div"));
    root = createRoot(host);
    act(() => {
      root?.render(
        createElement(MarkdownEditor, {
          value,
          editable,
          label: "What and why",
          ...(onChange === undefined ? {} : { onChange }),
        }),
      );
    });
    return host;
  }

  const CODE = '```ts\nconst results = await places.autocomplete(query, { country: "TR" });\n```';

  test("the code is coloured with the diff's tokens, and labelled", () => {
    const page = render(CODE, true);
    const block = page.querySelector(".md-code");
    expect(block?.querySelector("pre code")?.textContent).toContain("places.autocomplete");
    expect([...(block?.querySelectorAll(".tok-keyword") ?? [])].map((e) => e.textContent)).toEqual([
      "const",
      "await",
    ]);
    expect(block?.querySelector(".tok-string")?.textContent).toBe('"TR"');
    expect(block?.querySelector(".md-code-language")?.textContent).toBe("ts");
    expect(block?.querySelector(".md-code-tools")?.getAttribute("contenteditable")).toBe("false");
  });

  test("read mode's Copy copies just that block's code", () => {
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    const page = render(`Intro\n\n${CODE}\n\nAfter`, false);
    act(() => {
      page.querySelector<HTMLButtonElement>(".md-code-copy")?.click();
    });
    expect(writeText).toHaveBeenCalledWith(
      'const results = await places.autocomplete(query, { country: "TR" });',
    );
    writeText.mockRestore();
  });

  test("the picker: filter, ↵, and the new language is saved after the fence", () => {
    const onChange = vi.fn<(markdown: string) => void>();
    const page = render(CODE, true, onChange);
    act(() => {
      page.querySelector<HTMLButtonElement>(".md-code-language")?.click();
    });
    const filter = page.querySelector<HTMLInputElement>(".md-code-picker input");
    if (!filter) throw new Error("no picker");
    expect(page.querySelector('.md-code-option[aria-selected="true"]')?.textContent).toContain("ts");
    act(() => {
      // React tracks the value it last set; going through the prototype's setter makes the change visible to it.
      Reflect.set(HTMLInputElement.prototype, "value", "ru", filter);
      filter.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect([...page.querySelectorAll(".md-code-option")].map((e) => e.textContent)).toEqual(["rust"]);
    act(() => {
      filter.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    });
    expect(page.querySelector(".md-code-picker")).toBeNull();
    expect(onChange).toHaveBeenLastCalledWith(CODE.replace("```ts", "```rust"));
  });

  test("an untouched block is never re-saved", () => {
    const onChange = vi.fn<(markdown: string) => void>();
    render("````md\n```ts\nx\n```\n````", true, onChange);
    expect(onChange).not.toHaveBeenCalled();
  });
});
