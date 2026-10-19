// The page's one key dispatcher (T-0215, SES-10, UIT-06). The map is the Rust keymap (`keymap.map`, the user's keys
// of d0097's §21.3 map), so nothing here lists a shortcut: a key either goes to the focused terminal, or runs the
// screen handler of the action it is bound to (`app.shortcuts.run`), or, if it is a key the webview would act on
// (reload, print, find, zoom, devtools, caret browsing, history), is swallowed without doing anything. Only modifier
// keys (and ↵ / Esc) are dispatched here so far: single letters and `g` sequences come with T-0391.
import type { KeyAction } from "../generated/KeyAction";
import type { KeyBinding } from "../generated/KeyBinding";
import type { Keymap } from "../generated/Keymap";
import type { KeyModifiers } from "../generated/KeyModifiers";
import type { Keystroke } from "../generated/Keystroke";

/** What the dispatcher reads of a key event; a `KeyboardEvent` is one. */
export interface KeyLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/** Where the key was pressed. */
export interface KeyContext {
  /** A terminal has the keyboard (the selection's `focusedTerminal`). */
  inTerminal: boolean;
  /** The keys the focused control acts on itself (`keysKeptBy`); a binding on one of them waits. */
  focusKeeps: ReadonlySet<string>;
  /** A modal overlay is open: the app's shortcuts don't run (`modalOpen`). */
  modal: boolean;
  /** The description editor's text has the focus (`inEditorText`): ⌥⌘C is its Code block, not swallowed. */
  editorText: boolean;
}

/** What became of a key. */
export type KeyOutcome =
  /** Left to the focused terminal (the session gets it). */
  | { kind: "terminal" }
  /** Its action's handler took it. */
  | { kind: "ran"; action: KeyAction }
  /** Bound, but no handler took it (or a modal held it back); swallowed outside a terminal. */
  | { kind: "unhandled"; action: KeyAction; swallowed: boolean }
  /** Not in the map, but the webview would act on it: swallowed. */
  | { kind: "swallowed" }
  /** Neither: the page's elements get it as usual. */
  | { kind: "passed" };

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS", "AltGraph", "CapsLock", "Fn"]);

const TEXT_FIELD_KEYS: ReadonlySet<string> = new Set(["Enter", "Backspace", "Escape"]);
const CONTROL_KEYS: ReadonlySet<string> = new Set(["Enter"]);
const NO_KEYS: ReadonlySet<string> = new Set();
const CONTROL_ROLES = new Set(["button", "link", "menuitem", "option", "tab", "checkbox", "radio", "switch"]);
/** Inside these, ↵ and Esc are the widget's (T-0217): a menu chooses and closes; a dialog or popover presses its
 * default button and cancels (`dialog.rs`). */
const WIDGETS = '[role="menu"], [role="dialog"]';
const WIDGET_KEYS: ReadonlySet<string> = new Set(["Enter", "Escape"]);
/** The description editor (d0078 D2, d0097): while its text or one of its fields (the link popover's URL)
 * has the focus, ⌘K / Ctrl+K is its Link, not the command palette. Its read mode isn't a text field and keeps nothing. */
const DESCRIPTION_EDITOR = ".md-editor";
const EDITOR_KEYS: ReadonlySet<string> = new Set([...TEXT_FIELD_KEYS, "k"]);

/** What `keysKeptBy` reads of the focused element; an `HTMLElement` is one. */
export interface FocusLike {
  tagName: string;
  isContentEditable: boolean;
  getAttribute: (name: string) => string | null;
  /** The nearest ancestor (or itself) matching `selector`, as `Element.closest`. */
  closest?: (selector: string) => unknown;
}

/**
 * The keys the focused element acts on itself, so a binding on them waits: a text field keeps Enter, Backspace and
 * Esc (↵ Open panel, ⌘⌫ Send back and Esc Close panel don't fire while typing); a button, link or list keeps Enter;
 * anything in a menu, dialog or popover also keeps Enter and Esc. A text field of the description editor also keeps
 * K, so ⌘K reaches the editor's Link (d0078 D2); everywhere else ⌘K stays the command palette.
 */
export function keysKeptBy(element: FocusLike | null): ReadonlySet<string> {
  let own = keptByElement(element);
  if (own === TEXT_FIELD_KEYS && element?.closest?.(DESCRIPTION_EDITOR)) own = EDITOR_KEYS;
  // The editor's field itself, focused after Esc left its text (T-0259): Esc then goes to the form (discard), not
  // the panel, and ↵ goes back into the text.
  else if (own === NO_KEYS && element !== null && element.closest?.(DESCRIPTION_EDITOR) === element)
    own = TEXT_FIELD_KEYS;
  if (!element?.closest?.(WIDGETS)) return own;
  return new Set([...own, ...WIDGET_KEYS]);
}

