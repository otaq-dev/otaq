import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

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
  highlighted,
  isSaveKey,
  quickKey,
  refusalFor,
  shownPriority,
  step,
  withDep,
  withoutDep,
} from "./logic";
import { Compare, Markdown, ReadMode } from "./ReadMode";
import { takeEditRequest } from "./TicketTab";

afterEach(() => {
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
  read: null,
};

const READ: EditorReadView = {
  origin: "Created by you  ·  edited by the agent · 4m ago",
  origin_agent: false,
  title: "Dark mode",
  body: [
    { kind: "paragraph", text: "Add a toggle." },
    { kind: "heading", text: "Done when" },
    { kind: "list", items: ["Toggle works"] },
    { kind: "code", text: "npm test" },
  ],
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

  it("read mode: origin, Markdown, the agent's marker with Compare, deps and the External ID", () => {
    const out = html(
      createElement(ReadMode, {
        ticket: "SHOP-12",
        read: READ,
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
      }),
    );
    for (const text of [
      "edited by the agent",
      "WHAT AND WHY",
      "Updated by agent",
      "Compare",
      "Done when",
      "SHOP-18",
      "PAY-123",
    ]) {
      expect(out).toContain(text);
    }
    expect(out.match(/Updated by agent/g)).toHaveLength(1);
    expect(out).not.toContain("Dark mode");
    expect(html(createElement(Markdown, { blocks: [], empty: TEXTS.nothing_written }))).toContain(
      "Nothing written yet.",
    );
  });

  it("Compare shows before and after", () => {
    const out = html(
      createElement(Compare, {
        entries: [
          {
            heading: "“Title” · changed by agent:SHOP-19 · 2m ago",
            before: "Dark mode",
            after: "Light / dark",
          },
        ],
        texts: TEXTS,
      }),
    );
    for (const text of ["BEFORE", "AFTER", "Dark mode", "Light / dark"]) expect(out).toContain(text);
  });

  it("registers quick-create, the edit modal and the Ticket tab from its folder", () => {
    expect(registered(registerScreens({ editor }))).toMatchObject({
      overlays: ["new_ticket", "edit_ticket"],
      panelTabs: ["ticket"],
    });
  });
});
