// @vitest-environment happy-dom
// Keys and focus in the form (T-0259, d0078 D3 / D4): the ⌘N modal (a Dialog whose Esc cancels, `useFormKeys` for ⌘↵)
// and the Ticket tab (`useFormKeys` with Esc to discard) wait while the description or a menu has the key.
import type { Editor } from "@tiptap/core";
import { act, createElement, useEffect, useRef } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { Dialog } from "../../components";
import { MarkdownSurface, serializeMarkdown, useMarkdownEditor } from "../../markdown";
import { useFormKeys } from "./Modal";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

interface Calls {
  save: () => void;
  discard: () => void;
  cancel: () => void;
  onEditor: (editor: Editor) => void;
}

/** Priority, the description and Depends on, with a menu open in the form, the way both forms hold them. */
function Fields({ onEditor, value }: { onEditor: (editor: Editor) => void; value: string }) {
  const editor = useMarkdownEditor({ value, label: "What and why" });
  useEffect(() => {
    onEditor(editor);
  });
  return createElement(
    "div",
    null,
    createElement("button", { id: "priority" }, "P3"),
    createElement(MarkdownSurface, { editor }),
    createElement("input", { id: "deps" }),
    createElement("div", { role: "menu", tabIndex: -1, id: "menu" }),
  );
}

interface FormProps extends Calls {
  value: string;
  discards?: boolean;
}

function TicketTabForm({ save, discard, onEditor, value, discards = true }: FormProps) {
  const root = useRef<HTMLDivElement>(null);
  useFormKeys(root, save, discards ? discard : undefined);
  // eslint-disable-next-line react-hooks/refs -- the ref is handed to the div, as JSX's ref= does; nothing reads it here
  return createElement("div", { ref: root }, createElement(Fields, { onEditor, value }));
}

function ModalForm(props: FormProps) {
  return createElement(Dialog, {
    title: "New ticket",
    footer: null,
    onCancel: props.cancel,
    onConfirm: props.save,
    children: createElement(TicketTabForm, { ...props, discards: false }),
  });
}

let root: Root | null = null;
let host: HTMLElement | null = null;

function mount(kind: "modal" | "tab", value: string) {
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
  const shown: { editor?: Editor } = {};
  const calls: Calls = {
    save: vi.fn(),
    discard: vi.fn(),
    cancel: vi.fn(),
    onEditor: (editor) => {
      shown.editor = editor;
    },
  };
  act(() => {
    root?.render(createElement(kind === "modal" ? ModalForm : TicketTabForm, { ...calls, value }));
  });
  const editor = shown.editor;
  if (!editor) throw new Error("no editor");
  return { calls, editor, page: host };
}

function press(k: string, mods: KeyboardEventInit = {}, on?: Element | null): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...mods });
  act(() => {
    (on ?? document.activeElement ?? document.body).dispatchEvent(event);
  });
  return event;
}

function caretIn(editor: Editor, text: string) {
  let at = -1;
  editor.state.doc.descendants((node, pos) => {
    if (at < 0 && node.isText && node.text?.includes(text)) at = pos + node.text.indexOf(text) + text.length;
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
});

describe.each(["tab", "modal"] as const)("the %s form", (kind) => {
  // The Ticket tab's form discards on Esc; the ⌘N modal leaves Esc to its dialog, which cancels.
  const stepsBack = (calls: Calls) =>
    kind === "tab" ? (calls.discard as ReturnType<typeof vi.fn>) : (calls.cancel as ReturnType<typeof vi.fn>);

  it("Esc leaves the text first, then the form goes", () => {
    const { calls, editor } = mount(kind, "- one\n- two\n");
    caretIn(editor, "two");
    press("Escape");
    expect(stepsBack(calls)).not.toHaveBeenCalled();
    expect(document.activeElement?.classList.contains("md-editor")).toBe(true);
    press("Escape");
    expect(stepsBack(calls)).toHaveBeenCalledTimes(1);
  });

  it("Esc in a menu is the menu's", () => {
    const { calls, page } = mount(kind, "text\n");
    press("Escape", {}, page.querySelector("#menu"));
    expect(stepsBack(calls)).not.toHaveBeenCalled();
  });

  it("⌘↵ in the text saves, with no line break", () => {
    const { calls, editor } = mount(kind, "line\n");
    caretIn(editor, "line");
    press("Enter", { metaKey: true });
    expect(calls.save).toHaveBeenCalledTimes(1);
    expect(serializeMarkdown(editor)).toBe("line");
  });

  it("Tab nests a list item and stays; Tab from a paragraph goes to Depends on, ⇧Tab to Priority", () => {
    const { editor, page } = mount(kind, "- one\n- two\n\nend\n");
    caretIn(editor, "two");
    press("Tab");
    expect(serializeMarkdown(editor)).toBe("- one\n  - two\n\nend");
    expect(document.activeElement).toBe(editor.view.dom);
    caretIn(editor, "end");
    press("Tab");
    expect(document.activeElement).toBe(page.querySelector("#deps"));
    caretIn(editor, "end");
    press("Tab", { shiftKey: true });
    expect(document.activeElement).toBe(page.querySelector("#priority"));
  });
});
