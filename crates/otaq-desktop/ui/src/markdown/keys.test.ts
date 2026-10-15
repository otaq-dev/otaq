// The key routing (T-0259, d0078 D3 / D4, d0058) without a page: the Esc layer order, the Tab rules, ⌘↵ and input
// methods.

import { describe, expect, test } from "vitest";

import { ESC_LAYERS, escLayer, type KeyInput, type KeyPlace, routeKey } from "./keys";

const key = (k: string, mods: Partial<KeyInput> = {}): KeyInput => ({
  key: k,
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  ...mods,
});

const TEXT: KeyPlace = { layer: false, focus: "text", block: null };
const FIELD: KeyPlace = { layer: false, focus: "field" };
const FORM: KeyPlace = { layer: false, focus: "form", discards: true };

describe("Esc steps back one layer at a time (D4)", () => {
  test("the layers, innermost first", () => {
    expect(ESC_LAYERS).toEqual(["popover", "editor", "form"]);
    expect(escLayer(new Set(["form", "editor", "popover"]))).toBe("popover");
    expect(escLayer(new Set(["form", "editor"]))).toBe("editor");
    expect(escLayer(new Set(["form"]))).toBe("form");
    expect(escLayer(new Set())).toBeNull();
  });

  test("an open popover or menu takes it first, wherever it sits", () => {
    for (const at of [TEXT, FIELD, FORM])
      expect(routeKey(key("Escape"), { ...at, layer: true })).toEqual({ kind: "none" });
  });

  test("then focus leaves the editor's text, in a list, a code block or anywhere else", () => {
    for (const block of [null, "list", "code"] as const)
      expect(routeKey(key("Escape"), { ...TEXT, block })).toEqual({ kind: "leave" });
  });

  test("then the form discards; the field itself passes Esc on to it", () => {
    expect(routeKey(key("Escape"), FIELD)).toEqual({ kind: "none" });
    expect(routeKey(key("Escape"), FORM)).toEqual({ kind: "discard" });
  });

  test("the form doesn't discard a key taken inside, nor where it has no discard (the ⌘N modal's dialog cancels)", () => {
    expect(routeKey(key("Escape"), { ...FORM, taken: true })).toEqual({ kind: "none" });
    expect(routeKey(key("Escape"), { ...FORM, discards: false })).toEqual({ kind: "none" });
  });

  test("Esc with a modifier isn't stepping back", () => {
    expect(routeKey(key("Escape", { metaKey: true }), TEXT)).toEqual({ kind: "none" });
    expect(routeKey(key("Escape", { altKey: true }), FORM)).toEqual({ kind: "none" });
  });
});

describe("Tab (D3)", () => {
  test("in a list item, Tab indents and ⇧Tab outdents", () => {
    const list = { ...TEXT, block: "list" as const };
    expect(routeKey(key("Tab"), list)).toEqual({ kind: "indent", out: false });
    expect(routeKey(key("Tab", { shiftKey: true }), list)).toEqual({ kind: "indent", out: true });
  });

  test("in a code block, both are the block's own line indent", () => {
    const code = { ...TEXT, block: "code" as const };
    expect(routeKey(key("Tab"), code)).toEqual({ kind: "code" });
    expect(routeKey(key("Tab", { shiftKey: true }), code)).toEqual({ kind: "code" });
  });

  test("anywhere else in the text (a paragraph, a heading, a table) Tab moves focus out, ⇧Tab back", () => {
    expect(routeKey(key("Tab"), TEXT)).toEqual({ kind: "focus", back: false });
    expect(routeKey(key("Tab", { shiftKey: true }), TEXT)).toEqual({ kind: "focus", back: true });
  });

  test("Esc then Tab leaves a list or a code block: on the field Tab moves focus out", () => {
    expect(routeKey(key("Tab"), FIELD)).toEqual({ kind: "focus", back: false });
    expect(routeKey(key("Tab", { shiftKey: true }), FIELD)).toEqual({ kind: "focus", back: true });
  });

  test("outside the editor, in a popover, or with ⌘ / Ctrl / ⌥, Tab is left as it is", () => {
    expect(routeKey(key("Tab"), FORM)).toEqual({ kind: "none" });
    expect(routeKey(key("Tab"), { ...TEXT, block: "list", layer: true })).toEqual({ kind: "none" });
    expect(routeKey(key("Tab", { ctrlKey: true }), TEXT)).toEqual({ kind: "none" });
    expect(routeKey(key("Tab", { altKey: true }), TEXT)).toEqual({ kind: "none" });
  });
});

describe("⌘↵ / Ctrl+↵ saves from anywhere in the form", () => {
  test.each([
    ["the text", TEXT],
    ["a list", { ...TEXT, block: "list" as const }],
    ["the field", FIELD],
    ["the form", FORM],
    ["a popover", { ...TEXT, layer: true }],
    ["a key the editor took", { ...FORM, taken: true }],
  ])("in %s", (_, at) => {
    expect(routeKey(key("Enter", { metaKey: true }), at)).toEqual({ kind: "save" });
    expect(routeKey(key("Enter", { ctrlKey: true }), at)).toEqual({ kind: "save" });
  });

  test("↵ alone types in the text and goes back into it from the field", () => {
    expect(routeKey(key("Enter"), TEXT)).toEqual({ kind: "none" });
    expect(routeKey(key("Enter"), FORM)).toEqual({ kind: "none" });
    expect(routeKey(key("Enter"), FIELD)).toEqual({ kind: "enter" });
    expect(routeKey(key("Enter", { shiftKey: true }), FIELD)).toEqual({ kind: "none" });
  });
});

test("an input method is never interrupted", () => {
  for (const k of ["Escape", "Tab", "Enter"])
    for (const at of [TEXT, FIELD, FORM, { ...TEXT, block: "list" as const }])
      expect(routeKey(key(k, { isComposing: true, metaKey: k === "Enter" }), at)).toEqual({ kind: "none" });
});

test("other keys are never the routing's", () => {
  for (const k of ["a", "ArrowDown", "Backspace", "k"])
    for (const at of [TEXT, FIELD, FORM]) expect(routeKey(key(k), at)).toEqual({ kind: "none" });
});
