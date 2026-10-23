// @vitest-environment happy-dom
// Keys and focus in a page (T-0259, d0078 D3 / D4): Tab nests a list item and indents a code line; Tab out of a
// paragraph moves to the next field and ⇧Tab to the one before; Esc leaves the text for the field, then Tab leaves
// and Esc goes on to the form; ⌘↵ adds no line break and reaches the form; none of it changes an untouched description.

import type { Editor } from "@tiptap/core";
import { act, createElement, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, expect, test, vi } from "vitest";

import { type MarkdownEditorOptions, MarkdownSurface, useMarkdownEditor } from "./Editor";
import { serializeMarkdown } from "./changes";
import { focusPast } from "./keys";
import { SourceArea } from "./source/SourceToggle";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement | null = null;
const shown: { editor?: Editor } = {};

/** Priority before the description and Start from and merge into after it, as in the form; `source` shows the Markdown source. */
function Form({
  onEditor,
  source,
  ...options
}: MarkdownEditorOptions & { onEditor: (editor: Editor) => void; source?: string }) {
  const editor = useMarkdownEditor(options);
  useEffect(() => {
    onEditor(editor);
  });
  return createElement(
    "div",
    { className: "form" },
    createElement("button", { id: "priority" }, "P3"),
    createElement(MarkdownSurface, {
      editor,
      ...(source === undefined
        ? {}
        : {
            source: createElement(SourceArea, {
              value: source,
              label: "What and why",
              takeFocus: false,
              onChange: () => undefined,
            }),
          }),
    }),
    createElement("input", { id: "base" }),
  );
}

function render(options: Omit<MarkdownEditorOptions, "label"> & { source?: string }) {
  host ??= document.body.appendChild(document.createElement("div"));
  root ??= createRoot(host);
  const page = host;
  act(() => {
    root?.render(
      createElement(Form, {
        label: "What and why",
        ...options,
        onEditor: (editor) => {
          shown.editor = editor;
        },
      }),
    );
  });
  if (!shown.editor) throw new Error("not rendered");
  const field = page.querySelector<HTMLElement>(".md-editor");
  if (!field) throw new Error("no field");
  return { editor: shown.editor, page, field };
}

/** Presses a key on whatever has focus; returns the event (was it taken?). */
function press(k: string, mods: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods });
  act(() => {
    (document.activeElement ?? document.body).dispatchEvent(event);
  });
  return event;
}

/** Puts the caret at the end of the first text containing `text`. */
function caretAfter(editor: Editor, text: string) {
  let at = -1;
  editor.state.doc.descendants((node, pos) => {
    if (at < 0 && node.isText && node.text?.includes(text))
      at = pos + (node.text.indexOf(text) + text.length);
  });
  act(() => {
    editor.view.focus();
    editor.commands.setTextSelection(at);
  });
}

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  delete shown.editor;
});

test("Tab nests a list item under the one above, ⇧Tab moves it back out", () => {
  const onChange = vi.fn();
  const { editor } = render({ value: "- Suggest addresses\n- Keyboard: arrows\n", onChange });
  caretAfter(editor, "Keyboard");
  expect(press("Tab").defaultPrevented).toBe(true);
  expect(serializeMarkdown(editor)).toBe("- Suggest addresses\n  - Keyboard: arrows");
  expect(document.activeElement).toBe(editor.view.dom);
  press("Tab", { shiftKey: true });
  expect(serializeMarkdown(editor)).toBe("- Suggest addresses\n- Keyboard: arrows");
});

test("Tab on the first item, which can't nest, stays in the list", () => {
  const onChange = vi.fn();
  const { editor } = render({ value: "- [ ] only\n", onChange });
  caretAfter(editor, "only");
  expect(press("Tab").defaultPrevented).toBe(true);
  expect(document.activeElement).toBe(editor.view.dom);
  expect(onChange).not.toHaveBeenCalled();
});

test("Tab and ⇧Tab indent and outdent the code line (Tiptap's own, two spaces)", () => {
  const { editor } = render({ value: "```ts\nif (a) {\nb();\n```\n" });
  caretAfter(editor, "{\n");
  press("Tab");
  expect(serializeMarkdown(editor)).toBe("```ts\nif (a) {\n  b();\n```");
  press("Tab", { shiftKey: true });
  expect(serializeMarkdown(editor)).toBe("```ts\nif (a) {\nb();\n```");
});

