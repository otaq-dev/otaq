import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { createRegistries, registered, registerScreens, RegistriesContext } from "../../app/registry";
import { NOTHING_SELECTED, selection } from "../../app/selection";
import type { DrawerTab } from "../../generated/DrawerTab";
import type { DrawerView } from "../../generated/DrawerView";
import { TerminalsButtonView } from "./Button";
import { DrawerBody } from "./Drawer";
import { terminals } from ".";
import { requestTab, shownTab, showsHint, tabRequests } from "./logic";

afterEach(() => {
  overlays.store.set([]);
  selection.set(NOTHING_SELECTED);
  tabRequests.set(null);
});

const tab = (id: string, label: string, more: Partial<DrawerTab> = {}): DrawerTab => ({
  id,
  kind: id.startsWith("agent:") ? "agent" : "terminal",
  label,
  note: null,
  icon: id.startsWith("agent:") ? "bot" : "square-terminal",
  dot: null,
  muted: false,
  session: `s-${id}`,
  ended: false,
  ended_line: null,
  ...more,
});

const TABS: DrawerTab[] = [
  tab("agent:a1", "Checkout slowness", { dot: "working" }),
  tab("agent:a2", "Import GitHub issues", { dot: "waiting" }),
  tab("agent:a3", "New agent", { muted: true }),
  tab("terminal:t1", "Terminal 1"),
  tab("terminal:t2", "Terminal 2", {
    note: " · exited",
    ended: true,
    ended_line: "── shell exited 11:20 · scrollback kept · close the tab, or + → New terminal ──",
  }),
];

const item = (id: string, label: string, shortcut: string) => ({
  id,
  label,
  icon: null,
  shortcut,
  danger: false,
  disabled: false,
  hint: null,
  separated: false,
  checked: false,
  header: false,
});

const VIEW: DrawerView = {
  title: "Terminals & agents",
  tabs: TABS,
  selected: "terminal:t1",
  new_label: "New",
  new_menu: [
    item("new-terminal", "New terminal", "plain shell"),
    item("new-agent", "New Project agent", "⇧⌘A · uses tokens"),
  ],
  close_tab: "Close tab",
  hide: "Hide · terminals and agents keep running",
  maximize: "Maximize",
  restore: "Restore",
  hint: ["Project terminal · shop-web — a shell in ~/code/shop-web"],
  empty: ["Nothing open. + → New terminal opens a fresh shell in ~/code/shop-web."],
  height: 380,
};

function render(view: DrawerView): string {
  return renderToStaticMarkup(
    createElement(
      RegistriesContext,
      { value: createRegistries() },
      createElement(DrawerBody, {
        view,
        project: "SHOP",
        maximized: false,
        onMaximize: () => undefined,
        onHide: () => undefined,
      }),
    ),
  );
}

describe("which tab shows", () => {
  it("the picked one while open, else Rust's choice, else the first", () => {
    expect(shownTab(TABS, "agent:a2", "terminal:t1")?.label).toBe("Import GitHub issues");
    expect(shownTab(TABS, "terminal:gone", "terminal:t1")?.label).toBe("Terminal 1");
    expect(shownTab(TABS, null, null)?.label).toBe("Checkout slowness");
    expect(shownTab([], null, null)).toBeUndefined();
  });

  it("a shortcut's request counts up", () => {
    requestTab("terminal:t1");
    requestTab("terminal:t1");
    expect(tabRequests.get()).toEqual({ tab: "terminal:t1", seq: 2 });
  });

  it("the hint shows in a live terminal until its first keystroke", () => {
    const t1 = TABS[3] as DrawerTab;
    expect(showsHint(t1, new Set(), true)).toBe(true);
    expect(showsHint(t1, new Set(["terminal:t1"]), true)).toBe(false);
    expect(showsHint(t1, new Set(), false)).toBe(false);
    expect(showsHint(TABS[0] as DrawerTab, new Set(), true)).toBe(false);
    expect(showsHint(TABS[4] as DrawerTab, new Set(), true)).toBe(false);
  });
});

describe("the drawer", () => {
  it("draws a tab per entry and + New, no Recently ended (agents are the pane's); the shown tab has its close button", () => {
    const html = render(VIEW);
    expect(html.match(/role="tab"/g)).toHaveLength(5);
    for (const text of [
      "Checkout slowness",
      "Import GitHub issues",
      "New agent",
      "Terminal 1",
      " · exited",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain('data-dot="waiting"');
    expect(html).toContain("data-muted");
    expect(html.match(/aria-label="Close tab"/g)).toHaveLength(1);
    expect(html).toContain('aria-label="New"');
    expect(html).not.toContain("Recently ended");
    expect(html).toContain("height:380px");
  });

  it("an ended terminal keeps its scrollback with the exit line", () => {
    const html = render({ ...VIEW, selected: "terminal:t2" });
    expect(html).toContain("shell exited 11:20");
  });

  it("with nothing open it says how to open a terminal", () => {
    const html = render({ ...VIEW, tabs: [], selected: null });
    expect(html).toContain("Nothing open.");
    expect(html).not.toContain('role="tab"');
  });
});

describe("the header button", () => {
  it("shows the dot only while a Project agent waits", () => {
    const view = {
      label: "Terminals & agents",
      keys: "⇧⌘T",
      dot: false,
      waiting: null,
      note: "New Project agent  ⇧⌘A",
    };
    const calm = renderToStaticMarkup(createElement(TerminalsButtonView, { view, open: false }));
    expect(calm).toContain('aria-label="Terminals &amp; agents"');
    expect(calm).not.toContain("data-dot");
    const waiting = renderToStaticMarkup(
      createElement(TerminalsButtonView, {
        view: { ...view, dot: true, waiting: "Project agent “Import GitHub issues” asks you" },
        open: false,
      }),
    );
    expect(waiting).toContain("data-dot");
  });

  it("registers the drawer and ⇧⌘T from its folder (⇧⌘A is the pane's)", () => {
    const app = registerScreens({ terminals });
    expect(registered(app)).toMatchObject({ overlays: ["terminals_agents"] });
    expect(app.shortcuts.names()).toEqual(["project_terminal"]);
    // No project shown: the key passes.
    expect(app.shortcuts.run({ action: "project_terminal" })).toBe(false);
  });
});
