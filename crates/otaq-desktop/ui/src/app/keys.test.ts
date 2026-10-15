import { describe, expect, it } from "vitest";

import type { KeyAction } from "../generated/KeyAction";
import type { KeyBinding } from "../generated/KeyBinding";
import type { Keymap } from "../generated/Keymap";
import type { KeyModifiers } from "../generated/KeyModifiers";
import {
  type Dispatch,
  dispatchKey,
  type KeyContext,
  type KeyLike,
  inEditorText,
  keysKeptBy,
  logLine,
  stopsEvent,
} from "./keys";

// A few rows of the map as `keymap.map` sends them (the Rust tests cover the whole table).
const NONE: KeyModifiers = { control: false, alt: false, shift: false, platform: false, function: false };
const CMD = { ...NONE, platform: true };
const CTRL = { ...NONE, control: true };
const CTRL_SHIFT = { ...NONE, control: true, shift: true };

function bind(
  action: KeyAction,
  text: string,
  key: string,
  code: string,
  modifiers: KeyModifiers,
  inTerminal = false,
) {
  return { action, text, keystroke: { key, code, modifiers }, in_terminal: inTerminal } satisfies KeyBinding;
}

const MAC: Keymap = {
  platform: "mac",
  log: false,
  terminal_keys: [],
  terminal_app_modifiers: [CMD],
  bindings: [
    bind({ action: "search" }, "⌘K", "k", "KeyK", CMD),
    bind({ action: "switch_project" }, "⌘P", "p", "KeyP", CMD),
    bind({ action: "open_inbox" }, "⇧⌘I", "i", "KeyI", { ...CMD, shift: true }),
    bind({ action: "leave_terminal" }, "⇧⌘`", "`", "Backquote", { ...CMD, shift: true }),
    bind({ action: "switch_tab", number: 1 }, "⌘⌥1", "1", "Digit1", { ...CMD, alt: true }),
    bind({ action: "open_panel" }, "↵", "Enter", "Enter", NONE),
    bind({ action: "close_panel" }, "Esc", "Escape", "Escape", NONE),
    bind({ action: "send_back" }, "⌘⌫", "Backspace", "Backspace", CMD),
    bind({ action: "find_in_terminal" }, "⌘F", "f", "KeyF", CMD),
  ],
};

const WIN: Keymap = {
  platform: "win_linux",
  log: false,
  terminal_keys: [],
  terminal_app_modifiers: [CTRL_SHIFT, { ...NONE, platform: true }],
  bindings: [
    bind({ action: "search" }, "Ctrl+K", "k", "KeyK", CTRL),
    bind({ action: "switch_project" }, "Ctrl+P", "p", "KeyP", CTRL),
    bind({ action: "leave_terminal" }, "Ctrl+Shift+`", "`", "Backquote", CTRL_SHIFT),
    bind({ action: "search" }, "Ctrl+Shift+K", "k", "KeyK", CTRL_SHIFT, true),
    bind({ action: "leave_terminal" }, "Ctrl+Shift+`", "`", "Backquote", CTRL_SHIFT, true),
    bind({ action: "find_in_terminal" }, "Ctrl+Shift+F", "f", "KeyF", CTRL_SHIFT, true),
  ],
};

function key(name: string, code: string, mods: Partial<Omit<KeyLike, "key" | "code">> = {}): KeyLike {
  return { key: name, code, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods };
}

const BOARD: KeyContext = { inTerminal: false, focusKeeps: new Set(), modal: false, editorText: false };
const TERMINAL: KeyContext = { ...BOARD, inTerminal: true };

/** A dispatcher whose screens handle `handled`; records what ran and whether the terminal was left. */
function setup(map: Keymap | undefined, handled: KeyAction["action"][] = []) {
  const ran: string[] = [];
  let left = 0;
  const d: Dispatch = {
    keymap: () => map,
    run: (action) => {
      if (!handled.includes(action.action)) return false;
      ran.push(action.action);
      return true;
    },
    leaveTerminal: () => left++,
    mac: map?.platform === "mac",
  };
  return { d, ran, left: () => left };
}

