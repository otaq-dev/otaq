// Which buttons move into "…" when the toolbar is too narrow (d0078 D8): Strikethrough, Quote, Table and Horizontal
// rule, in that order, until the rest fits. Expand and the Markdown toggle aren't items here: they never move.

/** One thing drawn in the toolbar's left part (a button, the Heading menu, a separator), with its measured width. */
export interface OverflowItem {
  id: string;
  /** Its width plus the gap after it, in px. */
  width: number;
  /** Its place in the overflow order (`ToolbarButton.overflow`); without one it never moves. */
  overflow?: number | undefined;
}

/**
 * The ids that go into "…", in the order they moved (the menu lists them that way). None while everything fits;
 * once something moves, "…" itself (`moreWidth`) takes room too.
 */
export function overflowed(items: readonly OverflowItem[], available: number, moreWidth: number): string[] {
  let needed = items.reduce((sum, item) => sum + item.width, 0);
  if (needed <= available) return [];
  needed += moreWidth;
  const movable = items
    .filter((item) => item.overflow !== undefined)
    .toSorted((a, b) => (a.overflow ?? 0) - (b.overflow ?? 0));
  const moved: string[] = [];
  for (const item of movable) {
    if (needed <= available) break;
    moved.push(item.id);
    needed -= item.width;
  }
  return moved;
}
