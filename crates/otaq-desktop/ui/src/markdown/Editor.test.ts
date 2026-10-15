// @vitest-environment happy-dom
// `<MarkdownEditor>` in a page (T-0249): the example renders in both modes, edits come back as Markdown, an untouched
// description reports nothing, read-only task boxes don't toggle, a new value loads without counting as an edit, and
// `onKeyDown` sees keys first.

import type { Editor } from "@tiptap/core";
import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, test, vi } from "vitest";

import { type MarkdownEditorOptions, MarkdownSurface, useMarkdownEditor } from "./Editor";
import samples from "./roundtrip/samples.json";

const EXAMPLE = samples.find((s) => s.kind === "example")?.markdown ?? "";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement | null = null;
const shown: { editor?: Editor } = {};

/** The hook and the surface, as `<MarkdownEditor>` puts them together, handing the instance to the test. */
function Harness({ onEditor, ...options }: MarkdownEditorOptions & { onEditor: (editor: Editor) => void }) {
  const editor = useMarkdownEditor(options);
  useEffect(() => {
    onEditor(editor);
  });
  return createElement(MarkdownSurface, { editor });
}

function render(options: MarkdownEditorOptions) {
  host ??= document.body.appendChild(document.createElement("div"));
  root ??= createRoot(host);
  const page = host;
  act(() => {
    root?.render(
      createElement(Harness, {
        ...options,
        onEditor: (editor) => {
          shown.editor = editor;
        },
      }),
    );
  });
  if (!shown.editor) throw new Error("not rendered");
  return { editor: shown.editor, page };
}

function text(page: HTMLElement): HTMLElement {
  const el = page.querySelector<HTMLElement>(".ProseMirror");
  if (!el) throw new Error("no text");
  return el;
}

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  delete shown.editor;
});

describe("the example renders in both modes", () => {
  test.each([true, false])("editable: %s", (editable) => {
    const { page } = render({ value: EXAMPLE, editable, label: "What and why" });
    const el = text(page);
    expect(el.getAttribute("contenteditable")).toBe(String(editable));
    expect(el.getAttribute("aria-label")).toBe("What and why");
    expect(el.getAttribute("role")).toBe("textbox");
    expect(el.getAttribute("aria-readonly")).toBe(String(!editable));
    expect([...el.querySelectorAll("h2")].map((h) => h.textContent)).toEqual(["Why", "What", "Done when"]);
    expect(el.querySelector("strong")?.textContent).toBe("12%");
    expect(el.querySelector("a")?.getAttribute("href")).toBe("https://example.com/report");
    expect(el.querySelector("em")?.textContent).toBe("Street");
    const boxes = [...el.querySelectorAll<HTMLInputElement>('ul[data-type="taskList"] > li > label > input')];
    expect(boxes.map((b) => b.checked)).toEqual([true, false, false]);
    expect(el.querySelector(":not(pre) > code")?.textContent).toBe("AddressForm");
    expect(el.querySelectorAll("th")).toHaveLength(2);
    expect(el.querySelectorAll("td")).toHaveLength(4);
    expect(el.querySelector("pre code")?.textContent).toContain("places.autocomplete");
  });
});

test("an edit comes back as Markdown", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  const { editor } = render({ value: "## Why\nSome text", onChange, label: "What and why" });
  act(() => {
    editor.chain().focus("end").insertContent(" and more").run();
  });
  expect(onChange).toHaveBeenCalledTimes(1);
  expect(onChange).toHaveBeenLastCalledWith("## Why\n\nSome text and more");
});

test("an untouched description reports nothing, so its original string is what gets saved", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  const { editor, page } = render({ value: EXAMPLE, onChange, label: "What and why" });
  act(() => {
    editor.commands.focus("end");
    editor.commands.setTextSelection(1);
    text(page).dispatchEvent(new MouseEvent("mousedown", { bubbles: true }));
  });
  expect(onChange).not.toHaveBeenCalled();
});

describe("task boxes", () => {
  test("read-only boxes don't toggle", () => {
    const onChange = vi.fn<(markdown: string) => void>();
    const { editor, page } = render({ value: EXAMPLE, onChange, editable: false, label: "What and why" });
    const box = text(page).querySelectorAll<HTMLInputElement>(
      'ul[data-type="taskList"] > li > label > input',
    )[1];
    if (!box) throw new Error("no box");
    act(() => {
      box.click();
    });
    expect(box.checked).toBe(false);
    expect(editor.getMarkdown()).toContain("- [ ] Arrow keys move the highlight");
    expect(onChange).not.toHaveBeenCalled();
  });

  test("editable boxes toggle and report the change", () => {
    const onChange = vi.fn<(markdown: string) => void>();
    const { page } = render({ value: "- [ ] open", onChange, label: "What and why" });
    const box = text(page).querySelector<HTMLInputElement>('ul[data-type="taskList"] > li > label > input');
    act(() => {
      box?.click();
    });
    expect(onChange).toHaveBeenLastCalledWith("- [x] open");
  });
});

test("a new value loads in place, without an edit or an undo step", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  render({ value: "first", onChange, label: "What and why" });
  const { editor, page } = render({ value: "* second", onChange, label: "What and why" });
  expect(text(page).textContent).toBe("second");
  expect(onChange).not.toHaveBeenCalled();
  act(() => {
    editor.commands.undo();
  });
  expect(text(page).textContent).toBe("second");
});

test("a value the editor reported is not loaded again", () => {
  let stored = "one";
  const onChange = (markdown: string) => {
    stored = markdown;
  };
  const { editor } = render({ value: stored, onChange, label: "What and why" });
  act(() => {
    editor.chain().focus("end").insertContent(" two").run();
  });
  const before = editor.state.doc;
  render({ value: stored, onChange, label: "What and why" });
  expect(stored).toBe("one two");
  expect(editor.state.doc).toBe(before);
  act(() => {
    editor.commands.undo();
  });
  expect(stored).toBe("one");
});

test("onKeyDown sees a key first, and a handled key is left alone", () => {
  const seen: string[] = [];
  const { editor, page } = render({
    value: "line",
    label: "What and why",
    onKeyDown: (event) => {
      seen.push(event.key);
      return event.key === "Enter";
    },
  });
  act(() => {
    editor.commands.focus("end");
    text(page).dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
  });
  expect(seen).toEqual(["Enter"]);
  expect(editor.state.doc.childCount).toBe(1);
});

test("editable can be switched off and on", () => {
  render({ value: "x", label: "What and why" });
  const { editor, page } = render({ value: "x", editable: false, label: "What and why" });
  expect(editor.isEditable).toBe(false);
  expect(text(page).getAttribute("contenteditable")).toBe("false");
  render({ value: "x", editable: true, label: "What and why" });
  expect(editor.isEditable).toBe(true);
  expect(text(page).getAttribute("contenteditable")).toBe("true");
});
