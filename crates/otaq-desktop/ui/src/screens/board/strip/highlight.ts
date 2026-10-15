// What the strip does to the board's cards while it is hovered (*Note · Interactions (0080) → Hover*): a hovered
// stop outlines its card and scrolls it into view; a hovered track name previews the focus (the epic's cards stay,
// the other cards dim). Like the chain layer it marks the cards in the DOM, so a hover redraws no column.

/** What the strip's pointer (or keyboard focus) is on. */
export type StripHover = { kind: "stop"; key: string } | { kind: "preview"; keys: readonly string[] } | null;

function cardOf(board: HTMLElement, key: string): HTMLElement | null {
  return board.querySelector<HTMLElement>(`.board-card[data-card="${CSS.escape(key)}"]`);
}

/** Marks the cards for `hover` and returns what unmarks them. A card that isn't on the board (folded in Dev, in
 * the Done rail, filtered out) gets nothing. */
export function markCards(board: HTMLElement, hover: StripHover): () => void {
  if (hover === null) return () => undefined;
  if (hover.kind === "stop") {
    const card = cardOf(board, hover.key);
    card?.setAttribute("data-strip-found", "");
    return () => {
      card?.removeAttribute("data-strip-found");
    };
  }
  const kept = hover.keys.flatMap((key) => cardOf(board, key) ?? []);
  board.setAttribute("data-strip-preview", "");
  for (const card of kept) card.setAttribute("data-strip-keep", "");
  return () => {
    board.removeAttribute("data-strip-preview");
    for (const card of kept) card.removeAttribute("data-strip-keep");
  };
}

/** Scrolls a stop's card into view, inside its column and the board. */
export function revealCard(board: HTMLElement, key: string): void {
  cardOf(board, key)?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
}