describe("the key dispatcher on the board", () => {
  it("runs the handler of the bound action and stops the key", () => {
    const { d, ran } = setup(MAC, ["search"]);
    const outcome = dispatchKey(d, key("k", "KeyK", { metaKey: true }), BOARD);
    expect(outcome).toEqual({ kind: "ran", action: { action: "search" } });
    expect(stopsEvent(outcome)).toBe(true);
    expect(ran).toEqual(["search"]);
  });

  it("matches letters by layout and digits by physical key", () => {
    const { d, ran } = setup(MAC, ["search", "switch_tab"]);
    // Dvorak: the key that types k is where QWERTY has V.
    dispatchKey(d, key("k", "KeyV", { metaKey: true }), BOARD);
    // ⌘⌥1 types ¡.
    const tab = dispatchKey(d, key("¡", "Digit1", { metaKey: true, altKey: true }), BOARD);
    expect(tab).toEqual({ kind: "ran", action: { action: "switch_tab", number: 1 } });
    // A Cyrillic layout types л on K's key.
    dispatchKey(d, key("л", "KeyK", { metaKey: true }), BOARD);
    expect(ran).toEqual(["search", "switch_tab", "search"]);
  });

  it("needs the exact modifiers", () => {
    const { d, ran } = setup(MAC, ["search", "open_inbox"]);
    expect(dispatchKey(d, key("K", "KeyK", { metaKey: true, shiftKey: true }), BOARD).kind).toBe("passed");
    expect(dispatchKey(d, key("I", "KeyI", { metaKey: true, shiftKey: true }), BOARD).kind).toBe("ran");
    expect(dispatchKey(d, key("k", "KeyK", { ctrlKey: true }), BOARD).kind).toBe("passed");
    expect(ran).toEqual(["open_inbox"]);
  });

  it("swallows a bound key nobody handles, so ⌘P never prints", () => {
    const { d } = setup(MAC);
    const outcome = dispatchKey(d, key("p", "KeyP", { metaKey: true }), BOARD);
    expect(outcome).toEqual({ kind: "unhandled", action: { action: "switch_project" }, swallowed: true });
    expect(stopsEvent(outcome)).toBe(true);
  });

  it("swallows the webview's own keys without acting", () => {
    for (const [map, mods] of [
      [MAC, { metaKey: true }],
      [WIN, { ctrlKey: true }],
    ] as const) {
      const { d, ran } = setup(map);
      for (const k of [
        key("F5", "F5"),
        key("F7", "F7"),
        key("F12", "F12"),
        key("r", "KeyR", mods),
        key("R", "KeyR", { ...mods, shiftKey: true }),
        key("=", "Equal", mods),
        key("-", "Minus", mods),
        key("0", "Digit0", mods),
        key("f", "KeyF", mods),
        key("g", "KeyG", mods),
        key("s", "KeyS", mods),
        key("J", "KeyJ", { ...mods, shiftKey: true }),
      ]) {
        const outcome = dispatchKey(d, k, BOARD);
        expect(stopsEvent(outcome), `${map.platform} ${logLine(k, outcome)}`).toBe(true);
      }
      expect(ran).toEqual([]);
    }
    const win = setup(WIN).d;
    expect(dispatchKey(win, key("ArrowLeft", "ArrowLeft", { altKey: true }), BOARD).kind).toBe("swallowed");
    // On a Mac ⌥← moves by word.
    const mac = setup(MAC).d;
    expect(dispatchKey(mac, key("ArrowLeft", "ArrowLeft", { altKey: true }), BOARD).kind).toBe("passed");
  });

  it("leaves typing and the Edit keys to the page", () => {
    const { d } = setup(MAC);
    for (const k of [
      key("a", "KeyA"),
      key("c", "KeyC", { metaKey: true }),
      key("v", "KeyV", { metaKey: true }),
      key("z", "KeyZ", { metaKey: true }),
      key("Tab", "Tab"),
      key("ArrowDown", "ArrowDown"),
    ]) {
      expect(dispatchKey(d, k, BOARD).kind).toBe("passed");
    }
  });

  it("lets a text field keep Enter, Backspace and Esc", () => {
    const { d, ran } = setup(MAC, ["open_panel", "close_panel", "send_back", "search"]);
    const field = {
      ...BOARD,
      focusKeeps: keysKeptBy({ tagName: "INPUT", isContentEditable: false, getAttribute: () => null }),
    };
    expect(dispatchKey(d, key("Enter", "Enter"), field).kind).toBe("passed");
    expect(dispatchKey(d, key("Escape", "Escape"), field).kind).toBe("passed");
    expect(dispatchKey(d, key("Backspace", "Backspace", { metaKey: true }), field).kind).toBe("passed");
    expect(dispatchKey(d, key("k", "KeyK", { metaKey: true }), field).kind).toBe("ran");
    expect(dispatchKey(d, key("Enter", "Enter"), BOARD).kind).toBe("ran");
    expect(ran).toEqual(["search", "open_panel"]);
  });

  it("leaves ⌘K / Ctrl+K to the description editor while it has the focus (d0078 D2)", () => {
    const inEditor = (tagName: string, editable: boolean) => ({
      tagName,
      isContentEditable: editable,
      getAttribute: () => null,
      closest: (selector: string) => (selector === ".md-editor" ? {} : null),
    });
    const text = { ...BOARD, focusKeeps: keysKeptBy(inEditor("DIV", true)) };
    const popoverField = { ...BOARD, focusKeeps: keysKeptBy(inEditor("INPUT", false)) };
    const readMode = { ...BOARD, focusKeeps: keysKeptBy(inEditor("A", false)) };
    for (const [map, mods] of [
      [MAC, { metaKey: true }],
      [WIN, { ctrlKey: true }],
    ] as const) {
      const { d, ran } = setup(map, ["search", "switch_project"]);
      const cmdK = key("k", "KeyK", mods);
      expect(dispatchKey(d, cmdK, text)).toEqual({ kind: "passed" });
      expect(dispatchKey(d, cmdK, popoverField)).toEqual({ kind: "passed" });
      // The editor's other keys are the app's as before; read mode and the board keep ⌘K as Search.
      expect(dispatchKey(d, key("p", "KeyP", mods), text).kind).toBe("ran");
      expect(dispatchKey(d, cmdK, readMode).kind).toBe("ran");
      expect(dispatchKey(d, cmdK, BOARD).kind).toBe("ran");
      expect(ran).toEqual(["switch_project", "search", "search"]);
    }
  });

  it("lets ⌥⌘C through to the description editor's text only, where it's Code block (Q51)", () => {
    const inEditor = (tagName: string, editable: boolean) => ({
      tagName,
      isContentEditable: editable,
      getAttribute: () => null,
      closest: (selector: string) => (selector === ".md-editor" ? {} : null),
    });
    const element = (tagName: string, editable: boolean) => ({
      tagName,
      isContentEditable: editable,
      getAttribute: () => null,
      closest: () => null,
    });
    const at = (focused: Parameters<typeof inEditorText>[0]) => ({
      ...BOARD,
      focusKeeps: keysKeptBy(focused),
      editorText: inEditorText(focused),
    });
    const { d } = setup(MAC);
    const optCmdC = key("ç", "KeyC", { metaKey: true, altKey: true });
    expect(dispatchKey(d, optCmdC, at(inEditor("DIV", true)))).toEqual({ kind: "passed" });
    // The link popover's URL, the Markdown source, read mode, another text field and the board: the inspector key.
    for (const focused of [
      inEditor("INPUT", false),
      inEditor("TEXTAREA", false),
      inEditor("A", false),
      element("DIV", true),
      null,
    ]) {
      expect(dispatchKey(d, optCmdC, at(focused))).toEqual({ kind: "swallowed" });
    }
    // The editor lets only that inspector key through.
    const text = at(inEditor("DIV", true));
    expect(dispatchKey(d, key("i", "KeyI", { metaKey: true, altKey: true }), text).kind).toBe("swallowed");
    expect(dispatchKey(d, key("C", "KeyC", { metaKey: true, shiftKey: true }), text).kind).toBe("swallowed");
    // Ctrl+Alt+C isn't a webview key on Windows and Linux: it reaches the editor anyway.
    const win = setup(WIN).d;
    expect(dispatchKey(win, key("c", "KeyC", { ctrlKey: true, altKey: true }), text)).toEqual({
      kind: "passed",
    });
  });

  it("lets Esc on the description field go to the form, not Close panel (T-0259, d0078 D4)", () => {
    const field = {
      tagName: "DIV",
      isContentEditable: false,
      getAttribute: () => null,
      closest(this: unknown, selector: string) {
        return selector === ".md-editor" ? this : null;
      },
    };
    const { d, ran } = setup(MAC, ["close_panel"]);
    const focused = { ...BOARD, focusKeeps: keysKeptBy(field) };
    expect(dispatchKey(d, key("Escape", "Escape"), focused)).toEqual({ kind: "passed" });
    expect(ran).toEqual([]);
  });

  it("holds the shortcuts back while a modal is open", () => {
    const { d, ran } = setup(MAC, ["search"]);
    const outcome = dispatchKey(d, key("k", "KeyK", { metaKey: true }), { ...BOARD, modal: true });
    expect(outcome.kind).toBe("unhandled");
    expect(stopsEvent(outcome)).toBe(true);
    expect(ran).toEqual([]);
  });

  it("swallows the webview's keys before the map has arrived", () => {
    const { d } = setup(undefined);
    expect(dispatchKey(d, key("F5", "F5"), BOARD).kind).toBe("swallowed");
    expect(dispatchKey(d, key("k", "KeyK", { ctrlKey: true }), BOARD).kind).toBe("passed");
  });
});

