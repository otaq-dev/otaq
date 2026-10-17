// @vitest-environment happy-dom
// Settings → Agents (T-0343): the model picker takes no typed text, Esc closes the dialog without saving, ⋯ →
// Duplicate opens Add agent with "<name> copy", and Delete… opens the refused or the plain dialog as the view says.
import { act, createElement, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentDeleteDialog } from "../../generated/AgentDeleteDialog";
import type { AgentDialog as AgentDialogView } from "../../generated/AgentDialog";
import type { AgentDraft } from "../../generated/AgentDraft";
import type { AgentMenuItem } from "../../generated/AgentMenuItem";
import type { AgentsSection as AgentsSectionView } from "../../generated/AgentsSection";
import type { AgentTableRow } from "../../generated/AgentTableRow";
import type { ModelPickerView } from "../../generated/ModelPickerView";
import { request } from "../../tauri";
import { AgentDialog } from "./AgentDialog";
import { AgentsSection } from "./Agents";
import { agentChoices, modelChoices } from "./ModelPicker";

vi.mock("../../tauri", () => ({
  request: vi.fn(),
  subscribeEvents: vi.fn(() => Promise.resolve()),
  windowAction: vi.fn(),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// The shapes `named_agents.rs` and `model_picker.rs` send (their gallery fixtures).
const PICKER: ModelPickerView = {
  value: {
    label: "Fable · always the newest",
    detail: "now claude-fable-5-1",
    mono: false,
    no_longer_listed: null,
  },
  groups: [
    {
      heading: null,
      items: [
        { value: null, label: "Tool default", detail: "Otaq passes no model", mono: false, selected: false },
      ],
      empty: null,
    },
    {
      heading: "ALWAYS THE NEWEST",
      items: [
        {
          value: "opus",
          label: "Opus · always the newest",
          detail: "now claude-opus-5-5",
          mono: false,
          selected: false,
        },
        {
          value: "fable",
          label: "Fable · always the newest",
          detail: "now claude-fable-5-1",
          mono: false,
          selected: true,
        },
      ],
      empty: null,
    },
    { heading: "EXACT MODELS", items: [], empty: "Claude Code's list has only its aliases" },
  ],
  status: null,
  hint: "From Claude Code's own list. A follow choice is resolved when each session starts, and the session keeps the exact model it ran on.",
};

function dialogView(over: Partial<AgentDialogView> = {}): AgentDialogView {
  return {
    id: null,
    title: "Add agent",
    mark: null,
    name_label: "Name",
    draft: {
      name: "Fable test",
      tool: null,
      model: "fable",
      use_for: "Trying Fable on a few tickets",
      fallback: null,
    },
    name_problem: null,
    tool_label: "Tool",
    tool: "Claude Code",
    tool_avatar: null,
    tool_options: [],
    tool_note: "the only connected tool",
    tool_problem: null,
    model_label: "Model",
    picker: PICKER,
    model_problem: null,
    use_for_label: "Use for",
    use_for_hint: "Agents read this when they pick an agent for a ticket.",
    fallback: null,
    delete: null,
    cancel_label: "Cancel",
    save_label: "Add agent",
    save_enabled: true,
    ...over,
  };
}

const MENU: AgentMenuItem[] = [
  {
    action: "edit",
    icon: "pencil",
    label: "Edit…",
    danger: false,
    separated: false,
    enabled: true,
    tooltip: null,
  },
  {
    action: "duplicate",
    icon: "copy",
    label: "Duplicate",
    danger: false,
    separated: false,
    enabled: true,
    tooltip: null,
  },
  {
    action: "delete",
    icon: "trash-2",
    label: "Delete…",
    danger: true,
    separated: true,
    enabled: true,
    tooltip: null,
  },
];

function tableRow(id: string, name: string, model: string, useFor: string): AgentTableRow {
  return {
    id,
    name,
    tool: null,
    tool_name: null,
    model,
    model_mono: false,
    use_for: useFor,
    fallback: null,
    tool_problem: null,
    added: null,
    mark: null,
    menu_label: `More for ${name}`,
    menu: MENU,
  };
}

const SECTION: AgentsSectionView = {
  title: "Agents",
  intro:
    "Named agents are a tool, a model and what you use them for. Agents pick from these names when they create tickets.",
  table_title: "Named agents",
  columns: ["NAME", "MODEL", "USE FOR"],
  rows: [
    tableRow("cc", "Claude Code", "Tool default", "Everything, until you add more agents"),
    tableRow("strong", "Strong", "Opus · always the newest", "Complex features, tricky bugs"),
    tableRow("light", "Light", "Haiku · always the newest", "Typos, docs, small config"),
  ],
  add_label: "Add agent",
  fresh_note: null,
  defaults_title: "Default agent for each kind",
  defaults: [],
  defaults_note: "Projects can override any row in their own settings.",
  defaults_unchanged: null,
  fallbacks: {
    title: "Fallbacks",
    label: "Use fallback agents automatically",
    help: "When an agent's tool hits its usage limit or isn't available, waiting tickets start on that agent's fallback instead.",
    on: false,
    enabled: false,
    note: "Disabled: a fallback must be on another tool. Connect a second agent tool to use fallbacks.",
  },
};

const REFUSED: AgentDeleteDialog = {
  kind: "refused",
  title: "Light can't be deleted",
  text: "Light is used by 4 tickets and project docs-site. Change those first, then delete it.",
  lines: ["4 tickets · SHOP-24, SHOP-40.3, SHOP-41 and 1 more", "docs-site · Validator default"],
  tickets: ["SHOP-24", "SHOP-40.3", "SHOP-41", "SHOP-50"],
  show_tickets_label: "Show tickets",
  ok_label: "OK",
};

const CONFIRM: AgentDeleteDialog = {
  kind: "confirm",
  id: "strong",
  title: "Delete Strong?",
  text: "Nothing uses Strong: no unfinished ticket and no default.",
  note: "Finished tickets that ran on it keep their history as “Strong (deleted)”.",
  cancel_label: "Cancel",
  delete_label: "Delete",
};

let root: Root;
let box: HTMLElement;

beforeEach(() => {
  box = document.createElement("div");
  document.body.append(box);
  root = createRoot(box);
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  box.remove();
  document.body.innerHTML = "";
  vi.mocked(request).mockReset();
});

function show(element: ReactElement) {
  act(() => {
    root.render(element);
  });
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

function key(target: Element, k: string) {
  act(() => {
    target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
  });
}

function click(target: Element) {
  act(() => {
    (target as HTMLElement).click();
  });
}

const byText = (selector: string, text: string) =>
  [...document.querySelectorAll(selector)].find((el) => el.textContent.trim() === text);

function modelField(): HTMLButtonElement {
  const field = document.querySelector<HTMLButtonElement>(".settings-model-field");
  if (!field) throw new Error("no model field");
  return field;
}

function showDialog(over: Partial<AgentDialogView> = {}) {
  const calls = { drafts: [] as AgentDraft[], saves: [] as AgentDraft[], cancels: 0 };
  show(
    createElement(AgentDialog, {
      dialog: dialogView(over),
      onDraft: (d) => calls.drafts.push(d),
      onSave: (d) => calls.saves.push(d),
      onCancel: () => {
        calls.cancels += 1;
      },
      onDelete: () => undefined,
    }),
  );
  return calls;
}

describe("the model picker", () => {
  it("lists the view's groups in order, only what the tool's list has", () => {
    const lines = modelChoices(PICKER).map((c) => [c.kind, c.label, c.detail]);
    expect(lines).toEqual([
      ["option", "Tool default", "Otaq passes no model"],
      ["heading", "ALWAYS THE NEWEST", null],
      ["option", "Opus · always the newest", "now claude-opus-5-5"],
      ["option", "Fable · always the newest", "now claude-fable-5-1"],
      ["heading", "EXACT MODELS", null],
      ["note", "Claude Code's list has only its aliases", null],
    ]);
  });

  it("can't take typed text: no text field, and typing changes nothing", () => {
    const calls = showDialog();
    const picker = document.querySelector(".settings-model-picker");
    expect(picker?.querySelector("input, textarea, [contenteditable]")).toBeNull();
    const field = modelField();
    act(() => {
      field.focus();
    });
    key(field, "o");
    key(field, "p");
    // Open: the list takes the keys; letters still do nothing.
    click(field);
    const list = document.querySelector('[role="listbox"]');
    expect(list).not.toBeNull();
    key(list as Element, "x");
    expect(calls.drafts).toEqual([]);
    expect(document.querySelector(".settings-model-picker input")).toBeNull();
    // Only a pick from the list changes the model: ↑ from Fable to Opus, ↵.
    key(list as Element, "ArrowUp");
    key(list as Element, "Enter");
    expect(calls.drafts).toEqual([
      {
        name: "Fable test",
        tool: null,
        model: "opus",
        use_for: "Trying Fable on a few tickets",
        fallback: null,
      },
    ]);
    expect(document.querySelector('[role="listbox"]')).toBeNull();
  });

  it("picks the tool default as no model", () => {
    const calls = showDialog();
    click(modelField());
    const row = byText('[role="option"]', "Tool defaultOtaq passes no model");
    click(row as Element);
    expect(calls.drafts[0]?.model).toBeNull();
  });

  it("an agent picker with one agent is read-only; with more, each option names its model", () => {
    const options = agentChoices({
      value: { agent: null, tool: null, name: "Use app default (Claude Code)", model: null, selected: true },
      options: [
        { agent: null, tool: null, name: "Use app default (Claude Code)", model: null, selected: true },
        { agent: "strong", tool: null, name: "Strong", model: "Opus · always the newest", selected: false },
      ],
      read_only: false,
    });
    expect(options.map((o) => [o.id, o.label, o.detail, o.icon])).toEqual([
      ["", "Use app default (Claude Code)", null, "rotate-ccw"],
      ["strong", "Strong", "Opus · always the newest", null],
    ]);
  });
});

describe("the Add / Edit agent dialog", () => {
  it("Esc closes it without saving", () => {
    const calls = showDialog();
    const name = document.querySelector<HTMLInputElement>(".settings-agent-dialog input");
    key(name as Element, "Escape");
    expect(calls.cancels).toBe(1);
    expect(calls.saves).toEqual([]);
  });

  it("Esc in the open model list closes the list, not the dialog", () => {
    const calls = showDialog();
    click(modelField());
    key(document.querySelector('[role="listbox"]') as Element, "Escape");
    expect(document.querySelector('[role="listbox"]')).toBeNull();
    expect(calls.cancels).toBe(0);
  });

  it("↵ saves only while Save is enabled", () => {
    let calls = showDialog({ save_enabled: false, name_problem: "There's already an agent named Light" });
    key(document.querySelector(".settings-agent-dialog input") as Element, "Enter");
    expect(calls.saves).toEqual([]);
    expect(document.body.textContent).toContain("There's already an agent named Light");
    calls = showDialog();
    key(document.querySelector(".settings-agent-dialog input") as Element, "Enter");
    expect(calls.saves).toHaveLength(1);
  });

  it("shows the mark's amber box with the tool's message", () => {
    showDialog({
      id: "fable",
      title: "Edit agent · Fable test",
      mark: {
        before: "Claude Code couldn't run ",
        model: "claude-fable-5-1",
        after: " on SHOP-48",
        message: "There's an issue with the selected model (claude-fable-5-1).",
        edit_label: "Edit…",
      },
    });
    const mark = document.querySelector(".settings-agent-mark-box");
    expect(mark?.textContent).toContain("Claude Code couldn't run claude-fable-5-1 on SHOP-48");
    expect(mark?.querySelector("q")?.textContent).toBe(
      "There's an issue with the selected model (claude-fable-5-1).",
    );
  });
});

/** Answers the page's calls as the desktop would, recording them. */
function answer(replies: Record<string, (params: unknown) => unknown>) {
  const calls: [string, unknown][] = [];
  vi.mocked(request).mockImplementation((method: string, params: unknown = null) => {
    calls.push([method, params]);
    const reply = replies[method];
    return reply ? Promise.resolve(reply(params)) : Promise.reject(new Error(`unexpected ${method}`));
  });
  return calls;
}

async function openMenu(name: string, item: string) {
  const more = document.querySelector(`[aria-label="More for ${name}"]`);
  click(more as Element);
  const entry = byText(".menu-item", item);
  click(entry as Element);
  await settle();
}

describe("the Agents section", () => {
  it("shows Otaq's copy of the model list and reads the list again once when it opens", async () => {
    const calls = answer({ "settings.agents": () => SECTION });
    show(createElement(AgentsSection));
    await settle();
    // Otaq's copy first, then the tool's list read again, once.
    expect(calls).toEqual([
      ["settings.agents", { refresh: true }],
      ["settings.agents", { refresh: false }],
    ]);
    expect(document.querySelectorAll(".settings-agents-row[role=row]")).toHaveLength(4);
    expect(document.querySelector('[role="columnheader"]')?.textContent).toBe("NAME");
  });

  it("⋯ → Duplicate opens Add agent with “<name> copy”", async () => {
    const calls = answer({
      "settings.agents": () => SECTION,
      "settings.agent_dialog": () =>
        dialogView({
          draft: {
            name: "Strong copy",
            tool: null,
            model: "opus",
            use_for: "Complex features, tricky bugs",
            fallback: null,
          },
        }),
    });
    show(createElement(AgentsSection));
    await settle();
    await openMenu("Strong", "Duplicate");
    expect(calls).toContainEqual(["settings.agent_dialog", { duplicate: "strong" }]);
    expect(document.querySelector(".dialog-title")?.textContent).toBe("Add agent");
    expect(document.querySelector<HTMLInputElement>(".settings-agent-dialog input")?.value).toBe(
      "Strong copy",
    );
  });

  it("Delete… opens the refused dialog while something uses the agent, and Show tickets lists every key", async () => {
    const calls = answer({ "settings.agents": () => SECTION, "settings.agent_delete_dialog": () => REFUSED });
    show(createElement(AgentsSection));
    await settle();
    await openMenu("Light", "Delete…");
    expect(calls).toContainEqual(["settings.agent_delete_dialog", { id: "light" }]);
    expect(document.querySelector(".dialog-title")?.textContent).toBe("Light can't be deleted");
    expect(document.body.textContent).toContain("docs-site · Validator default");
    expect(byText("button", "Delete")).toBeUndefined();
    click(byText("button", "Show tickets") as Element);
    expect([...document.querySelectorAll(".settings-agent-ticket")].map((b) => b.textContent)).toEqual([
      "SHOP-24",
      "SHOP-40.3",
      "SHOP-41",
      "SHOP-50",
    ]);
  });

  it("Delete… of an unused agent asks once, and Delete deletes it", async () => {
    const calls = answer({
      "settings.agents": () => SECTION,
      "settings.agent_delete_dialog": () => CONFIRM,
      "settings.agent_delete": () => ({
        outcome: "deleted",
        section: { ...SECTION, rows: SECTION.rows.slice(0, 1) },
      }),
    });
    show(createElement(AgentsSection));
    await settle();
    await openMenu("Strong", "Delete…");
    expect(document.querySelector(".dialog-title")?.textContent).toBe("Delete Strong?");
    click(byText("button", "Delete") as Element);
    await settle();
    expect(calls).toContainEqual(["settings.agent_delete", { id: "strong" }]);
    expect(document.querySelector(".dialog")).toBeNull();
    expect(document.querySelectorAll(".settings-agents-row[role=row]")).toHaveLength(2);
  });
});
