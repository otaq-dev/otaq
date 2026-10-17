import { createElement, isValidElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { registered, registerScreens } from "../../app/registry";
import { NOTHING_SELECTED, selection } from "../../app/selection";
import type { HeaderView } from "../../generated/HeaderView";
import type { QuickSwitcherView } from "../../generated/QuickSwitcherView";
import type { RailItem } from "../../generated/RailItem";
import type { RailView } from "../../generated/RailView";
import { HeaderBar } from "./Header";
import { shell, switchToDigit } from ".";
import { LimitsBody } from "./LimitsPopover";
import { clampCursor, switcherKey } from "./logic";
import { QuickList } from "./QuickSwitcher";
import { RailTiles } from "./Rail";

afterEach(() => {
  overlays.store.set([]);
  selection.set(NOTHING_SELECTED);
});

const hint = (text: string, keys: string | null) => ({ text, keys });

const HEADER: HeaderView = {
  project: {
    key: "SHOP",
    name: "shop-web",
    base_branch: "main",
    path: "~/code/shop-web",
    repo_path: "/code/shop-web",
    copy_path: "Copy path",
  },
  search: hint("Search  ·  is:needs-me", "⌘K"),
  activity: {
    agents: { count: "3", label: "running", control: false, active: true },
    processes: "3",
    warning: { text: "5h 18% left", token: "st-paused" },
    hover: {
      title: "3 agents running across all projects",
      lines: ["shop-web 2 · billing-api 1 · 3 processes"],
    },
    label: "Activity",
    keys: "⇧⌘U",
  },
  inbox: { label: "Needs you", count: 7, active: true },
  inbox_hint: hint("Needs you", "⇧⌘I"),
};

const item = (key: string, name: string, extra: Partial<RailItem> = {}): RailItem => ({
  key,
  name,
  repo_path: `/code/${name}`,
  title: `${name} · ${key}`,
  path: `~/code/${name}`,
  agents_working: 0,
  needs_you: 0,
  selected: false,
  idle: true,
  ringing: false,
  shortcut: null,
  detail: ["idle"],
  note: null,
  ...extra,
});

const RAIL: RailView = {
  selected: "SHOP",
  items: [
    item("SHOP", "shop-web", {
      selected: true,
      idle: false,
      needs_you: 6,
      agents_working: 2,
      shortcut: "⌘1",
    }),
    item("BILL", "billing-api", { idle: false, needs_you: 1, ringing: true }),
    item("CREW", "otaq"),
  ],
  width: 64,
  ring_ends_in_ms: 4000,
  switch_keys: "⌘P",
  bottom: { terminals: { label: "Terminals", keys: "⇧⌘T" }, settings: { label: "Settings", keys: "⌘," } },
};

describe("the quick switcher's keys", () => {
  it("move with wrapping, switch on ↵, close on Esc, and leave typing to the field", () => {
    expect(switcherKey("ArrowDown", 2, 3)).toEqual({ kind: "move", index: 0 });
    expect(switcherKey("ArrowUp", 0, 3)).toEqual({ kind: "move", index: 2 });
    expect(switcherKey("ArrowDown", 0, 0)).toEqual({ kind: "move", index: 0 });
    expect(switcherKey("Enter", 1, 3)).toEqual({ kind: "choose", index: 1 });
    expect(switcherKey("Enter", 5, 2)).toEqual({ kind: "choose", index: 1 });
    expect(switcherKey("Enter", 0, 0)).toEqual({ kind: "pass" });
    expect(switcherKey("Escape", 0, 3)).toEqual({ kind: "close" });
    expect(switcherKey("b", 0, 3)).toEqual({ kind: "pass" });
  });

  it("keep the cursor on a row when the list shrinks", () => {
    expect(clampCursor(4, 2)).toBe(1);
    expect(clampCursor(3, 0)).toBe(0);
    expect(clampCursor(1, 5)).toBe(1);
  });
});

describe("the rail", () => {
  it("draws the badges, the selection, a ringing tile and quiet idle projects", () => {
    const out = renderToStaticMarkup(createElement(RailTiles, { view: RAIL }));
    expect(out).toContain('data-selected="true"');
    expect(out).toContain('<span class="rail-needs">6</span>');
    expect(out).toContain('<span class="rail-tile" data-ringing="true">BILL');
    expect(out).toMatch(/data-idle="true"[^>]*aria-label="otaq · CREW"/);
    expect(out).toContain('aria-label="Add project"');
  });
});

describe("the header", () => {
  it("draws what the view says, left to right", () => {
    const out = renderToStaticMarkup(createElement(HeaderBar, { view: HEADER, onSearch: () => undefined }));
    const order = [
      "shop-web",
      "~/code/shop-web",
      "Search  ·  is:needs-me",
      "running",
      "▷ 3",
      "5h 18% left",
      "Needs you",
    ].map((text) => out.indexOf(text));
    expect(order.every((at) => at >= 0)).toBe(true);
    expect(order).toEqual(order.toSorted((a, b) => a - b));
    expect(out).toContain('<span class="header-mono" data-amber="true">5h 18% left</span>');
    expect(out).toContain('class="header-inbox" data-active="true"');
    expect(out).not.toContain("chevron-down");
  });

  it("each button opens its overlay by id", () => {
    // The elements the header returns, walked without a DOM: every element with a click handler, by what it says.
    const clicks = new Map<string, () => void>();
    const walk = (node: ReactNode, name: string) => {
      if (Array.isArray(node)) {
        for (const n of node as ReactNode[]) walk(n, name);
        return;
      }
      if (!isValidElement(node)) return;
      const props = node.props as {
        className?: string;
        label?: string;
        onClick?: () => void;
        children?: ReactNode;
      };
      const here = props.label ?? props.className ?? name;
      if (props.onClick) clicks.set(here, props.onClick);
      walk(props.children, here);
    };
    let searched = 0;
    walk(
      HeaderBar({
        view: {
          ...HEADER,
          activity: { ...HEADER.activity, agents: { ...HEADER.activity.agents, control: true } },
        },
        onSearch: () => (searched += 1),
      }),
      "",
    );
    const press = (name: string) => {
      const click = clicks.get(name);
      expect(click, name).toBeDefined();
      click?.();
    };
    press("header-search");
    expect(searched).toBe(1);
    for (const name of ["header-chip header-agents", "header-inbox"]) {
      press(name);
    }
    expect(overlays.store.get().map((o) => o.id)).toEqual(["limits", "inbox"]);
  });

  it("hides what has no data, and reads slots with limits on", () => {
    const quiet: HeaderView = {
      ...HEADER,
      project: null,
      activity: {
        ...HEADER.activity,
        agents: { count: "0 / 5", label: "agents", control: true, active: false },
        processes: null,
        warning: null,
      },
      inbox: { label: "Inbox", count: 0, active: false },
    };
    const out = renderToStaticMarkup(createElement(HeaderBar, { view: quiet, onSearch: () => undefined }));
    expect(out).not.toContain("header-search");
    expect(out).not.toContain("▷");
    expect(out).not.toContain("data-amber");
    expect(out).toContain("0 / 5");
    expect(out).toContain("<span>Inbox</span>");
  });

  it("the limits popover disables a stepper at its bound", () => {
    const out = renderToStaticMarkup(
      createElement(LimitsBody, {
        view: {
          title: "Agent limits",
          max_agents: { label: "Max agents running at once", value: 1, can_lower: false, can_raise: true },
          back_pressure: {
            label: "Pause new starts when the inbox has",
            value: 3,
            can_lower: true,
            can_raise: true,
          },
          note: "Queued tickets wait for a free slot.",
        },
        onStep: () => undefined,
      }),
    );
    expect(out).toContain('aria-label="Fewer" disabled=""');
    expect(out.match(/disabled=""/g)).toHaveLength(1);
  });
});

describe("the quick switcher's list", () => {
  it("highlights the cursor's row and shows the footer hints", () => {
    const view: QuickSwitcherView = {
      placeholder: "Switch project…",
      rows: [
        { project: RAIL.items[0] as RailItem, label: "shop-web · SHOP · current", running: "2 running" },
        { project: RAIL.items[2] as RailItem, label: "otaq · CREW", running: null },
      ],
      hints: [
        { keys: "↑↓", label: "move" },
        { keys: "↵", label: "switch" },
        { keys: "⌘N", label: "add project" },
      ],
    };
    const out = renderToStaticMarkup(
      createElement(QuickList, { view, cursor: 1, onChoose: () => undefined }),
    );
    expect(out).toMatch(/aria-selected="false"[^>]*>.*shop-web · SHOP · current/);
    expect(out).toMatch(/aria-selected="true" data-idle="true"/);
    expect(out).toContain("2 running");
    expect(out).toContain("add project");
  });
});

describe("the shell screen", () => {
  it("fills the header and the rail and owns ⌘P, ⌘1–9 and the limits popover", () => {
    const app = registerScreens({ shell });
    expect(registered(app)).toMatchObject({
      slots: ["header", "rail"],
      overlays: ["quick_switcher", "limits"],
      shortcuts: ["project", "switch_project"],
    });
    expect(app.shortcuts.run({ action: "switch_project" })).toBe(true);
    expect(overlays.store.get()).toEqual([{ id: "quick_switcher", params: undefined }]);
    app.shortcuts.run({ action: "switch_project" });
    expect(overlays.store.get()).toEqual([]);
  });

  it("⌘1–9 switch to the rail's project at that place, when there is one", async () => {
    const asked: unknown[] = [];
    const ask = (method: string, params: unknown) => {
      asked.push([method, params]);
      return Promise.resolve((params as { digit: number }).digit === 2 ? "BILL" : null);
    };
    expect(await switchToDigit(2, ask)).toBe("BILL");
    expect(selection.get().project).toBe("BILL");
    expect(await switchToDigit(7, ask)).toBe(null);
    expect(selection.get().project).toBe("BILL");
    expect(asked).toEqual([
      ["shell.project_for_digit", { digit: 2 }],
      ["shell.project_for_digit", { digit: 7 }],
    ]);
  });
});
