// A click on the board's own background closes the ticket sheet (SHL-05, OT-3): a column's empty area, the gap between
// columns, the room after the last one. Only a plain left press and release on it counts: the press and the release
// both on background, the pointer moved less than a drag's threshold, and no menu or popover open at the press (that
// click is the one that closes it). Cards, buttons, the strip, the agent chip and the Done rail are no background.
import { DRAG_THRESHOLD } from "./drag";

/** The board's own surfaces: a press right on one of them (not on something inside it) is on the background. */
const BACKGROUND = [
  ".board",
  ".board-columns",
  ".board-column",
  ".board-column-header",
  ".board-spacer",
  ".board-column-hint",
  ".board-column-cards",
  ".board-review-section",
  ".board-review-empty",
  ".board-empty-column",
  ".board-hidden",
].join(", ");

/** What closes itself on a press outside it, so the press is taken by it. */
const FLOATING = ".menu, .popover";

export function isBoardBackground(target: EventTarget | null): boolean {
  return target instanceof Element && target.matches(BACKGROUND);
}

interface Press {
  button: number;
  target: EventTarget | null;
  clientX: number;
  clientY: number;
}

export interface BackgroundClick {
  press(e: Press): void;
  /** Calls `close` when the press was a plain click on the background. */
  release(e: Pick<Press, "target" | "clientX" | "clientY">): void;
  cancel(): void;
}

export function createBackgroundClick(close: () => void): BackgroundClick {
  let pressed: { x: number; y: number } | null = null;
  return {
    press(e) {
      const floating = e.target instanceof Node && e.target.ownerDocument?.querySelector(FLOATING);
      pressed =
        e.button === 0 && isBoardBackground(e.target) && !floating ? { x: e.clientX, y: e.clientY } : null;
    },
    release(e) {
      const at = pressed;
      pressed = null;
      if (at === null || !isBoardBackground(e.target)) return;
      if (Math.hypot(e.clientX - at.x, e.clientY - at.y) < DRAG_THRESHOLD) close();
    },
    cancel() {
      pressed = null;
    },
  };
}
