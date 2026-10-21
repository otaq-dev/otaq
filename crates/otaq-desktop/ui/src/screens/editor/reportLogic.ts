// The report view's scrolling (CMT-12): which lines a scroll position shows and which pages of lines that needs. The
// list is as tall as the report's lines, up to the tallest box a browser draws; past that, the scroll position maps
// onto the lines proportionally, so a report of millions of short lines still scrolls end to end.

/** One line's height, px (the frame's 12 px mono lines with their padding). */
export const ROW_HEIGHT = 20;
/** The tallest list drawn, px. */
export const MAX_HEIGHT = 8_000_000;
/** How long after typing stops the search runs, ms. */
export const SEARCH_DELAY = 200;

/** The list's full height. */
export function listHeight(total: number): number {
  return Math.min(total * ROW_HEIGHT, MAX_HEIGHT);
}

/** How many lines a viewport `height` px tall shows (one more for the part-shown line). */
export function visibleRows(height: number): number {
  return Math.ceil(height / ROW_HEIGHT) + 1;
}

/** The rows' own scroll positions are exact while the list is no taller than `MAX_HEIGHT`. */
export function exact(total: number): boolean {
  return total * ROW_HEIGHT <= MAX_HEIGHT;
}

/** The first line (0-based) shown at scroll position `top` in a viewport `height` px tall. */
export function firstLine(top: number, height: number, total: number): number {
  const last = Math.max(0, total - visibleRows(height) + 1);
  if (exact(total)) return Math.min(Math.floor(top / ROW_HEIGHT), Math.max(0, total - 1));
  const room = MAX_HEIGHT - height;
  return room <= 0 ? 0 : Math.min(last, Math.round((top / room) * last));
}

/** The scroll position that shows `line` a third of the way down a viewport `height` px tall. */
export function scrollFor(line: number, height: number, total: number): number {
  if (exact(total)) return Math.max(0, line * ROW_HEIGHT - Math.floor(height / 3));
  const last = Math.max(1, total - visibleRows(height) + 1);
  return Math.max(0, Math.min(1, line / last) * (MAX_HEIGHT - height));
}

/** The pages (by index) holding lines `first .. first + rows`, within `total`. */
export function pagesFor(first: number, rows: number, pageLines: number, total: number): number[] {
  if (total === 0) return [];
  const from = Math.floor(first / pageLines);
  const to = Math.floor(Math.min(total - 1, first + rows) / pageLines);
  const out: number[] = [];
  for (let p = from; p <= to; p++) out.push(p);
  return out;
}
