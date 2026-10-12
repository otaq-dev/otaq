// The chain highlight's drawing layer over the board (*03 → Chain highlight · hover or select SHOP-20 → Chain arrows,
// Chain legend*): accent arrows parent → child, only between members whose cards are in view (a card scrolled out of
// its column, folded in Dev or in the Done rail gets none), and the legend at the board's bottom right that reads the
// chain. Like the lifted card it is fixed to the window inside the scrolling board, laid over the board's own box,
// and measured again as the board or a column scrolls or resizes.
import { useLayoutEffect, useRef } from "react";

import type { ChainView } from "../../generated/ChainView";
import { Icon } from "../../theme/Icon";
import { type Arrow, arrow, type Box, inView } from "./chain";

/** The legend's distance from the board's right and bottom edges (the frame's, clear of the Done rail). */
const LEGEND_RIGHT = 52;
const LEGEND_BOTTOM = 40;

const SVG = "http://www.w3.org/2000/svg";

function path(className: string, d: string): SVGPathElement {
  const el = document.createElementNS(SVG, "path");
  el.setAttribute("class", className);
  el.setAttribute("d", d);
  return el;
}

function relative(r: DOMRect, origin: DOMRect): Box {
  return {
    left: r.left - origin.left,
    top: r.top - origin.top,
    right: r.right - origin.left,
    bottom: r.bottom - origin.top,
  };
}

/** The arrows of `chain` between cards in view inside `board`, in the board box's coordinates. */
function measure(chain: ChainView, board: HTMLElement, origin: DOMRect): Arrow[] {
  const view = relative(origin, origin);
  const boxes = new Map<string, Box>();
  for (const key of chain.members) {
    const card = board.querySelector<HTMLElement>(`[data-card="${CSS.escape(key)}"]`);
    if (!card) continue;
    const box = relative(card.getBoundingClientRect(), origin);
    const scroller = card.closest<HTMLElement>(".board-column-cards");
    const clips = scroller ? [view, relative(scroller.getBoundingClientRect(), origin)] : [view];
    if (inView(box, clips)) boxes.set(key, box);
  }
  return chain.edges.flatMap(({ parent, child }) => {
    const from = boxes.get(parent);
    const to = boxes.get(child);
    return from && to ? [arrow(from, to)] : [];
  });
}

const px = (n: number) => `${String(Math.round(n))}px`;

export interface ChainLayerProps {
  chain: ChainView | null;
  /** The board's scrolling element, holding the cards. */
  board: HTMLElement | null;
  /** Changes whenever the cards may have moved (a new board view). */
  layout: unknown;
}

export function ChainLayer({ chain, board, layout }: ChainLayerProps) {
  const svg = useRef<SVGSVGElement>(null);
  const legend = useRef<HTMLDivElement>(null);

  // The layer follows the cards' places on screen, so it is placed and drawn straight in the DOM (no render per
  // scroll).
  useLayoutEffect(() => {
    const layer = svg.current;
    const note = legend.current;
    if (chain === null || board === null || layer === null || note === null) return;
    let pending = 0;
    const draw = () => {
      const box = board.getBoundingClientRect();
      Object.assign(layer.style, {
        left: px(box.left),
        top: px(box.top),
        width: px(box.width),
        height: px(box.height),
      });
      Object.assign(note.style, {
        right: px(window.innerWidth - box.right + LEGEND_RIGHT),
        bottom: px(window.innerHeight - box.bottom + LEGEND_BOTTOM),
        maxWidth: px(Math.max(box.width - 2 * LEGEND_RIGHT, 0)),
      });
      layer.replaceChildren(
        ...measure(chain, board, box).flatMap((a) => [
          path("board-chain-line", a.line),
          path("board-chain-head", a.head),
        ]),
      );
    };
    const later = () => {
      cancelAnimationFrame(pending);
      pending = requestAnimationFrame(draw);
    };
    draw();
    // Scroll events don't bubble; capturing them on the board catches the board's and every column's.
    board.addEventListener("scroll", later, { capture: true, passive: true });
    const observer = new ResizeObserver(later);
    observer.observe(board);
    window.addEventListener("resize", later);
    return () => {
      cancelAnimationFrame(pending);
      board.removeEventListener("scroll", later, { capture: true });
      observer.disconnect();
      window.removeEventListener("resize", later);
    };
  }, [chain, board, layout]);

  if (chain === null) return null;
  return (
    <>
      <svg className="board-chain-arrows" ref={svg} aria-hidden="true" />
      <div className="board-chain-legend" ref={legend} role="status" title={chain.legend}>
        <Icon name="link-2" size={12} />
        <span>{chain.legend}</span>
      </div>
    </>
  );
}
