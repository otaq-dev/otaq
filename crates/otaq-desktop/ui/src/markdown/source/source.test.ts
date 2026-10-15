// @vitest-environment happy-dom
// The Markdown source toggle (T-0251, d0078 D9): switching keeps edits both ways, and switching alone never counts as
// an edit, so an untouched description still saves byte for byte. The rules are plain functions (source.ts); the page
// tests run them with a real editor and text area, as the form's Description puts them together.

import type { Editor } from "@tiptap/core";
import { act, createElement, useEffect, useState } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, test, vi } from "vitest";

import { MarkdownSurface, useMarkdownEditor } from "../Editor";
import samples from "../roundtrip/samples.json";
import { MarkdownToolbar } from "../toolbar/Toolbar";
import { editorValue, openingMode, rememberMode, sourceStart, switchSource } from "./source";
import { SourceArea, SourceToggle, useFocusOnLeave, useSourceMode } from "./SourceToggle";

const EXAMPLE = samples.find((s) => s.kind === "example")?.markdown ?? "";

describe("the switching rules", () => {
  test("the editor keeps what it had while the source shows, then gets what the parent holds", () => {
    let state = sourceStart(false, "a");
    expect(editorValue(state, "a")).toBe("a");
    state = switchSource(state, "a");
    expect(state).toEqual({ on: true, entered: "a", switched: true });
    // Typing in the source changes what the parent holds, not what the editor is given.
    expect(editorValue(state, "a typed")).toBe("a");
    state = switchSource(state, "a typed");
    expect(state.on).toBe(false);
    expect(editorValue(state, "a typed")).toBe("a typed");
  });

  test("an editor opening in source mode keeps the value it opened with", () => {
    const state = sourceStart(true, "stored");
    expect(state.switched).toBe(false);
    expect(editorValue(state, "stored and typed")).toBe("stored");
  });

  test("a new editor opens in the mode last chosen in this run, over a view loaded before it", () => {
    rememberMode(true);
    expect(openingMode(false)).toBe(true);
    rememberMode(false);
    expect(openingMode(true)).toBe(false);
  });
});

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement | null = null;
const shown: { editor?: Editor } = {};

/** The form's Description, cut down: the parent holds the value, the editor and the source share it. */
function Field({
  stored,
  sourceMode,
  onChange,
}: {
  stored: string;
  sourceMode: boolean;
  onChange: (v: string) => void;
}) {
  const [value, setValue] = useState(stored);
  const change = (next: string) => {
    setValue(next);
    onChange(next);
  };
  const mode = useSourceMode(value, sourceMode);
  const editor = useMarkdownEditor({ value: mode.editorValue, onChange: change, label: "What and why" });
  useFocusOnLeave(editor, mode);
  useEffect(() => {
    shown.editor = editor;
  });
  return createElement(MarkdownSurface, {
    editor,
    toolbar: createElement(MarkdownToolbar, {
      disabled: mode.on,
      sourceToggle: createElement(SourceToggle, { on: mode.on, label: "Markdown", onChange: mode.toggle }),
    }),
    ...(mode.on
      ? {
          source: createElement(SourceArea, {
            value,
            label: "What and why",
            takeFocus: mode.switched,
            onChange: change,
          }),
        }
      : {}),
  });
}

function render(stored: string, sourceMode = false) {
  const onChange = vi.fn<(markdown: string) => void>();
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
  act(() => {
    root?.render(createElement(Field, { stored, sourceMode, onChange }));
  });
  const editor = shown.editor;
  if (!editor) throw new Error("not rendered");
  const page = host;
  return { editor, page, onChange };
}

function toggle(page: HTMLElement) {
  const el = page.querySelector<HTMLButtonElement>('[role="switch"]');
  if (!el) throw new Error("no toggle");
  act(() => {
    el.click();
  });
}

function area(page: HTMLElement): HTMLTextAreaElement | null {
  return page.querySelector<HTMLTextAreaElement>("textarea.md-source");
}

