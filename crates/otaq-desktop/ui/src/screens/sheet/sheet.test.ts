// @vitest-environment happy-dom
// The ticket sheet (SHT-01, SHT-02; T-0464): drawn to a string, and its keys mounted.
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

import { shellLayout, INITIAL_LAYOUT } from "../../app/layout";
import { createRegistries, registered, registerScreens, RegistriesContext } from "../../app/registry";
import { NOTHING_SELECTED, openTicket, selection } from "../../app/selection";
import type { PanelView } from "../../generated/PanelView";
import { sheet } from ".";
import { contentOf, numberedSheetTab, sheetWidth, shownSheetTab, steppedSheetTab } from "./logic";
import { useSheetKeys } from "./Sheet";
import { renderSheet as render, SHEET, VIEW } from "./sheet_fixtures";

vi.mock("../../tauri", () => ({
  request: vi.fn(() => new Promise(() => undefined)),
  subscribeEvents: vi.fn(),
}));

afterEach(() => {
  selection.set(NOTHING_SELECTED);
  shellLayout.set(INITIAL_LAYOUT);
});

const TABS = SHEET.tabs;

describe("the sheet's tabs (SHT-02)", () => {
  it("opens on Overview; a selected panel tab shows the sheet tab holding it", () => {
    expect(shownSheetTab(TABS, null, "overview")?.tab).toBe("overview");
    expect(shownSheetTab(TABS, "run", "overview")?.tab).toBe("session");
    expect(shownSheetTab(TABS, "diff", "overview")?.tab).toBe("files");
    expect(shownSheetTab(TABS, "history", "overview")?.tab).toBe("activity");
    // An epic has no Session: its Terminal asked for falls back to the tab it opens on.
    const epic = TABS.filter((t) => t.tab !== "session");
    expect(shownSheetTab(epic, "terminal", "overview")?.tab).toBe("overview");
  });

  it("Session shows what is picked in it, Agent first", () => {
    const session = TABS[1];
    if (!session) throw new Error("no Session");
    expect(contentOf(session, null)).toBe("terminal");
    expect(contentOf(session, "shell")).toBe("shell");
    expect(contentOf(session, "diff")).toBe("terminal");
  });

  it("1–4 (g o / s / f / a) pick by number; ⌃Tab steps round the ends", () => {
    expect(numberedSheetTab(TABS, 4)?.tab).toBe("activity");
    expect(numberedSheetTab(TABS, 5)).toBeUndefined();
    expect(steppedSheetTab(TABS, TABS[3], 1)?.tab).toBe("overview");
    expect(steppedSheetTab(TABS, TABS[0], -1)?.tab).toBe("activity");
    expect(steppedSheetTab([], undefined, 1)).toBeUndefined();
  });

  it("is 1040 px, 920 px beside the Project agent, and never wider than the main area less its margins", () => {
    expect(sheetWidth(SHEET, false, 1376)).toBe(1040);
    expect(sheetWidth(SHEET, true, 976)).toBe(920);
    expect(sheetWidth(SHEET, true, 896)).toBe(860);
  });
});

