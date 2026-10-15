// @vitest-environment happy-dom
// The table bar in a page (T-0253): it shows only in edit mode while the caret is in a table, its buttons save through
// `onChange` as one undo step, moving the caret reports nothing, and Tab is left to the page (d0078).

import type { Editor } from "@tiptap/core";
import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, expect, test, vi } from "vitest";

import { type MarkdownEditorOptions, MarkdownSurface, useMarkdownEditor } from "../Editor";

const DOC = `Where each address field comes from:

| Field | Source |
|---|---|
| Street | Places API |
`;

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement | null = null;
const shown: { editor?: Editor } = {};

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

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  delete shown.editor;
});

/** Puts the caret at the start of the text `cell`, with the editor focused. */
function caretAt(editor: Editor, cell: string) {
  let at = -1;
  editor.state.doc.descendants((node, pos) => {
    if (at < 0 && node.isText && node.text === cell) at = pos;
  });
  act(() => {
    // Tiptap's focus command waits a frame; the view's own focus is immediate.
    editor.view.focus();
    editor.commands.setTextSelection(at);
  });
}

const bar = (page: HTMLElement) => page.querySelector<HTMLElement>(".md-table-bar");

function button(page: HTMLElement, name: string): HTMLButtonElement {
  const found = [...page.querySelectorAll<HTMLButtonElement>(".md-table-bar button")].find(
    (b) => (b.getAttribute("aria-label") ?? b.textContent) === name,
  );
  if (!found) throw new Error(`no button ${name}`);
  return found;
}

test("the bar shows while the caret is in a table, and only then", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  const { editor, page } = render({ value: DOC, onChange, label: "What and why" });
  expect(bar(page)).toBeNull();
  caretAt(editor, "Where each address field comes from:");
  expect(bar(page)).toBeNull();
  caretAt(editor, "Street");
  expect(bar(page)?.getAttribute("role")).toBe("toolbar");
  expect(
    [...page.querySelectorAll(".md-table-bar button")].map(
      (b) => b.getAttribute("aria-label") ?? b.textContent,
    ),
  ).toEqual([
    "Add row above",
    "Add row below",
    "Add column left",
    "Add column right",
    "Delete row",
    "Delete column",
    "Delete table",
  ]);
  expect(button(page, "Delete table").dataset.danger).toBe("true");
  expect(button(page, "Add row above").tabIndex).toBe(-1);
  // Moving the caret is not an edit.
  expect(onChange).not.toHaveBeenCalled();
  act(() => {
    editor.view.dom.blur();
  });
  expect(bar(page)).toBeNull();
});

test("no bar in read mode", () => {
  const { editor, page } = render({ value: DOC, editable: false, label: "What and why" });
  caretAt(editor, "Street");
  expect(bar(page)).toBeNull();
});

test("a press keeps the caret, saves the table and is one undo step", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  const { editor, page } = render({ value: DOC, onChange, label: "What and why" });
  caretAt(editor, "Street");
  const press = button(page, "Add row below");
  const down = new MouseEvent("mousedown", { bubbles: true, cancelable: true });
  act(() => {
    press.dispatchEvent(down);
    press.click();
  });
  expect(down.defaultPrevented).toBe(true);
  expect(onChange).toHaveBeenLastCalledWith(
    // Tiptap re-pads the table and writes extra blank lines around it (S2 in tickets/m15-roundtrip.md, accepted in Q48).
    "Where each address field comes from:\n\n\n| Field  | Source     |\n| ------ | ---------- |\n| Street | Places API |\n|        |            |\n",
  );
  act(() => {
    editor.commands.undo();
  });
  expect(onChange).toHaveBeenLastCalledWith(DOC);
});

test("the last row can't be deleted on its own", () => {
  const { editor, page } = render({ value: "| A | B |\n|---|---|\n", label: "What and why" });
  caretAt(editor, "A");
  expect(button(page, "Delete row").disabled).toBe(true);
  expect(button(page, "Delete column").disabled).toBe(false);
});

test("Delete table removes it, and the bar goes with it", () => {
  const onChange = vi.fn<(markdown: string) => void>();
  const { editor, page } = render({ value: DOC, onChange, label: "What and why" });
  caretAt(editor, "Street");
  act(() => {
    button(page, "Delete table").click();
  });
  expect(onChange).toHaveBeenLastCalledWith("Where each address field comes from:");
  expect(bar(page)).toBeNull();
});

test("Tab in a table leaves the editor (T-0259): it doesn't move to the next cell or add a row", () => {
  const { editor } = render({ value: DOC, label: "What and why" });
  caretAt(editor, "Street");
  const before = editor.state;
  const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
  act(() => {
    editor.view.dom.dispatchEvent(tab);
  });
  // Taken by the editor's own key routing, which moves focus out of the editor (markdown/keys.ts).
  expect(tab.defaultPrevented).toBe(true);
  expect(editor.state.doc).toBe(before.doc);
  expect(editor.state.selection.eq(before.selection)).toBe(true);
});
