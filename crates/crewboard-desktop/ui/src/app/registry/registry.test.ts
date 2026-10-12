import { afterEach, describe, expect, it } from "vitest";

import type { PanelTab } from "../../generated/PanelTab";
import * as screens from "../../screens";
import { sample } from "../../screens/_sample";
import { overlays } from "../overlays";
import { createRegistries, registered, registerScreens } from ".";
import { OVERLAY_IDS, OVERLAY_MODAL, type OverlayId } from "./overlays";
import { PANEL_TABS, panelTabNumbered } from "./panel";
import { createRegistry } from "./registry";
import { createShortcuts } from "./shortcuts";

afterEach(() => {
  overlays.store.set([]);
});

describe("a screen plugs in", () => {
  it("from its own folder plus one line in the screens index, and nothing else changes", () => {
    const before = registered(registerScreens(screens));
    const app = registerScreens({ ...screens, sample });
    const after = registered(app);

    // Exactly the sample's four registrations were added.
    const added = Object.fromEntries(
      Object.entries(after).map(([registry, ids]) => [
        registry,
        ids.filter((id) => !before[registry]?.includes(id)),
      ]),
    );
    expect(added).toEqual({
      slots: [],
      overlays: ["processes"],
      panelTabs: ["history"],
      panelSections: [],
      globalSettings: ["tools"],
      projectSettings: [],
      shortcuts: ["switch_tab"],
    });

    expect(app.panelTabs.get("history")).toBeDefined();
    expect(app.settings.global.get("tools")).toBeDefined();
    expect(app.overlays.get("processes")).toBeDefined();
    expect(app.shortcuts.run({ action: "switch_tab", number: 6 })).toBe(true);
    expect(overlays.store.get()).toEqual([{ id: "processes", params: undefined }]);
    // Not the sample's key: no handler takes it.
    expect(app.shortcuts.run({ action: "switch_tab", number: 2 })).toBe(false);
  });

  it("each id has one owner", () => {
    const app = registerScreens({ sample });
    expect(() => {
      sample(app);
    }).toThrow('panel tabs: "history" is registered twice');
  });
});

describe("a registry", () => {
  it("lists what is registered in its fixed order", () => {
    const r = createRegistry<Record<"a" | "b" | "c", number>>("letters", ["a", "b", "c"]);
    r.register("c", 3);
    r.register("a", 1);
    expect(r.ids()).toEqual(["a", "c"]);
    expect(r.get("a")).toBe(1);
    expect(r.get("b")).toBeUndefined();
  });

  it("starts empty", () => {
    expect(Object.values(registered(createRegistries())).flat()).toEqual([]);
  });
});

describe("the fixed ids", () => {
  it("panel tabs: Terminal · Diff · Run · Shell · Ticket · History, numbered 1–6 (UI-04)", () => {
    expect(PANEL_TABS).toEqual(["terminal", "diff", "run", "shell", "ticket", "history"]);
    expect([1, 2, 6, 7].map(panelTabNumbered)).toEqual(["terminal", "diff", "history", undefined]);
  });

  it("overlays: every id has its modality; the inbox and popovers don't hold shortcuts back", () => {
    expect(OVERLAY_IDS).toHaveLength(20);
    const nonModal = OVERLAY_IDS.filter((id) => !OVERLAY_MODAL[id]);
    expect(nonModal).toEqual(["inbox", "quick_switcher", "processes", "terminals_agents", "limits"]);
  });

  // No `default`: an id added to a union without a case here fails ESLint's switch-exhaustiveness-check.
  it("are exhaustive unions", () => {
    const tabIcon = (tab: PanelTab): string => {
      switch (tab) {
        case "terminal":
        case "shell":
          return "terminal";
        case "diff":
        case "run":
        case "ticket":
        case "history":
          return tab;
      }
    };
    const kind = (id: OverlayId): string => {
      switch (id) {
        case "inbox":
        case "quick_switcher":
        case "limits":
          return "popover";
        case "terminals_agents":
        case "processes":
          return "drawer";
        case "new_ticket":
        case "edit_ticket":
        case "settings":
        case "run_command":
        case "cleanup":
        case "after_restart":
        case "add_project":
        case "delete_ticket":
        case "close_ticket":
        case "send_back":
        case "local_merge":
        case "open_pr":
        case "confirm":
        case "restart_service":
          return "dialog";
        case "gallery":
          return "page";
      }
    };
    expect(PANEL_TABS.map(tabIcon)).toHaveLength(6);
    expect(OVERLAY_IDS.filter((id) => kind(id) === "dialog")).toHaveLength(14);
  });
});

describe("shortcut handlers", () => {
  it("the newest handler that takes the key wins; removing it hands the key back", () => {
    const shortcuts = createShortcuts();
    const calls: string[] = [];
    shortcuts.on("close_panel", () => calls.push("panel"));
    const stop = shortcuts.on("close_panel", () => {
      calls.push("overlay");
      return false;
    });
    expect(shortcuts.run({ action: "close_panel" })).toBe(true);
    expect(calls).toEqual(["overlay", "panel"]);

    const project = shortcuts.on("project", ({ number }) => calls.push(`project ${String(number)}`));
    expect(shortcuts.run({ action: "project", number: 3 })).toBe(true);
    expect(calls.at(-1)).toBe("project 3");

    stop();
    project();
    expect(shortcuts.names()).toEqual(["close_panel"]);
    expect(shortcuts.run({ action: "project", number: 3 })).toBe(false);
  });
});
