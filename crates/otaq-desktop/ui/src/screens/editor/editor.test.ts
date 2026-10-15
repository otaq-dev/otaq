// @vitest-environment happy-dom
// Read mode mounts the description editor, which needs a DOM.
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { registered, registerScreens } from "../../app/registry";
import { NOTHING_SELECTED, requestTicketEdit, selection, ticketEditRequests } from "../../app/selection";
import type { EditorForm } from "../../generated/EditorForm";
import type { EditorReadView } from "../../generated/EditorReadView";
import type { EditorTexts } from "../../generated/EditorTexts";
import type { EditorView } from "../../generated/EditorView";
import type { PriorityView } from "../../generated/PriorityView";
import { editor } from ".";
import { EditorFields, RefusalBox } from "./Fields";
import {
  canSave,
  changed,
  COPIED_FOR,
  copyMarkdown,
  highlighted,
  isSaveKey,
  quickKey,
  refusalFor,
  shownPriority,
  step,
  withDep,
  withoutDep,
} from "./logic";
import { Compare, ReadMode } from "./ReadMode";
import { takeEditRequest } from "./TicketTab";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  selection.set(NOTHING_SELECTED);
  ticketEditRequests.set(null);
});

const priority = (number: number, word: string): PriorityView => ({
  number,
  short: `P${String(number)}`,
  word,
  label: `P${String(number)} · ${word}`,
  icon: "equal",
  ink: "muted",
});

const TEXTS: EditorTexts = {
  title: "Title",
  title_placeholder: "Short, imperative title",
  title_required: "A title is required",
  priority: "Priority",
  priority_hint: "3 Normal by default · decides start order among ready tickets",
  kind: "Type",
  kind_ticket: "Ticket",
  kind_epic: "Epic",
  epic: "Epic",
  epic_hint: "optional · fixed once created (it sets the key)",
  epic_none: "No epic",
  body: "What and why",
  body_hint: "Markdown · code blocks",
  body_placeholder: "Context, constraints, links…",
  deps: "Depends on",
  deps_hint: "same project only",
  deps_placeholder: "Search tickets by # or title",
  deps_none: "None",
  base: "Start from and merge into",
  base_hint: "project default · local and remote branches · locked once the ticket starts",
  external_id: "External ID",
  external_id_hint: "optional · standalone tickets only · used in the branch and PR title",
  external_id_placeholder: "e.g. PAY-123",
  save_word: "create",
  discard_word: "discard",
  close_word: "close",
  cancel: "Cancel",
  close: "Close",
  primary: "Create ticket",
  edit: "Edit",
  back: "Back",
  in_word: "in",
  quick_create: "create",
  quick_full: "full form",
  open_terminal: "Open terminal",
  marker: "Updated by agent",
  compare: "Compare",
  before: "BEFORE",
  after: "AFTER",
  nothing_written: "Nothing written yet.",
  copy_markdown: "Copy as Markdown",
  copied: "Copied ✓",
  expand: "Expand",
  compare_view: "Show the description",
  rendered: "Rendered",
  markdown: "Markdown",
};

const FORM: EditorForm = {
  title: "",
  body: "",
  priority: null,
  epic_kind: false,
  epic: null,
  deps: [],
  base_branch: "main",
  external_id: "",
};

const VIEW: EditorView = {
  create: true,
  key: null,
  project: "SHOP",
  project_name: "shop-web",
  heading: "New ticket",
  texts: TEXTS,
  form: FORM,
  priorities: [priority(1, "Urgent"), priority(2, "High"), priority(3, "Normal"), priority(4, "Low")].map(
    (view) => ({
      number: view.number,
      view,
    }),
  ),
  shown_priority: 3,
  follows: null,
  epic_menu: [
    {
      id: "",
      label: "No epic",
      icon: null,
      shortcut: null,
      danger: false,
      disabled: false,
      hint: null,
      separated: false,
      checked: false,
      header: false,
    },
    {
      id: "SHOP-40",
      label: "SHOP-40 Wishlist",
      icon: "layers",
      shortcut: null,
      danger: false,
      disabled: false,
      hint: "1 / 3 merged",
      separated: false,
      checked: false,
      header: false,
    },
  ],
  base: { kind: "editable" },
  external_id_shown: true,
  deps_shown: true,
  deps: [],
  save_hint: null,
  save_warning: null,
  unsaved_warning: null,
  read: null,
};