/**
 * Whether the focus is in the description editor's text (d0078, Q51): the one place ⌥⌘C / Ctrl+Alt+C is Code block.
 * Its link popover's URL, its Markdown source and its read mode don't toggle code blocks, so there ⌥⌘C stays swallowed.
 */
export function inEditorText(element: FocusLike | null): boolean {
  return element !== null && element.isContentEditable && Boolean(element.closest?.(DESCRIPTION_EDITOR));
}

/** ⌥⌘C on a Mac: Safari's inspector for the webview, Code block for the description editor (Q51). */
function isCodeBlockKey(e: KeyLike, mac: boolean): boolean {
  return mac && e.metaKey && e.altKey && !e.ctrlKey && !e.shiftKey && letterOf(e) === "c";
}

function keptByElement(element: FocusLike | null): ReadonlySet<string> {
  if (element === null) return NO_KEYS;
  const tag = element.tagName.toLowerCase();
  if (element.isContentEditable || tag === "textarea") return TEXT_FIELD_KEYS;
  if (tag === "input") {
    const type = (element.getAttribute("type") ?? "text").toLowerCase();
    return ["button", "submit", "reset", "checkbox", "radio", "file", "color", "range"].includes(type)
      ? CONTROL_KEYS
      : TEXT_FIELD_KEYS;
  }
  if (["button", "a", "select", "summary"].includes(tag)) return CONTROL_KEYS;
  return CONTROL_ROLES.has(element.getAttribute("role") ?? "") ? CONTROL_KEYS : NO_KEYS;
}

export function modifiersOf(e: KeyLike): KeyModifiers {
  return { control: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, platform: e.metaKey, function: false };
}

/** "Ctrl+Shift+K", "Meta+K", "F5": a key as the log shows it. */
export function keyName(e: KeyLike): string {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.metaKey) parts.push("Meta");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(e.key.length === 1 ? e.key.toUpperCase() : e.key);
  return `${parts.join("+")} (${e.code})`;
}

const isLatinLetter = (key: string) => /^[a-z]$/i.test(key);

/**
 * The letter a key types, or else its name: letters follow the keyboard layout (⌘K is K on a Dvorak keyboard too);
 * where the layout types no Latin letter, the physical key's.
 */
function letterOf(e: KeyLike): string {
  if (isLatinLetter(e.key)) return e.key.toLowerCase();
  const physical = /^Key([A-Z])$/.exec(e.code);
  return physical ? (physical[1] ?? "").toLowerCase() : e.key.toLowerCase();
}

/** Whether `e` is `stroke`: the same modifiers, and the same letter, named key or (digits and signs) physical key. */
export function strokeMatches(stroke: Keystroke, e: KeyLike): boolean {
  const m = modifiersOf(e);
  const want = stroke.modifiers;
  if (
    m.control !== want.control ||
    m.alt !== want.alt ||
    m.shift !== want.shift ||
    m.platform !== want.platform
  ) {
    return false;
  }
  if (stroke.key.length > 1) return e.key === stroke.key;
  if (isLatinLetter(stroke.key)) return letterOf(e) === stroke.key;
  return e.code === stroke.code;
}

/** The modifier key a binding has, or `undefined` for a single letter or a `g` sequence (T-0391 dispatches those). */
function strokeOf(binding: KeyBinding): Keystroke | undefined {
  return binding.key.kind === "stroke" ? binding.key.stroke : undefined;
}

/** Whether a binding works where the key was pressed (`KeyPlace`). */
function worksHere(binding: KeyBinding, inTerminal: boolean): boolean {
  return inTerminal ? binding.place !== "outside_terminals" : binding.place !== "inside_terminals";
}

/**
 * Whether the key goes to the focused terminal: `keymap::route`'s rule, from the map. A key reaches the app when it
 * holds one of the terminal app modifiers (⌘ on a Mac) or is a binding that works in terminals (a Mac's ⌃` and ⌃Tab).
 */
export function goesToTerminal(map: Keymap, e: KeyLike): boolean {
  const held = modifiersOf(e);
  const app = map.terminal_app_modifiers.some(
    (m) =>
      (!m.control || held.control) &&
      (!m.alt || held.alt) &&
      (!m.shift || held.shift) &&
      (!m.platform || held.platform),
  );
  return !app && bindingFor(map, e, true) === undefined;
}

/** The binding `e` presses where it was pressed (in a terminal only those that work there). */
export function bindingFor(map: Keymap, e: KeyLike, inTerminal: boolean): KeyBinding | undefined {
  return map.bindings.find((b) => {
    const stroke = strokeOf(b);
    return stroke !== undefined && worksHere(b, inTerminal) && strokeMatches(stroke, e);
  });
}

const WEBVIEW_FUNCTION_KEYS = new Set([
  "F1", // help
  "F3", // find next
  "F5", // reload
  "F7", // caret browsing
  "F11", // full screen
  "F12", // devtools
  "BrowserBack",
  "BrowserForward",
  "BrowserRefresh",
  "BrowserSearch",
]);

