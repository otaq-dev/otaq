// @vitest-environment happy-dom
// The full-window view in a page (T-0260, d0078 D11 / D4): Expand moves the same editor over the window and Collapse
// brings it back, with its text, caret and undo history; neither counts as an edit; Esc collapses it once a popover
// has closed; ⌘↵ saves; the view closes with its owner.

import type { Editor } from "@tiptap/core";
import { closeHistory } from "@tiptap/pm/history";
import { act, createElement, Fragment, useEffect } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, expect, test, vi } from "vitest";

import { overlays, useOpenOverlays } from "../../app/overlays";
import { MarkdownSurface, useMarkdownEditor } from "../Editor";
import { serializeMarkdown } from "../changes";
import { FullView, FullViewHost, useFullView } from "./FullView";
import { fullViewSlot } from "./slot";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let root: Root | null = null;
let host: HTMLElement | null = null;
const shown: { editor?: Editor; expand?: () => void; collapse?: () => void } = {};

/** The frame's overlay host, reduced to the one overlay. */
function Overlays() {
  const open = useOpenOverlays();
  return createElement(
    Fragment,
    null,
    open.map((o) =>
      o.id === "full_view"
        ? createElement(FullViewHost, {
            key: o.id,
            params: o.params,
            close: () => {
              overlays.close(o.id);
            },
          })
        : null,
    ),
  );
}

/** A form's description, as Fields.tsx draws it: one instance, in the field or in the view. */
function Form({ value, onChange, onSave }: { value: string; onChange: () => void; onSave: () => void }) {
  const editor = useMarkdownEditor({ value, onChange, label: "What and why" });
  const full = useFullView();
  useEffect(() => {
    shown.editor = editor;
    shown.expand = full.expand;
    shown.collapse = full.collapse;
  });
  return createElement(
    "div",
    { className: "form" },
    createElement(
      "div",
      { className: "field" },
      full.slot === null && createElement(MarkdownSurface, { editor }),
    ),
    full.slot !== null &&
      createElement(FullView, {
        slot: full.slot,
        ticketKey: "SHOP-47",
        name: "What and why",
        title: "Checkout address autocomplete",
        state: "Editing",
        collapseLabel: "Collapse",
        onCollapse: full.collapse,
        onSave,
        editor: createElement(MarkdownSurface, {
          editor,
          className: "md-fullview-field",
          toolbar: createElement("div", { role: "menu", className: "test-menu", tabIndex: -1 }),
        }),
      }),
  );
}

function render(value: string, shownForm = true) {
  const onChange = vi.fn();
  const onSave = vi.fn();
  host ??= document.body.appendChild(document.createElement("div"));
  root ??= createRoot(host);
  act(() => {
    root?.render(
      createElement(
        Fragment,
        null,
        shownForm && createElement(Form, { value, onChange, onSave }),
        createElement("div", { id: "overlay-host" }, createElement(Overlays)),
      ),
    );
  });
  const editor = shown.editor;
  if (!editor) throw new Error("not rendered");
  return { editor, onChange, onSave };
}

const inView = (editor: Editor) => editor.view.dom.closest(".md-fullview") !== null;
const inField = (editor: Editor) => editor.view.dom.closest(".field") !== null;

function expand() {
  act(() => {
    shown.expand?.();
  });
}

function collapse() {
  act(() => {
    shown.collapse?.();
  });
}

function press(k: string, mods: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods });
  act(() => {
    (document.activeElement ?? document.body).dispatchEvent(event);
  });
  return event;
}

/** Types `text` at the caret as its own undo step. */
function type(editor: Editor, text: string) {
  act(() => {
    editor.view.dispatch(closeHistory(editor.state.tr));
    editor.commands.insertContent(text);
  });
}

afterEach(() => {
  act(() => root?.unmount());
  host?.remove();
  root = null;
  host = null;
  delete shown.editor;
  act(() => {
    overlays.store.set([]);
  });
});

