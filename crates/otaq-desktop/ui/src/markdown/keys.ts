// Keys and focus around the description editor (T-0259, d0078 D3 / D4, d0058). One plain function, `routeKey`, decides
// what Tab, Esc and ⌘↵ do wherever they are pressed: in the editor's text (or the Markdown source view), on the field
// once Esc has taken focus out of the text, and anywhere else in the form. The editor, the field and the form's
// `useFormKeys` each ask it and do only their own part; a key one of them took (`preventDefault`) never reaches the
// next one's action.
//
// - Tab / ⇧Tab indent and outdent in a list item or a code block; anywhere else in the editor they move focus out of it
//   (forward to the next field, back to the previous one). From a list or code block, Esc then Tab leaves.
// - Esc steps back one layer at a time (`ESC_LAYERS`): an open popover or menu closes (it takes the key itself), else
//   the full-window view collapses (T-0260), else focus leaves the editor's text, else the form discards (no
//   confirmation).
// - ⌘↵ / Ctrl+↵ saves from anywhere in the form, the editor included.
// - Nothing interrupts an input method (`isComposing`).

/** The parts of a key event the routing reads; a `KeyboardEvent` is one. */
export interface KeyInput {
  key: string;
  shiftKey: boolean;
  metaKey: boolean;
  ctrlKey: boolean;
  altKey: boolean;
  isComposing?: boolean;
}

/** The layers Esc steps back through, innermost first. Each one that is open takes Esc before the ones after it. */
export const ESC_LAYERS = ["popover", "fullView", "editor", "form"] as const;
export type EscLayer = (typeof ESC_LAYERS)[number];

/** Where focus is, as the routing sees it. */
export interface KeyPlace {
  /** A popover or menu (link popover, language list, "…", heading menu, a select's list) has the key. */
  layer: boolean;
  /**
   * `text`: the editor's text or its source view has the caret. `field`: the field itself has focus (after Esc).
   * `form`: anywhere else in the form.
   */
  focus: "text" | "field" | "form";
  /** The block the caret is in, when it is in the text: Tab means indent there. */
  block?: "list" | "code" | null;
  /** The key was pressed in the full-window view (T-0260): Esc collapses it before anything else but a popover. */
  fullView?: boolean;
  /** Whether the form discards on Esc (the ⌘N modal leaves Esc to its dialog, which cancels). */
  discards?: boolean;
  /** Something inside already took the key (`preventDefault`): the editor's Esc or Tab, a list's indent. */
  taken?: boolean;
}

export type KeyRoute =
  /** Not ours: the key goes on as usual (typing, a layer's own keys, the editor's keymaps). */
  | { kind: "none" }
  /** ⌘↵: the form saves. */
  | { kind: "save" }
  /** Tab / ⇧Tab in a list item: the item nests under the one above, or moves out a level. */
  | { kind: "indent"; out: boolean }
  /** Tab / ⇧Tab in a code block: left to the code block's own indent and outdent of the line. */
  | { kind: "code" }
  /** Tab out of the editor: focus moves to the next field, or back to the previous one. */
  | { kind: "focus"; back: boolean }
  /** Esc in the text: focus leaves it for the field, so Tab leaves next and Esc after that discards. */
  | { kind: "leave" }
  /** ↵ on the field: the caret goes back into the text. */
  | { kind: "enter" }
  /** Esc anywhere else in the form: the form discards. */
  | { kind: "discard" }
  /** Esc in the full-window view: it collapses back into the form or the Ticket tab. */
  | { kind: "collapse" };

const NONE: KeyRoute = { kind: "none" };

/** ⌘↵ on macOS, Ctrl+↵ elsewhere (either is accepted everywhere, as the form always has). */
export function isSaveKey(e: KeyInput): boolean {
  return e.key === "Enter" && (e.metaKey || e.ctrlKey) && !e.isComposing;
}