test("Tab out of a paragraph goes to Start from and merge into, ⇧Tab back to Priority, and nothing is reported", () => {
  const onChange = vi.fn();
  const { editor, page } = render({ value: "Users mistype addresses.\n", onChange });
  caretAfter(editor, "Users");
  press("Tab");
  expect(document.activeElement).toBe(page.querySelector("#base"));
  caretAfter(editor, "Users");
  press("Tab", { shiftKey: true });
  expect(document.activeElement).toBe(page.querySelector("#priority"));
  expect(onChange).not.toHaveBeenCalled();
});

test("Esc leaves the text for the field; then Tab leaves, ↵ goes back, and Esc goes on to the form", () => {
  const onChange = vi.fn();
  const { editor, page, field } = render({ value: "- one\n- two\n", onChange });
  const formEsc = vi.fn();
  page.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !e.defaultPrevented) formEsc();
  });
  caretAfter(editor, "two");
  expect(press("Escape").defaultPrevented).toBe(true);
  expect(document.activeElement).toBe(field);
  expect(formEsc).not.toHaveBeenCalled();
  press("Enter");
  expect(document.activeElement).toBe(editor.view.dom);
  press("Escape");
  press("Tab");
  expect(document.activeElement).toBe(page.querySelector("#base"));
  act(() => {
    field.focus();
  });
  expect(press("Escape").defaultPrevented).toBe(false);
  expect(formEsc).toHaveBeenCalledTimes(1);
  expect(onChange).not.toHaveBeenCalled();
});

test("⌘↵ adds no line break and goes on to the form", () => {
  const { editor, page } = render({ value: "line\n" });
  const saved = vi.fn();
  page.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && e.metaKey) saved();
  });
  caretAfter(editor, "line");
  press("Enter", { metaKey: true });
  expect(saved).toHaveBeenCalledTimes(1);
  expect(serializeMarkdown(editor)).toBe("line");
});

test("read mode keeps none of the keys", () => {
  const { field } = render({ value: "text\n", editable: false });
  expect(field.hasAttribute("tabindex")).toBe(false);
});

test("focusPast skips hidden and out-of-order elements, and wraps inside a modal", () => {
  document.body.innerHTML = `
    <div aria-modal="true">
      <button id="a">a</button>
      <div id="from" tabindex="-1"><button id="inside">x</button></div>
      <button id="skip" tabindex="-1">s</button>
    </div>`;
  const from = document.getElementById("from") as HTMLElement;
  expect(focusPast(from, false)).toBe(true);
  expect(document.activeElement?.id).toBe("a");
  expect(focusPast(from, true)).toBe(true);
  expect(document.activeElement?.id).toBe("a");
  document.body.innerHTML = "";
});

test("in the source view's text area (T-0251), Esc leaves it for the field and Tab leaves without indenting", () => {
  const { page, field } = render({ value: "- one\n", source: "- one\n" });
  const formEsc = vi.fn();
  const formSave = vi.fn();
  page.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !e.defaultPrevented) formEsc();
    if (e.key === "Enter" && e.metaKey) formSave();
  });
  const source = page.querySelector<HTMLTextAreaElement>("textarea.md-source");
  if (!source) throw new Error("no source");
  act(() => {
    source.focus();
  });
  expect(press("Escape").defaultPrevented).toBe(true);
  expect(document.activeElement).toBe(field);
  expect(formEsc).not.toHaveBeenCalled();
  // ↵ on the field goes back to the source, not the hidden rich text; a second Esc on the field reaches the form.
  press("Enter");
  expect(document.activeElement).toBe(source);
  press("Escape");
  press("Escape");
  expect(formEsc).toHaveBeenCalledTimes(1);
  act(() => {
    source.focus();
  });
  press("Enter", { metaKey: true });
  expect(formSave).toHaveBeenCalledTimes(1);
  expect(source.value).toBe("- one\n");
  act(() => {
    source.focus();
  });
  press("Tab");
  expect(document.activeElement).toBe(page.querySelector("#base"));
  expect(source.value).toBe("- one\n");
  act(() => {
    source.focus();
  });
  press("Tab", { shiftKey: true });
  expect(document.activeElement).toBe(page.querySelector("#priority"));
});
