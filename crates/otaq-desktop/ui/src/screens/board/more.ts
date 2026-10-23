// The "7 more stages" pill at the board's right edge (BRD-02, `s24-04`): how many columns are cut off past the
// edge as the board scrolls, worded with Rust's words (`BoardMoreStages`), and where a click scrolls to.
import type { BoardMoreStages } from "../../generated/BoardMoreStages";

/** A column's left and right edge, in the same coordinates as the view. */
export interface Span {
  left: number;
  right: number;
}

/** The columns cut off at the right edge `right` (a pixel of rounding is not cut off), left to right. */
export function columnsAfter(columns: readonly Span[], right: number): Span[] {
  return columns.filter((c) => c.right > right + 1);
}

/** The pill's words for `n` columns cut off; `null` when none is. */
export function moreText(words: BoardMoreStages, n: number): string | null {
  if (n === 0) return null;
  return n === 1 ? words.one : words.many.replace("{n}", String(n));
}

/** Where a click on the pill scrolls the board (its `scrollLeft`): the first cut-off column at the left edge. */
export function nextScroll(
  after: readonly Span[],
  boardLeft: number,
  scrollLeft: number,
  padding: number,
): number | null {
  const first = after[0];
  return first === undefined ? null : first.left - boardLeft + scrollLeft - padding;
}