/**
 * Keys a webview acts on by itself (UIT-06): reload, print, find, zoom, devtools, caret browsing, history, save, open,
 * view source. The Edit keys (copy, paste, undo…) aren't among them: text fields need them.
 */
export function isWebviewKey(e: KeyLike, mac: boolean): boolean {
  if (WEBVIEW_FUNCTION_KEYS.has(e.key)) return true;
  const command = mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey;
  // ⌥← / ⌥→ move by word in a Mac text field; elsewhere Alt+← / → is back and forward.
  if (!mac && e.altKey && !e.ctrlKey && (e.key === "ArrowLeft" || e.key === "ArrowRight")) return true;
  if (!command) return false;
  const key = letterOf(e);
  // Devtools, hard reload, save as (⇧⌘I, ⇧⌘J, ⇧⌘C, ⇧⌘R, ⇧⌘S); ⌥⌘I / ⌥⌘C / ⌥⌘J / ⌥⌘U open Safari's inspector.
  if (e.shiftKey && !e.altKey && ["i", "j", "c", "r", "s"].includes(key)) return true;
  if (mac && e.altKey && ["i", "j", "c", "u"].includes(key)) return true;
  // Find, find next, reload, print, save, open, view source; zoom in, out and back.
  if (!e.altKey && ["f", "g", "r", "p", "s", "o", "u"].includes(key)) return true;
  return (
    ["=", "+", "-", "_", "0"].includes(e.key) ||
    ["Equal", "Minus", "Digit0", "NumpadAdd", "NumpadSubtract"].includes(e.code)
  );
}

export interface Dispatch {
  /** The map; `undefined` until `keymap.map` answered (keys then only get swallowed or passed). */
  keymap: () => Keymap | undefined;
  /** Runs the newest handler for the action; whether one took it (`app.shortcuts.run`). */
  run: (action: KeyAction) => boolean;
  /** Leaves the focused terminal: Terminals (⌃`) pressed in one. */
  leaveTerminal: () => void;
  /** Whether this is a Mac (only used before the map arrived). */
  mac: boolean;
}

/** Decides one key and does it (runs a handler, leaves the terminal); the caller stops the event for `ran` and swallowed keys. */
export function dispatchKey(d: Dispatch, e: KeyLike, ctx: KeyContext): KeyOutcome {
  const map = d.keymap();
  if (ctx.inTerminal && (map === undefined || goesToTerminal(map, e))) return { kind: "terminal" };
  const mac = map ? map.platform === "mac" : d.mac;
  let binding = map && bindingFor(map, e, ctx.inTerminal);
  if (binding && ctx.focusKeeps.has(strokeOf(binding)?.key ?? "")) binding = undefined;
  if (binding) {
    const { action } = binding;
    // Terminals in a terminal leaves it (as Leave terminal did); elsewhere its screen opens or closes the drawer.
    const leaves = !ctx.modal && action.action === "terminals" && ctx.inTerminal;
    if (leaves) d.leaveTerminal();
    const ran = leaves || (!ctx.modal && d.run(action));
    if (ran) return { kind: "ran", action };
    // A bound key does nothing else outside a terminal; in one it falls through to the terminal's own keys.
    return { kind: "unhandled", action, swallowed: !ctx.inTerminal };
  }
  // In a terminal the key is the terminal's (its copy and paste on Windows and Linux are Ctrl+Shift+C / V).
  // ⌥⌘C reaches the editor's text, whose Code block takes it and stops it (`ToolbarShortcuts`); it's swallowed elsewhere.
  if (!ctx.inTerminal && isWebviewKey(e, mac) && !(ctx.editorText && isCodeBlockKey(e, mac))) {
    return { kind: "swallowed" };
  }
  return { kind: "passed" };
}

/** Whether the page stops the event (the webview and the focused element don't see it). */
export function stopsEvent(outcome: KeyOutcome): boolean {
  switch (outcome.kind) {
    case "ran":
    case "swallowed":
      return true;
    case "unhandled":
      return outcome.swallowed;
    case "terminal":
    case "passed":
      return false;
  }
}

export function isModifierKey(e: KeyLike): boolean {
  return MODIFIER_KEYS.has(e.key);
}

/** The log line for a key (`OTAQ_KEY_LOG=1`). */
export function logLine(e: KeyLike, outcome: KeyOutcome): string {
  const name = keyName(e);
  const action = (a: KeyAction) => ("number" in a ? `${a.action} ${String(a.number)}` : a.action);
  switch (outcome.kind) {
    case "terminal":
      return `${name} → terminal`;
    case "ran":
      return `${name} → ${action(outcome.action)}`;
    case "unhandled":
      return `${name} → ${action(outcome.action)} (no handler${outcome.swallowed ? ", swallowed" : ""})`;
    case "swallowed":
      return `${name} → swallowed (webview key)`;
    case "passed":
      return `${name} → page`;
  }
}