function typeInto(el: HTMLTextAreaElement, text: string) {
  act(() => {
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set?.call(el, text);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  delete shown.editor;
  rememberMode(false);
});

test("switching without typing changes nothing: the source shows the stored string, byte for byte", () => {
  const { editor, page, onChange } = render(EXAMPLE);
  const doc = editor.state.doc;
  toggle(page);
  expect(area(page)?.value).toBe(EXAMPLE);
  expect(page.querySelector(".md-editor")?.hasAttribute("data-source")).toBe(true);
  expect(page.querySelector('[role="switch"]')?.getAttribute("aria-checked")).toBe("true");
  toggle(page);
  expect(area(page)).toBeNull();
  toggle(page);
  toggle(page);
  expect(onChange).not.toHaveBeenCalled();
  // Not even parsed again: the rich text is the very document it was.
  expect(editor.state.doc).toBe(doc);
});

test("an edit in the rich text shows in the source", () => {
  const { editor, page, onChange } = render("## Why\nSome text");
  act(() => {
    editor.chain().focus("end").insertContent(" and more").run();
  });
  expect(onChange).toHaveBeenLastCalledWith("## Why\n\nSome text and more");
  toggle(page);
  expect(area(page)?.value).toBe("## Why\n\nSome text and more");
  expect(onChange).toHaveBeenCalledTimes(1);
});

test("an edit in the source shows in the rich text, and the typed text is what the form holds", () => {
  const { editor, page, onChange } = render("Some text");
  toggle(page);
  const el = area(page);
  if (!el) throw new Error("no source");
  typeInto(el, "Some *typed*   text\n\n* item");
  expect(onChange).toHaveBeenLastCalledWith("Some *typed*   text\n\n* item");
  toggle(page);
  expect(page.querySelector(".ProseMirror em")?.textContent).toBe("typed");
  expect(page.querySelector(".ProseMirror li")?.textContent).toBe("item");
  // Loading it back isn't an edit: the form keeps the typed spelling until the rich text is changed.
  expect(onChange).toHaveBeenCalledTimes(1);
  toggle(page);
  expect(area(page)?.value).toBe("Some *typed*   text\n\n* item");
  toggle(page);
  act(() => {
    editor.chain().focus(1).insertContent("New. ").run();
  });
  expect(onChange).toHaveBeenCalledTimes(2);
  expect(onChange).toHaveBeenLastCalledWith("New. Some *typed*   text\n\n- item");
});

test("an editor opens in source mode, and leaving it untouched changes nothing", () => {
  const { editor, page, onChange } = render(EXAMPLE, true);
  expect(area(page)?.value).toBe(EXAMPLE);
  // Opening in it doesn't take the caret (the form's title has it); switching does.
  expect(document.activeElement).not.toBe(area(page));
  toggle(page);
  expect(page.querySelector(".ProseMirror h2")?.textContent).toBe("Why");
  expect(onChange).not.toHaveBeenCalled();
  toggle(page);
  expect(document.activeElement).toBe(area(page));
  expect(editor.isEditable).toBe(true);
});

test("the formatting buttons do nothing while the source shows; the toggle still works", () => {
  const { editor, page } = render("plain");
  toggle(page);
  const toolbar = page.querySelector(".md-toolbar");
  expect(toolbar?.hasAttribute("data-disabled")).toBe(true);
  const bold = page.querySelector<HTMLButtonElement>('button[aria-label="Bold"]');
  expect(bold?.getAttribute("aria-disabled")).toBe("true");
  act(() => {
    editor.commands.selectAll();
    bold?.click();
  });
  expect(editor.isActive("bold")).toBe(false);
  expect(page.querySelector('button[aria-label="Heading"]')?.getAttribute("aria-disabled")).toBe("true");
  toggle(page);
  expect(toolbar?.hasAttribute("data-disabled")).toBe(false);
  expect(page.querySelector('button[aria-label="Bold"]')?.hasAttribute("aria-disabled")).toBe(false);
});
