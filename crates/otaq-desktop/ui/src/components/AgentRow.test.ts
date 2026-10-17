// @vitest-environment happy-dom
// The agent rows (T-0344): mounted, since the picker opens, takes the keyboard and closes.
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentBlock as AgentBlockView } from "../generated/AgentBlock";
import type { AgentRowView } from "../generated/AgentRowView";
import { AgentBlock, type AgentBlockProps, shownRow } from "./AgentRow";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let host: HTMLElement;
let root: Root;

beforeEach(() => {
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  host.remove();
  vi.useRealTimers();
});

/** Hand-written as `agent_rows.rs` words them. */
const items = (selected: string | null) =>
  [
    { agent: null, label: "Project default (Claude Code)", detail: "tool default" },
    { agent: "cc", label: "Claude Code", detail: "tool default" },
    { agent: "strong", label: "Strong", detail: "Opus · always the newest" },
    { agent: "fable", label: "Fable test", detail: "Fable · always the newest" },
  ].map((i) => ({ ...i, selected: i.agent === selected, avatar: null }));

const row = (over: Partial<AgentRowView>): AgentRowView => ({
  role: "developer",
  label: "Developer",
  value: "Project default (Claude Code)",
  detail: "tool default",
  set_by: null,
  let_agents_pick: null,
  lock: null,
  picker: items(null),
  change: "Change",
  moved_on: null,
  next_lead: null,
  avatar: null,
  swap: null,
  moved: null,
  ...over,
});

const SET_BY_YOU = row({
  value: "Fable test",
  detail: "Fable · always the newest",
  set_by: { text: "set by you", agent_made: false, icon: "user" },
  let_agents_pick: "Let agents pick again",
  picker: items("fable"),
});
const VALIDATOR = row({ role: "validator", label: "Validator" });
const LOCKED = row({
  value: "Strong",
  detail: "claude-opus-5-5 · started",
  set_by: { text: "set by you", agent_made: false, icon: "user" },
  lock: "Locked: Send back continues the same conversation",
  picker: null,
  change: null,
});
const ONLY = row({ value: "Claude Code", picker: null, change: null });

const block = (rows: AgentRowView[], over: Partial<AgentBlockView> = {}): AgentBlockView => ({
  heading: "Agents",
  hint: "who writes the code and who checks it",
  rows,
  note: null,
  note_icon: null,
  children: [],
  children_heading: null,
  children_note: null,
  picked: null,
  ...over,
});

function show(props: Partial<AgentBlockProps> & { block: AgentBlockView }) {
  const all: AgentBlockProps = { look: "form", onPick: vi.fn(), ...props };
  act(() => {
    root.render(createElement(AgentBlock, all));
  });
  return all.onPick;
}

const rowOf = (role: string) => host.querySelector<HTMLElement>(`[data-role="${role}"]`);
const valueOf = (role: string) =>
  rowOf(role)?.querySelector<HTMLButtonElement>("button.agent-row-value") ?? null;
const listbox = () => host.querySelector<HTMLElement>('[role="listbox"]');
const chevron = (el: Element | null) =>
  [...(el?.querySelectorAll<HTMLElement>(".icon") ?? [])].some((i) =>
    i.style.maskImage.includes("chevron-down"),
  );

function key(target: Element | null, k: string) {
  act(() => {
    target?.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
  });
}