const READ: EditorReadView = {
  origin: "Created by you  ·  edited by the agent · 4m ago",
  origin_agent: false,
  title: "Dark mode",
  body_markdown: "Add a toggle.\n\n## Done when\n\n- Toggle works\n\n```\nnpm test\n```",
  body_empty: false,
  priority: priority(3, "Normal"),
  priority_menu: [],
  follows: null,
  deps: [{ key: "SHOP-18", pill: { palette: "review", icon: "eye", label: "Ready for Review" } }],
  base: "main",
  external_id: "PAY-123",
  marked: ["body"],
};

const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);
const noop = () => undefined;

const readMode = (read: EditorReadView = READ, onExpand?: () => void) =>
  createElement(ReadMode, {
    ticket: "SHOP-12",
    read,
    texts: TEXTS,
    labels: {
      title: "Title",
      body: "What and why",
      priority: "Priority",
      deps: "Depends on",
      base: "Start from and merge into",
      externalId: "External ID",
    },
    onCompare: noop,
    ...(onExpand === undefined ? {} : { onExpand }),
  });

let unmount = noop;

/** Renders into a page (the description editor draws its text after mounting). */
function mount(el: Parameters<typeof renderToStaticMarkup>[0]): HTMLElement {
  unmount();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  act(() => {
    root.render(el);
  });
  unmount = () => {
    act(() => {
      root.unmount();
    });
    host.remove();
    unmount = noop;
  };
  return host;
}

describe("Copy as Markdown", () => {
  it("puts exactly the stored text on the clipboard, as plain text", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    for (const stored of [
      "## Why\nUsers mistype addresses; **12%** (see [report](https://example.com/report)).\n",
      "* star bullets\r\n1) paren list  \r\nno final newline",
      "Escaped \\* and \\_ <kbd>⌘</kbd>\n\n~~~ts\nconst a = 1;\n~~~\n\n",
      "",
    ]) {
      await copyMarkdown(stored, { writeText });
      expect(writeText).toHaveBeenLastCalledWith(stored);
    }
    expect(writeText).toHaveBeenCalledTimes(4);
  });
});

describe("the editor's form state", () => {
  it("quick-create: ↵ creates, ⇧↵ opens the full form, Esc cancels", () => {
    expect(quickKey({ key: "Enter", shiftKey: false })).toBe("create");
    expect(quickKey({ key: "Enter", shiftKey: true })).toBe("full");
    expect(quickKey({ key: "Escape", shiftKey: false })).toBe("cancel");
    expect(quickKey({ key: "Enter", shiftKey: false, isComposing: true })).toBeNull();
    expect(quickKey({ key: "a", shiftKey: false })).toBeNull();
  });

  it("⌘↵ saves from any field; save needs a title", () => {
    expect(isSaveKey({ key: "Enter", metaKey: true, ctrlKey: false })).toBe(true);
    expect(isSaveKey({ key: "Enter", metaKey: false, ctrlKey: true })).toBe(true);
    expect(isSaveKey({ key: "Enter", metaKey: false, ctrlKey: false })).toBe(false);
    expect(canSave({ ...FORM, title: "  " }, false)).toBe(false);
    expect(canSave({ ...FORM, title: "Checkout" }, true)).toBe(false);
    expect(canSave({ ...FORM, title: "Checkout" }, false)).toBe(true);
  });

  it("priority shows the picked one, else the view's; deps are added once", () => {
    expect(shownPriority(VIEW, FORM)).toBe(3);
    expect(shownPriority(VIEW, { ...FORM, priority: 1 })).toBe(1);
    const one = withDep(withDep(FORM, "SHOP-18"), "SHOP-18");
    expect(one.deps).toEqual(["SHOP-18"]);
    expect(withoutDep(one, "SHOP-18").deps).toEqual([]);
    expect(changed(FORM, one)).toBe(true);
    expect(changed(FORM, { ...FORM })).toBe(false);
  });

  it("the picker highlights the first row that can be picked and steps over the rest", () => {
    const enabled = [false, true, false, true];
    expect(highlighted(null, enabled)).toBe(1);
    expect(highlighted(0, enabled)).toBe(1);
    expect(step(null, 1, enabled)).toBe(3);
    expect(step(3, 1, enabled)).toBe(3);
    expect(step(3, -1, enabled)).toBe(1);
    expect(highlighted(null, [false])).toBeNull();
  });

  it("a refusal shows under its own field", () => {
    const refusal = {
      field: "deps" as const,
      title: "That would create a loop",
      message: "SHOP-18 waits on it",
      icon: "refresh-ccw" as const,
    };
    expect(refusalFor(refusal, "deps")).toBe(refusal);
    expect(refusalFor(refusal, "base")).toBeNull();
    const out = html(createElement(RefusalBox, { refusal }));
    expect(out).toContain("That would create a loop");
    expect(out).toContain("SHOP-18 waits on it");
  });

  it("an edit request is taken once, by its own ticket's tab", () => {
    requestTicketEdit("SHOP-12");
    expect(selection.get().tab).toBe("ticket");
    expect(takeEditRequest("SHOP-19")).toBe(false);
    expect(takeEditRequest("SHOP-12")).toBe(true);
    expect(takeEditRequest("SHOP-12")).toBe(false);
  });
});