test("Expand and Collapse keep the text, caret and undo history", () => {
  const { editor, onChange } = render("Arrow keys move the highlight\n");
  act(() => {
    editor.view.focus();
    editor.commands.setTextSelection(6);
  });
  type(editor, "X");
  expect(inField(editor)).toBe(true);

  expand();
  expect(inView(editor)).toBe(true);
  expect(document.querySelectorAll(".md-fullview .ProseMirror")).toHaveLength(1);
  expect(editor.state.selection.from).toBe(7);
  expect(serializeMarkdown(editor)).toBe("ArrowX keys move the highlight");
  type(editor, "Y");

  collapse();
  expect(inField(editor)).toBe(true);
  expect(document.querySelector(".md-fullview")).toBeNull();
  expect(editor.state.selection.from).toBe(8);
  expect(serializeMarkdown(editor)).toBe("ArrowXY keys move the highlight");
  expect(onChange).toHaveBeenLastCalledWith("ArrowXY keys move the highlight");

  // Undo after collapsing undoes what was typed in the full view, then what was typed before it.
  act(() => {
    editor.commands.undo();
  });
  expect(serializeMarkdown(editor)).toBe("ArrowX keys move the highlight");
  expand();
  act(() => {
    editor.commands.undo();
  });
  expect(serializeMarkdown(editor)).toBe("Arrow keys move the highlight");
});

test("expanding and collapsing alone isn't an edit", () => {
  const stored = "- [ ] Suggest addresses\n\n| a | b |\n|---|---|\n| 1 | 2 |\n";
  const { onChange } = render(stored);
  expand();
  collapse();
  expand();
  collapse();
  expect(onChange).not.toHaveBeenCalled();
});

test("the view is a modal overlay with the header's words, and takes the focus", () => {
  render("text\n");
  expand();
  expect(overlays.store.get().map((o) => o.id)).toEqual(["full_view"]);
  const view = document.querySelector<HTMLElement>(".md-fullview");
  expect(view?.getAttribute("aria-modal")).toBe("true");
  expect(view?.getAttribute("aria-label")).toBe("SHOP-47 · What and why");
  expect(view?.textContent).toContain("Checkout address autocomplete");
  expect(view?.textContent).toContain("Editing");
  expect(view?.contains(document.activeElement)).toBe(true);
});

test("Esc in the text collapses the view, and the next Esc is the field's again", () => {
  const { editor } = render("text\n");
  expand();
  act(() => {
    editor.view.focus();
  });
  const esc = press("Escape");
  expect(esc.defaultPrevented).toBe(true);
  expect(document.querySelector(".md-fullview")).toBeNull();
  expect(overlays.store.get()).toEqual([]);
  // Collapsed: Esc in the text leaves it for the field (T-0259).
  act(() => {
    editor.view.focus();
  });
  press("Escape");
  expect(document.activeElement).toBe(editor.view.dom.closest(".md-editor"));
});

test("Esc in a popover or menu of the view is the popover's: the view stays", () => {
  render("text\n");
  expand();
  const menu = document.querySelector<HTMLElement>(".md-fullview .test-menu");
  act(() => {
    menu?.focus();
  });
  expect(press("Escape").defaultPrevented).toBe(false);
  expect(document.querySelector(".md-fullview")).not.toBeNull();
});

test("⌘↵ in the view saves without a line break", () => {
  const { editor, onSave, onChange } = render("text\n");
  expand();
  act(() => {
    editor.view.focus();
  });
  expect(press("Enter", { metaKey: true }).defaultPrevented).toBe(true);
  expect(onSave).toHaveBeenCalledTimes(1);
  expect(onChange).not.toHaveBeenCalled();
});

test("the view closes with its owner", () => {
  render("text\n");
  expand();
  expect(fullViewSlot.get()).not.toBeNull();
  render("text\n", false);
  expect(overlays.store.get()).toEqual([]);
  expect(fullViewSlot.get()).toBeNull();
});