describe("the sheet", () => {
  it("draws the breadcrumb, title, status, Move…, the details line, the path and four tabs", () => {
    const html = render(VIEW);
    for (const text of [
      "shop-web",
      "SHOP-19",
      "Price table",
      "Working",
      "Move…",
      "Assignee",
      "Rust developer",
      "Claude Code · sonnet",
      "3 min · 182k tokens",
      "otaq/SHOP-19",
      "from main",
      "Grill",
    ]) {
      expect(html).toContain(text);
    }
    expect(html.match(/role="tab"/g)).toHaveLength(4);
    expect(html).toContain('data-tab="overview"');
    expect(html).toContain('aria-current="step"');
    expect(html).toContain('aria-label="Priority: P3"');
    expect(html).toContain('aria-label="Full page"');
    expect(html).toContain("width:1040px");
  });

  it("shows the selected tab's body: Session's switch over Agent, Activity's History; Files empty until its screen", () => {
    const session = render(VIEW, { tab: "run" });
    expect(session).toContain('data-tab="session"');
    expect(session.match(/role="radio"/g)).toHaveLength(3);
    expect(session).toContain('aria-checked="true">Run');
    expect(render(VIEW, { tab: "history" })).toContain("history of SHOP-19");
    expect(render(VIEW, { tab: "diff" })).toContain('<div class="sheet-body" data-tab="files"></div>');
  });

  it("at full page: ← Board in place of the project, no fixed width, and the button goes back", () => {
    const html = render(VIEW, { fullPage: true });
    expect(html).toContain("data-full-page");
    expect(html).toContain("Board");
    expect(html).not.toContain("shop-web");
    expect(html).toContain('aria-label="Back to the sheet"');
    expect(html).not.toContain("width:1040px");
  });

  it("an epic child's crumb opens the epic", () => {
    const html = render({
      ...VIEW,
      sheet: {
        ...SHEET,
        crumbs: [
          { text: "shop-web", opens: null, key: false },
          { text: "Wishlist", opens: "SHOP-40", key: false },
          { text: "40.2", opens: null, key: true },
        ],
      },
    });
    expect(html).toContain('<button type="button" class="sheet-crumb sheet-crumb-link">Wishlist</button>');
  });

  it("fills the panel slot and registers the tab bodies", () => {
    const app = registerScreens({ sheet });
    expect(registered(app)).toMatchObject({
      slots: ["panel"],
      sheetTabs: ["session", "files", "activity", "lead"],
    });
  });
});

describe("the sheet's keys (0097, SHT-02)", () => {
  function mount(view: PanelView) {
    const app = createRegistries();
    const host = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    function Probe() {
      useSheetKeys(view);
      return null;
    }
    act(() => {
      root.render(createElement(RegistriesContext, { value: app }, createElement(Probe)));
    });
    return {
      app,
      done: () => {
        act(() => {
          root.unmount();
        });
        host.remove();
      },
    };
  }

  it("tab keys, ⌃Tab, j / k keeping the tab, e and Esc", () => {
    openTicket("SHOP-19");
    const { app, done } = mount(VIEW);
    expect(app.shortcuts.run({ action: "switch_tab", number: 2 })).toBe(true);
    expect(selection.get().tab).toBe("terminal");
    expect(app.shortcuts.run({ action: "switch_tab", number: 4 })).toBe(true);
    expect(selection.get().tab).toBe("history");
    expect(app.shortcuts.run({ action: "switch_tab", number: 5 })).toBe(false);
    app.shortcuts.run({ action: "next_tab" });
    expect(selection.get().tab).toBe("ticket");
    app.shortcuts.run({ action: "next_ticket" });
    expect(selection.get()).toMatchObject({ ticket: "SHOP-20", tab: "ticket" });
    expect(app.shortcuts.run({ action: "open_diff" })).toBe(true);
    expect(selection.get().tab).toBe("diff");
    app.shortcuts.run({ action: "expand_panel" });
    expect(shellLayout.get().panelExpanded).toBe(true);
    app.shortcuts.run({ action: "close_panel" });
    expect(selection.get().ticket).toBeNull();
    done();
  });

  it("Esc in the Project agent's panel is left to the panel; the sheet stays open", () => {
    openTicket("SHOP-19");
    const { app, done } = mount(VIEW);
    const pane = document.body.appendChild(document.createElement("div"));
    pane.dataset.slot = "pane";
    const input = pane.appendChild(document.createElement("textarea"));
    input.focus();
    expect(app.shortcuts.run({ action: "close_panel" })).toBe(false);
    expect(selection.get().ticket).toBe("SHOP-19");
    input.blur();
    pane.remove();
    expect(app.shortcuts.run({ action: "close_panel" })).toBe(true);
    expect(selection.get().ticket).toBeNull();
    done();
  });

  it("g a is Activity's popover while no sheet takes it", () => {
    const { app, done } = mount({ ...VIEW, sheet: { ...SHEET, tabs: [] } });
    expect(app.shortcuts.run({ action: "switch_tab", number: 4 })).toBe(false);
    done();
  });
});