describe("the key dispatcher in a focused terminal", () => {
  it("gives a Mac terminal every key without ⌘, Esc included", () => {
    const { d, ran } = setup(MAC, ["close_panel", "open_panel"]);
    for (const k of [
      key("Escape", "Escape"),
      key("Enter", "Enter"),
      key("c", "KeyC", { ctrlKey: true }),
      key("j", "KeyJ", { ctrlKey: true }),
      key("a", "KeyA"),
      key("F5", "F5"),
    ]) {
      const outcome = dispatchKey(d, k, TERMINAL);
      expect(outcome.kind).toBe("terminal");
      expect(stopsEvent(outcome)).toBe(false);
    }
    expect(ran).toEqual([]);
  });

  it("still runs the ⌘ shortcuts on a Mac, Find among them", () => {
    const { d, ran } = setup(MAC, ["search", "switch_project", "open_inbox", "find_in_terminal"]);
    dispatchKey(d, key("k", "KeyK", { metaKey: true }), TERMINAL);
    dispatchKey(d, key("p", "KeyP", { metaKey: true }), TERMINAL);
    dispatchKey(d, key("I", "KeyI", { metaKey: true, shiftKey: true }), TERMINAL);
    expect(dispatchKey(d, key("f", "KeyF", { metaKey: true }), TERMINAL).kind).toBe("ran");
    expect(ran).toEqual(["search", "switch_project", "open_inbox", "find_in_terminal"]);
  });

  it("leaves the terminal on ⇧⌘` when no screen handles it", () => {
    const { d, left } = setup(MAC);
    const outcome = dispatchKey(d, key("~", "Backquote", { metaKey: true, shiftKey: true }), TERMINAL);
    expect(outcome).toEqual({ kind: "ran", action: { action: "leave_terminal" } });
    expect(left()).toBe(1);
    // Outside a terminal there's nothing to leave.
    dispatchKey(d, key("~", "Backquote", { metaKey: true, shiftKey: true }), BOARD);
    expect(left()).toBe(1);
  });

  it("gives plain Ctrl keys to the session on Windows and Linux and takes the Ctrl+Shift column", () => {
    const { d, ran, left } = setup(WIN, ["search", "switch_project", "find_in_terminal"]);
    for (const k of [
      key("k", "KeyK", { ctrlKey: true }),
      key("p", "KeyP", { ctrlKey: true }),
      key("r", "KeyR", { ctrlKey: true }),
      key("f", "KeyF", { ctrlKey: true }),
      key("Escape", "Escape"),
    ]) {
      expect(dispatchKey(d, k, TERMINAL).kind).toBe("terminal");
    }
    dispatchKey(d, key("K", "KeyK", { ctrlKey: true, shiftKey: true }), TERMINAL);
    dispatchKey(d, key("F", "KeyF", { ctrlKey: true, shiftKey: true }), TERMINAL);
    dispatchKey(d, key("~", "Backquote", { ctrlKey: true, shiftKey: true }), TERMINAL);
    expect(ran).toEqual(["search", "find_in_terminal"]);
    expect(left()).toBe(1);
    // Outside the terminal the Ctrl+Shift column means nothing and Ctrl+K is Search.
    expect(dispatchKey(d, key("F", "KeyF", { ctrlKey: true, shiftKey: true }), BOARD).kind).not.toBe("ran");
    expect(dispatchKey(d, key("k", "KeyK", { ctrlKey: true }), BOARD).kind).toBe("ran");
  });

  it("leaves unbound Ctrl+Shift keys to the terminal (its copy and paste)", () => {
    const { d } = setup(WIN);
    const copy = dispatchKey(d, key("C", "KeyC", { ctrlKey: true, shiftKey: true }), TERMINAL);
    expect(copy.kind).toBe("passed");
    expect(stopsEvent(copy)).toBe(false);
  });

  it("gives every key to the terminal before the map has arrived", () => {
    const { d } = setup(undefined);
    expect(dispatchKey(d, key("k", "KeyK", { metaKey: true }), TERMINAL).kind).toBe("terminal");
  });
});