function press(el: Element | null | undefined) {
  act(() => {
    el?.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
    el?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

describe("an agent row", () => {
  it("locked: no picker, the lock with its tooltip", () => {
    vi.useFakeTimers();
    show({ block: block([LOCKED]) });
    const dev = rowOf("developer");
    expect(valueOf("developer")).toBeNull();
    expect(chevron(dev)).toBe(false);
    expect(dev?.textContent).toContain("Strong");
    expect(dev?.textContent).toContain("claude-opus-5-5 · started");
    const lock = dev?.querySelector<HTMLElement>(".agent-row-lock");
    expect(lock?.getAttribute("aria-label")).toBe("Locked: Send back continues the same conversation");
    act(() => {
      lock?.focus();
    });
    act(() => {
      vi.advanceTimersByTime(0);
    });
    expect(host.querySelector('[role="tooltip"]')?.textContent).toBe(
      "Locked: Send back continues the same conversation",
    );
  });

  it("read-only with one named agent: no chevron, no Change, and the note with its icon", () => {
    show({
      block: block([ONLY, { ...ONLY, role: "validator", label: "Validator" }], {
        note: "Your only agent. Add another in Settings → Agents to choose per ticket.",
        note_icon: "info",
      }),
      look: "panel",
    });
    expect(chevron(host)).toBe(false);
    expect(host.querySelector("button")).toBeNull();
    const note = host.querySelector(".agent-block-note");
    expect(note?.textContent).toBe("Your only agent. Add another in Settings → Agents to choose per ticket.");
    expect(note?.querySelector(".icon")).not.toBeNull();
  });

  it("set by you: neutral, with Let agents pick again, which empties the row", () => {
    const onPick = show({ block: block([SET_BY_YOU]) });
    const who = rowOf("developer")?.querySelector(".agent-row-who");
    expect(who?.textContent).toBe("set by you");
    expect(who?.hasAttribute("data-agent")).toBe(false);
    const again = [...host.querySelectorAll("button")].find((b) => b.textContent === "Let agents pick again");
    press(again);
    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick).toHaveBeenCalledWith("developer", null);
  });

  it("an agent's pick is violet", () => {
    show({
      block: block([
        row({
          value: "Light",
          set_by: { text: "picked by Checkout slowness: footer text only", agent_made: true, icon: "bot" },
        }),
      ]),
    });
    expect(rowOf("developer")?.querySelector(".agent-row-who")?.hasAttribute("data-agent")).toBe(true);
  });

  it("picking writes the row once, and picking the current one writes nothing", () => {
    const onPick = show({ block: block([SET_BY_YOU, VALIDATOR]) });
    press(valueOf("validator"));
    const options = listbox()?.querySelectorAll('[role="option"]');
    expect([...(options ?? [])].map((o) => o.querySelector(".agent-picker-name")?.textContent)).toEqual([
      "Project default (Claude Code)",
      "Claude Code",
      "Strong",
      "Fable test",
    ]);
    press(options?.[2]);
    expect(onPick).toHaveBeenCalledTimes(1);
    expect(onPick).toHaveBeenCalledWith("validator", "strong");
    expect(listbox()).toBeNull();
    press(valueOf("developer"));
    press(listbox()?.querySelectorAll('[role="option"]')[3]);
    expect(onPick).toHaveBeenCalledTimes(1);
  });

  it("the keyboard: ↓ and ↵ pick, Esc closes and gives the field its focus back", () => {
    const onPick = show({ block: block([VALIDATOR]) });
    press(valueOf("validator"));
    expect(document.activeElement).toBe(listbox());
    key(listbox(), "Escape");
    expect(listbox()).toBeNull();
    expect(onPick).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(valueOf("validator"));
    press(valueOf("validator"));
    key(listbox(), "ArrowDown");
    key(listbox(), "ArrowDown");
    key(listbox(), "Enter");
    expect(onPick).toHaveBeenCalledWith("validator", "strong");
  });

  it("focus_agent_row opens that row's picker with the keyboard in it", () => {
    show({ block: block([SET_BY_YOU, VALIDATOR]), look: "panel", focus: "validator" });
    const list = listbox();
    expect(list).not.toBeNull();
    expect(rowOf("validator")?.contains(list ?? null)).toBe(true);
    expect(document.activeElement).toBe(list);
    expect(list?.getAttribute("aria-label")).toBe("Validator");
  });

  it("the panel offers Change; the editor only the chevron", () => {
    show({ block: block([VALIDATOR]), look: "panel" });
    expect(rowOf("validator")?.querySelector(".agent-row-link")?.textContent).toBe("Change");
    show({ block: block([VALIDATOR]), look: "form" });
    expect(rowOf("validator")?.querySelector(".agent-row-link")).toBeNull();
    expect(chevron(valueOf("validator"))).toBe(true);
  });

  it("the moved-on line and Change for the next lead on their own lines", () => {
    show({
      block: block([
        { ...LOCKED, moved_on: "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)" },
        row({ role: "lead", label: "Epic lead", next_lead: "Change for the next lead" }),
      ]),
      look: "panel",
    });
    expect(rowOf("developer")?.querySelector(".agent-row-note")?.textContent).toBe(
      "Sonnet → claude-sonnet-5-6 · new since the last run (claude-sonnet-5-5)",
    );
    const lead = rowOf("lead");
    expect(lead?.querySelector(".agent-row-line .agent-row-link")).toBeNull();
    press(lead?.querySelector(".agent-row-note .agent-row-link"));
    expect(lead?.querySelector('[role="listbox"]')).not.toBeNull();
  });

  it("an epic panel lists its children read-only", () => {
    show({
      block: block([LOCKED], {
        hint: "who leads the epic's children",
        children_heading: "Children",
        children_note:
          "Each child's own Developer and Validator rows live on the child; this list is a summary.",
        children: [
          { key: "SHOP-40.1", number: "40.1", value: "Strong", set_by: null, note: "merged" },
          {
            key: "SHOP-40.2",
            number: "40.2",
            value: "Light",
            set_by: { text: "picked by Wishlist lead", agent_made: true, icon: "bot" },
            note: null,
          },
        ],
      }),
      look: "panel",
    });
    const children = [...host.querySelectorAll(".agent-child")].map((c) => c.textContent);
    expect(children).toEqual(["40.1Strongmerged", "40.2Lightpicked by Wishlist lead"]);
    expect(host.querySelector(".agent-child-who[data-agent]")?.textContent).toBe("picked by Wishlist lead");
  });
});

describe("a pick in the ⌘N form", () => {
  const picked = {
    set_by: { text: "set by you", agent_made: false, icon: "user" as const },
    let_agents_pick: "Let agents pick again",
  };

  it("shows the picked agent set by you until the ticket is created", () => {
    const shown = shownRow(VALIDATOR, "strong", picked);
    expect([shown.value, shown.detail, shown.set_by?.text, shown.let_agents_pick]).toEqual([
      "Strong",
      "Opus · always the newest",
      "set by you",
      "Let agents pick again",
    ]);
    expect(shown.picker?.filter((i) => i.selected).map((i) => i.agent)).toEqual(["strong"]);
  });

  it("no pick, or no choice, shows the row as the view gave it", () => {
    expect(shownRow(VALIDATOR, null, picked)).toBe(VALIDATOR);
    expect(shownRow(ONLY, "cc", picked)).toBe(ONLY);
    expect(shownRow(VALIDATOR, "strong", null)).toBe(VALIDATOR);
  });
});
