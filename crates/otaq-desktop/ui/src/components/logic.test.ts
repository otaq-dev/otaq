import { describe, expect, it } from "vitest";

import type { MenuItem } from "../generated/MenuItem";
import {
  acceptsPress,
  chooseMenu,
  defaultOutcome,
  dialogKey,
  EDGE,
  FLOAT_GAP,
  menuKey,
  placeFloating,
  stepMenu,
  trapFocus,
} from "./logic";
import { arrowIndex } from "./Tabs";

const item = (id: string, change: Partial<MenuItem> = {}): MenuItem => ({
  id,
  label: id,
  icon: null,
  shortcut: null,
  danger: false,
  disabled: false,
  hint: null,
  separated: false,
  checked: false,
  header: false,
  ...change,
});

// The same menu as `menu.rs`'s tests, so both sides keep one rule.
const menu = [
  item("open", { shortcut: "↵" }),
  item("pin", { separated: true }),
  item("close", { danger: true }),
  item("delete", { danger: true, disabled: true, hint: "Started tickets can only be closed." }),
];

describe("menu keyboard (menu.rs, MenuModel)", () => {
  it("arrows skip disabled rows and wrap; up from nothing starts at the bottom", () => {
    let at = stepMenu(menu, null, 1);
    expect(at).toBe(0);
    at = stepMenu(menu, stepMenu(menu, at, 1), 1);
    expect(at).toBe(2);
    expect(stepMenu(menu, at, 1)).toBe(0);
    expect(stepMenu(menu, 0, -1)).toBe(2);
    expect(stepMenu(menu, null, -1)).toBe(2);
  });

  it("↵ and clicks choose enabled rows only", () => {
    expect(chooseMenu(menu, null)).toBeNull();
    expect(chooseMenu(menu, 0)).toBe("open");
    expect(chooseMenu(menu, 1)).toBe("pin");
    expect(chooseMenu(menu, 3)).toBeNull();
  });

  it("skips headers; an empty or all-disabled menu highlights nothing", () => {
    const list = [
      item("", { header: true, label: "LOCAL" }),
      item("main"),
      item("", { header: true }),
      item("next"),
    ];
    expect(stepMenu(list, null, 1)).toBe(1);
    expect(stepMenu(list, 1, 1)).toBe(3);
    expect(chooseMenu(list, 0)).toBeNull();
    expect(stepMenu([item("x", { disabled: true })], null, 1)).toBeNull();
    expect(stepMenu([], null, -1)).toBeNull();
  });

  it("maps keys: arrows, Home/End, ↵ and Space choose, Esc and Tab close", () => {
    expect(menuKey(menu, null, "ArrowDown")).toEqual({ kind: "highlight", index: 0 });
    expect(menuKey(menu, 0, "ArrowUp")).toEqual({ kind: "highlight", index: 2 });
    expect(menuKey(menu, 2, "Home")).toEqual({ kind: "highlight", index: 0 });
    expect(menuKey(menu, 0, "End")).toEqual({ kind: "highlight", index: 2 });
    expect(menuKey(menu, 2, "Enter")).toEqual({ kind: "choose", id: "close" });
    expect(menuKey(menu, 1, " ")).toEqual({ kind: "choose", id: "pin" });
    expect(menuKey(menu, null, "Enter")).toEqual({ kind: "none" });
    expect(menuKey(menu, 0, "Escape")).toEqual({ kind: "close" });
    expect(menuKey(menu, 0, "Tab")).toEqual({ kind: "close" });
    expect(menuKey(menu, 0, "a")).toEqual({ kind: "none" });
  });
});

describe("dialogs (dialog.rs)", () => {
  it("Esc cancels; ↵ presses the default: confirm, but Cancel on a destructive dialog", () => {
    expect(dialogKey(false, "Escape")).toBe("cancel");
    expect(dialogKey(true, "Escape")).toBe("cancel");
    expect(dialogKey(false, "Enter")).toBe("confirm");
    expect(dialogKey(true, "Enter")).toBe("cancel");
    expect(dialogKey(false, "a")).toBeNull();
    expect(defaultOutcome(true)).toBe("cancel");
    expect(defaultOutcome(false)).toBe("confirm");
  });

  it("Tab and ⇧Tab cycle inside the dialog", () => {
    expect(trapFocus(3, 0, false)).toBe(1);
    expect(trapFocus(3, 2, false)).toBe(0);
    expect(trapFocus(3, 0, true)).toBe(2);
    expect(trapFocus(3, -1, false)).toBe(0);
    expect(trapFocus(3, -1, true)).toBe(2);
    expect(trapFocus(0, -1, false)).toBe(-1);
  });
});

describe("floating layers", () => {
  const viewport = { width: 1280, height: 800 };
  const size = { width: 260, height: 200 };

  it("go under the anchor, left edges aligned, or right edges with `end`", () => {
    const anchor = { left: 100, top: 100, width: 80, height: 28 };
    expect(placeFloating(anchor, size, viewport)).toEqual({ left: 100, top: 128 + FLOAT_GAP });
    expect(placeFloating(anchor, size, viewport, "end")).toEqual({ left: EDGE, top: 128 + FLOAT_GAP });
  });

  it("flip above when there is no room below, and stay inside the window", () => {
    const low = { left: 1200, top: 700, width: 60, height: 28 };
    expect(placeFloating(low, size, viewport)).toEqual({
      left: 1280 - EDGE - 260,
      top: 700 - FLOAT_GAP - 200,
    });
    const tall = { width: 260, height: 900 };
    expect(placeFloating({ left: 0, top: 10, width: 10, height: 10 }, tall, viewport).top).toBe(EDGE);
  });
});

describe("buttons and tabs", () => {
  it("a press reaches the action only when neither disabled nor loading", () => {
    expect(acceptsPress(false, false)).toBe(true);
    expect(acceptsPress(true, false)).toBe(false);
    expect(acceptsPress(false, true)).toBe(false);
  });

  it("←/→ move between tabs and segments, wrapping", () => {
    expect(arrowIndex("ArrowRight", 5, 6)).toBe(0);
    expect(arrowIndex("ArrowLeft", 0, 6)).toBe(5);
    expect(arrowIndex("ArrowDown", 0, 6)).toBeNull();
    expect(arrowIndex("ArrowRight", 0, 0)).toBeNull();
  });
});