describe("⇧⌘G / Ctrl+Shift+G (d0080, T-0274)", () => {
  const G_MAC: Keymap = {
    ...MAC,
    bindings: [...MAC.bindings, bind({ action: "focus_epic" }, "⇧⌘G", "g", "KeyG", { ...CMD, shift: true })],
  };
  const G_WIN: Keymap = {
    ...WIN,
    bindings: [
      ...WIN.bindings,
      bind({ action: "focus_epic" }, "Ctrl+Shift+G", "g", "KeyG", CTRL_SHIFT),
      bind({ action: "focus_epic" }, "Ctrl+Shift+G", "g", "KeyG", CTRL_SHIFT, true),
    ],
  };

  it("focuses the epic on the board and from a terminal", () => {
    const mac = setup(G_MAC, ["focus_epic"]);
    const g = key("G", "KeyG", { metaKey: true, shiftKey: true });
    expect(dispatchKey(mac.d, g, BOARD)).toEqual({ kind: "ran", action: { action: "focus_epic" } });
    expect(dispatchKey(mac.d, g, TERMINAL).kind).toBe("ran");
    const win = setup(G_WIN, ["focus_epic"]);
    const ctrl = key("G", "KeyG", { ctrlKey: true, shiftKey: true });
    expect(dispatchKey(win.d, ctrl, BOARD).kind).toBe("ran");
    expect(dispatchKey(win.d, ctrl, TERMINAL).kind).toBe("ran");
    expect(win.ran).toEqual(["focus_epic", "focus_epic"]);
  });

  it("is never the webview's Find previous, even with no handler", () => {
    const { d } = setup(G_MAC);
    const outcome = dispatchKey(d, key("G", "KeyG", { metaKey: true, shiftKey: true }), BOARD);
    expect(stopsEvent(outcome)).toBe(true);
    expect(
      stopsEvent(dispatchKey(setup(MAC).d, key("G", "KeyG", { metaKey: true, shiftKey: true }), BOARD)),
    ).toBe(true);
  });
});