/** The layer that takes Esc: the first open one in `ESC_LAYERS`' order, or `null` when none does. */
export function escLayer(open: ReadonlySet<EscLayer>): EscLayer | null {
  return ESC_LAYERS.find((layer) => open.has(layer)) ?? null;
}

/** Which layers are open for a key pressed at `at`. */
export function openLayers(at: KeyPlace): ReadonlySet<EscLayer> {
  const open = new Set<EscLayer>();
  if (at.layer) open.add("popover");
  if (at.fullView === true) open.add("fullView");
  if (at.focus === "text") open.add("editor");
  if (at.discards === true) open.add("form");
  return open;
}

/** What a key does, pressed at `at`. */
export function routeKey(e: KeyInput, at: KeyPlace): KeyRoute {
  if (e.isComposing === true) return NONE;
  if (isSaveKey(e)) return { kind: "save" };
  // A popover or menu has its own keys (its ↵, Esc and Tab), and a key taken inside is done.
  if (at.layer || at.taken === true) return NONE;
  if (e.key === "Escape" && !e.metaKey && !e.ctrlKey && !e.altKey) return escRoute(escLayer(openLayers(at)));
  if (e.key === "Tab" && !e.metaKey && !e.ctrlKey && !e.altKey) return tabRoute(e.shiftKey, at);
  if (e.key === "Enter" && at.focus === "field" && !e.shiftKey && !e.altKey) return { kind: "enter" };
  return NONE;
}

function escRoute(layer: EscLayer | null): KeyRoute {
  switch (layer) {
    case "fullView":
      return { kind: "collapse" };
    case "editor":
      return { kind: "leave" };
    case "form":
      return { kind: "discard" };
    // The popover closes itself; with no layer left (the field, in the ⌘N modal) Esc goes on to the dialog.
    case "popover":
    case null:
      return NONE;
  }
}

function tabRoute(back: boolean, at: KeyPlace): KeyRoute {
  if (at.focus === "form") return NONE;
  if (at.focus === "text" && at.block === "list") return { kind: "indent", out: back };
  if (at.focus === "text" && at.block === "code") return { kind: "code" };
  return { kind: "focus", back };
}

// ---- The DOM side: kept small, the decisions are above. ----

/** The popovers and menus that take their own keys: menus and selects, the link popover, the code block's controls. */
export const KEY_LAYERS = '[role="menu"], [role="listbox"], [role="dialog"], .md-code-tools';

/**
 * Whether `target` is inside a popover or menu that sits inside `scope` (a modal is itself a dialog: only the layers
 * inside it count).
 */
export function inKeyLayer(target: EventTarget | null, scope: Element): boolean {
  if (!(target instanceof Element)) return false;
  const layer = target.closest(KEY_LAYERS);
  return layer !== null && layer !== scope && scope.contains(layer);
}

const TABBABLE =
  'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [contenteditable="true"], [tabindex]';

/** The elements Tab visits inside `scope`, in document order: focusable, not taken out with `tabindex="-1"`. */
export function tabbables(scope: ParentNode): HTMLElement[] {
  return [...scope.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (el) => el.tabIndex >= 0 && !el.closest("[inert]"),
  );
}

/**
 * Moves focus from `from` (the editor's field) to the next tabbable element after it, or the one before it, skipping
 * everything inside it. Inside a modal the order wraps around, as its own Tab does. Returns whether focus moved.
 */
export function focusPast(from: HTMLElement, back: boolean): boolean {
  const scope = from.closest('[aria-modal="true"]') ?? from.ownerDocument.body;
  const items = tabbables(scope).filter((el) => !from.contains(el));
  const after = items.filter(
    (el) => (from.compareDocumentPosition(el) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
  );
  const before = items.filter((el) => !after.includes(el));
  const wraps = scope !== from.ownerDocument.body;
  const order = back
    ? [...before.reverse(), ...(wraps ? after.reverse() : [])]
    : [...after, ...(wraps ? before : [])];
  // A hidden element doesn't take focus: the next one in line does.
  return order.some((el) => {
    el.focus();
    return from.ownerDocument.activeElement === el;
  });
}
