// The palette's interaction rules (UIT-04: only the moment of interaction lives here; the rows, their order, which can
// run and every word come from Rust, `palette.view`): the highlight, the field's keys, the `>` prefix, and which
// selection changes go into the recent items.
import type { Selection } from "../../app/selection";
import type { PaletteRow } from "../../generated/PaletteRow";
import type { PaletteUse } from "../../generated/PaletteUse";

/** The `>` that lists actions only (KEY-06). */
export const ACTIONS_PREFIX = ">";

/** The first row the highlight may take (not a greyed action, not the divider); `-1` when there is none. */
export function firstPickable(rows: readonly PaletteRow[]): number {
  return rows.findIndex((r) => r.pickable);
}

/** The next row the highlight may take from `index` going `step`, wrapping and skipping greyed rows; `index` itself
 * when no other can, `-1` when none can. */
export function stepPickable(rows: readonly PaletteRow[], index: number, step: 1 | -1): number {
  const n = rows.length;
  for (let i = 1; i <= n; i++) {
    const at = (((index + step * i) % n) + n) % n;
    if (rows[at]?.pickable) return at;
  }
  return -1;
}

/** What a key in the palette's field does. */
export type PaletteStep =
  | { kind: "move"; index: number }
  | { kind: "choose"; index: number }
  | { kind: "close" }
  /** Backspace in an empty field after `>`: the prefix goes. */
  | { kind: "drop_prefix" }
  /** Not the palette's key: the field types it. */
  | { kind: "pass" };

/** ↑ ↓ move the highlight over the rows that can run, ↵ runs or opens the highlighted one (a greyed row does
 * nothing), Esc closes. */
export function paletteKey(
  key: string,
  index: number,
  rows: readonly PaletteRow[],
  field: { value: string; actionsOnly: boolean },
): PaletteStep {
  switch (key) {
    case "ArrowDown":
      return { kind: "move", index: stepPickable(rows, index, 1) };
    case "ArrowUp":
      return { kind: "move", index: stepPickable(rows, index, -1) };
    case "Enter":
      return rows[index]?.pickable ? { kind: "choose", index } : { kind: "pass" };
    case "Escape":
      return { kind: "close" };
    case "Backspace":
      return field.actionsOnly && field.value === "" ? { kind: "drop_prefix" } : { kind: "pass" };
    default:
      return { kind: "pass" };
  }
}

/** The field's text without the `>` (drawn in front of it), and whether the query lists actions only. */
export function splitQuery(query: string): { actionsOnly: boolean; value: string } {
  return query.startsWith(ACTIONS_PREFIX)
    ? { actionsOnly: true, value: query.slice(ACTIONS_PREFIX.length) }
    : { actionsOnly: false, value: query };
}

/** The query for the field's new text. */
export function joinQuery(actionsOnly: boolean, value: string): string {
  return actionsOnly ? ACTIONS_PREFIX + value : value;
}

/**
 * What a selection change records for the recent items: switching from one project to another records the project
 * (the ticket that project shows again isn't "opened"); opening another ticket in the same project records it.
 */
export function usesOf(before: Selection, after: Selection): PaletteUse | null {
  if (after.project !== before.project) {
    return before.project !== null && after.project !== null
      ? { kind: "project", project: after.project }
      : null;
  }
  if (after.ticket !== null && after.ticket !== before.ticket) return { kind: "ticket", key: after.ticket };
  return null;
}