describe("keys a focused control keeps", () => {
  const element = (tagName: string, attrs: Record<string, string> = {}, editable = false) => ({
    tagName,
    isContentEditable: editable,
    getAttribute: (name: string) => attrs[name] ?? null,
  });

  it("knows text fields, controls and the rest", () => {
    expect([...keysKeptBy(element("TEXTAREA"))]).toEqual(["Enter", "Backspace", "Escape"]);
    expect([...keysKeptBy(element("DIV", {}, true))]).toEqual(["Enter", "Backspace", "Escape"]);
    expect([...keysKeptBy(element("INPUT", { type: "search" }))]).toEqual(["Enter", "Backspace", "Escape"]);
    expect([...keysKeptBy(element("INPUT", { type: "checkbox" }))]).toEqual(["Enter"]);
    expect([...keysKeptBy(element("BUTTON"))]).toEqual(["Enter"]);
    expect([...keysKeptBy(element("DIV", { role: "option" }))]).toEqual(["Enter"]);
    expect([...keysKeptBy(element("DIV"))]).toEqual([]);
    expect([...keysKeptBy(null)]).toEqual([]);
  });

  it("keeps ↵ and Esc for menus, dialogs and popovers (T-0217)", () => {
    const inside = (tag: string, widget: boolean) => ({
      ...element(tag),
      closest: (selector: string) => (widget && selector.includes('role="dialog"') ? {} : null),
    });
    expect([...keysKeptBy(inside("BUTTON", true))]).toEqual(["Enter", "Escape"]);
    expect([...keysKeptBy(inside("DIV", true))]).toEqual(["Enter", "Escape"]);
    expect([...keysKeptBy(inside("TEXTAREA", true))]).toEqual(["Enter", "Backspace", "Escape"]);
    expect([...keysKeptBy(inside("BUTTON", false))]).toEqual(["Enter"]);
    const { d } = setup(MAC, []);
    const modal = { ...BOARD, modal: true, focusKeeps: keysKeptBy(inside("BUTTON", true)) };
    expect(dispatchKey(d, key("Escape", "Escape"), modal).kind).toBe("passed");
    expect(dispatchKey(d, key("Enter", "Enter"), modal).kind).toBe("passed");
  });
});

describe("the key log", () => {
  it("says where a key went", () => {
    const k = key("k", "KeyK", { metaKey: true });
    expect(logLine(k, { kind: "ran", action: { action: "search" } })).toBe("Meta+K (KeyK) → search");
    expect(logLine(key("F5", "F5"), { kind: "swallowed" })).toBe("F5 (F5) → swallowed (webview key)");
    expect(logLine(k, { kind: "unhandled", action: { action: "project", number: 3 }, swallowed: true })).toBe(
      "Meta+K (KeyK) → project 3 (no handler, swallowed)",
    );
  });
});
