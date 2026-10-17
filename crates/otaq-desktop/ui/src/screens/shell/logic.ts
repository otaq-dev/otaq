// The shell's interaction rules (UIT-04: only the moment of interaction lives here; the rows, their order and their
// wording come from Rust): the quick switcher's keys and cursor.

/** What a key in the quick switcher's field does. */
export type SwitcherStep =
  | { kind: "move"; index: number }
  | { kind: "choose"; index: number }
  | { kind: "close" }
  /** Not the switcher's key: the field types it. */
  | { kind: "pass" };

/** ↑/↓ move the highlighted row (wrapping), ↵ switches to it, Esc closes (*Quick switcher · ⌘P*). */
export function switcherKey(key: string, index: number, rows: number): SwitcherStep {
  switch (key) {
    case "ArrowDown":
      return { kind: "move", index: rows > 0 ? (index + 1) % rows : 0 };
    case "ArrowUp":
      return { kind: "move", index: rows > 0 ? (index + rows - 1) % rows : 0 };
    case "Enter":
      return rows > 0 ? { kind: "choose", index: clampCursor(index, rows) } : { kind: "pass" };
    case "Escape":
      return { kind: "close" };
    default:
      return { kind: "pass" };
  }
}

/** Keeps the highlighted row on a row after the list shrank. */
export function clampCursor(index: number, rows: number): number {
  return Math.max(0, Math.min(index, rows - 1));
}
