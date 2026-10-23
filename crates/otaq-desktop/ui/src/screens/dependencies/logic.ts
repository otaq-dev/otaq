// The Dependencies section's moment of interaction (DPV-04, T-0463): which picker row is highlighted and what a key
// does in the search. What it lists and every word are Rust's (`dependencies.view`, `dependencies.search`); the
// highlight follows `dep_picker.rs`.

/** The service events that change the section: any ticket's change moves a linked ticket's place. */
export const DEPENDENCY_EVENTS = ["ticket_changed", "ticket_deleted", "session_state"] as const;

/** The row ↵ picks (`dep_picker.rs`): the hovered or arrowed-to row, else the first. */
export function highlighted(hover: number | null, count: number): number | null {
  if (count === 0) return null;
  return hover !== null && hover < count ? hover : 0;
}

/** ↑ / ↓ in the picker: the previous or next row, stopping at the ends. */
export function step(hover: number | null, delta: 1 | -1, count: number): number | null {
  const from = highlighted(hover, count);
  if (from === null) return null;
  return Math.min(Math.max(from + delta, 0), count - 1);
}