describe("the editor", () => {
  it("the full form: every field of a new ticket, the title error once a save was tried", () => {
    const props = {
      view: VIEW,
      form: FORM,
      onChange: noop,
      titleMissing: true,
      refusal: null,
      chips: [],
      onAddDep: noop,
      onRemoveDep: noop,
    };
    const out = html(createElement(EditorFields, props));
    for (const text of [
      "Title",
      "A title is required",
      "P3 Normal",
      "Ticket",
      "No epic",
      "What and why",
      "Depends on",
      "same project only",
      "Start from and merge into",
      "External ID",
    ]) {
      expect(out).toContain(text);
    }
    // An epic child has no base or External ID of its own; an epic has no Depends on.
    const child = html(createElement(EditorFields, { ...props, form: { ...FORM, epic: "SHOP-40" } }));
    expect(child).not.toContain("External ID");
    expect(child).not.toContain("Start from and merge into");
    const epic = html(createElement(EditorFields, { ...props, form: { ...FORM, epic_kind: true } }));
    expect(epic).not.toContain("same project only");
  });

  it("a locked base shows why", () => {
    const out = html(
      createElement(EditorFields, {
        view: { ...VIEW, create: false, base: { kind: "locked", text: "locked · the ticket has started" } },
        form: FORM,
        onChange: noop,
        titleMissing: false,
        refusal: null,
        chips: [],
        onAddDep: noop,
        onRemoveDep: noop,
      }),
    );
    expect(out).toContain("locked · the ticket has started");
    expect(out).toContain("disabled");
  });

  it("read mode: origin, the rendered Markdown, the agent's marker with Compare, deps and the External ID", () => {
    const page = mount(readMode());
    const out = page.innerHTML;
    for (const text of [
      "edited by the agent",
      "WHAT AND WHY",
      "Updated by agent",
      "Compare",
      "SHOP-18",
      "PAY-123",
    ]) {
      expect(out).toContain(text);
    }
    expect(out.match(/Updated by agent/g)).toHaveLength(1);
    expect(out).not.toContain("Dark mode");
    // Rendered by the read-only editor, not shown as source.
    const text = page.querySelector(".editor-read-body .ProseMirror");
    expect(text?.getAttribute("contenteditable")).toBe("false");
    expect(text?.querySelector("h2")?.textContent).toBe("Done when");
    expect(text?.querySelector("li")?.textContent).toBe("Toggle works");
    expect(text?.querySelector("pre")?.textContent).toBe("npm test");
    expect(out).not.toContain("## Done when");
    unmount();
  });

  it("the label row: marker and Compare, then Copy as Markdown and Expand, in that order", () => {
    const page = mount(readMode());
    const row = page.querySelector(".editor-section");
    const order = Array.from(row?.querySelectorAll(".editor-marker, .editor-link, .editor-tool") ?? []).map(
      (el) => el.textContent || el.getAttribute("aria-label"),
    );
    expect(order).toEqual(["Updated by agent", "Compare", "Copy as Markdown", "Expand"]);
    unmount();
  });

  it("the icons show without a marker too, and an empty description says nothing is written", () => {
    const page = mount(readMode({ ...READ, marked: [], body_markdown: "  \n", body_empty: true }));
    expect(page.querySelectorAll(".editor-section .editor-tool")).toHaveLength(2);
    expect(page.querySelector(".editor-read-body")).toBeNull();
    expect(page.textContent).toContain("Nothing written yet.");
    unmount();
  });

  it("Expand is a slot for the full-window view: disabled until it is given a handler", () => {
    const expand = (page: HTMLElement) => page.querySelector<HTMLButtonElement>('[aria-label="Expand"]');
    expect(expand(mount(readMode()))?.disabled).toBe(true);
    const onExpand = vi.fn();
    const page = mount(readMode(READ, onExpand));
    act(() => expand(page)?.click());
    expect(onExpand).toHaveBeenCalledOnce();
    unmount();
  });

  it("Copy as Markdown copies the stored text, then shows a check for 1.5 s", async () => {
    vi.useFakeTimers();
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    const stored = "## Why\r\nUsers *mistype*  \r\n* [x] done\n\n| a |\n|:-|\n| \\* |";
    const page = mount(readMode({ ...READ, body_markdown: stored }));
    const copy = page.querySelector<HTMLButtonElement>('[aria-label="Copy as Markdown"]');
    await act(async () => {
      copy?.click();
      await Promise.resolve();
    });
    expect(writeText).toHaveBeenCalledWith(stored);
    expect(copy?.hasAttribute("data-copied")).toBe(true);
    act(() => {
      vi.advanceTimersByTime(COPIED_FOR);
    });
    expect(copy?.hasAttribute("data-copied")).toBe(false);
    unmount();
  });

  it("Compare shows other fields as plain text, without the toggle", () => {
    const page = mount(
      createElement(Compare, {
        entries: [
          {
            field: "title",
            heading: "“Title” · changed by agent:SHOP-19 · 2m ago",
            before: "Dark *mode*",
            after: "Light / dark",
          },
        ],
        texts: TEXTS,
      }),
    );
    for (const text of ["BEFORE", "AFTER", "Dark *mode*", "Light / dark"])
      expect(page.textContent).toContain(text);
    expect(page.querySelector(".ProseMirror")).toBeNull();
    expect(page.querySelector('[role="radiogroup"]')).toBeNull();
    unmount();
  });

  it("Compare renders the description on both sides, and the toggle shows its source", () => {
    const before = "Add a toggle.\n\n## Done when\n\n- [x] Toggle works";
    const after = "Add a toggle in **Settings**.\n\n## Done when\n\n- [x] Toggle works\n- [ ] Persists";
    const page = mount(
      createElement(Compare, {
        entries: [{ field: "body", heading: "“What and why” (the description)", before, after }],
        texts: TEXTS,
      }),
    );
    const side = (name: string) => page.querySelector(`.editor-compare-col[data-side="${name}"]`);
    const segments = () => Array.from(page.querySelectorAll<HTMLButtonElement>('[role="radio"]'));
    // Rendered by default: read-only editors on both sides.
    expect(segments().map((b) => [b.textContent, b.getAttribute("aria-checked")])).toEqual([
      ["Rendered", "true"],
      ["Markdown", "false"],
    ]);
    for (const name of ["before", "after"]) {
      const text = side(name)?.querySelector(".editor-read-body .ProseMirror");
      expect(text?.getAttribute("contenteditable")).toBe("false");
      expect(text?.querySelector("h2")?.textContent).toBe("Done when");
    }
    expect(side("after")?.querySelector("strong")?.textContent).toBe("Settings");
    expect(side("after")?.querySelectorAll("li")).toHaveLength(2);
    expect(page.textContent).not.toContain("## Done when");
    // Markdown: the stored source, as is.
    act(() => segments()[1]?.click());
    expect(page.querySelector(".ProseMirror")).toBeNull();
    expect(side("before")?.querySelector(".editor-compare-source")?.textContent).toBe(before);
    expect(side("after")?.querySelector(".editor-compare-source")?.textContent).toBe(after);
    act(() => segments()[0]?.click());
    expect(side("after")?.querySelector(".ProseMirror h2")?.textContent).toBe("Done when");
    unmount();
  });

  it("registers quick-create, the edit modal and the Ticket tab from its folder", () => {
    expect(registered(registerScreens({ editor }))).toMatchObject({
      overlays: ["new_ticket", "edit_ticket"],
      panelTabs: ["ticket"],
    });
  });
});
