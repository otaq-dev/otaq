// Placing a floating layer (menu, popover, tooltip) next to its anchor: fixed to the window, so a scrolling or
// clipping parent never cuts it, and drawn inside the anchor's subtree, so the anchor's theme applies to it.
import { type CSSProperties, type RefObject, useLayoutEffect, useState } from "react";

import { placeFloating, type Rect } from "./logic";

/** What a floating layer sits next to: an element, or a point's rectangle (a context menu at the pointer). */
export type Anchor = RefObject<HTMLElement | null> | Rect;

function isRef(anchor: Anchor): anchor is RefObject<HTMLElement | null> {
  return "current" in anchor;
}

/**
 * The fixed position of `floating` next to `anchor` (see `placeFloating`), measured before the first paint. Hidden
 * until measured, so it never flashes at the window's corner.
 */
export function useFloatingStyle(
  anchor: Anchor,
  floating: RefObject<HTMLElement | null>,
  align: "start" | "end" = "start",
): CSSProperties {
  const [position, setPosition] = useState<{ left: number; top: number }>();
  // A rectangle is a new object on each render: compare it by value.
  const ref = isRef(anchor) ? anchor : null;
  const rect = isRef(anchor) ? null : anchor;
  const rectKey = rect
    ? `${String(rect.left)},${String(rect.top)},${String(rect.width)},${String(rect.height)}`
    : "";
  useLayoutEffect(() => {
    const f = floating.current;
    if (!f) return;
    const place = () => {
      const box = ref ? ref.current?.getBoundingClientRect() : rect;
      if (!box) return;
      setPosition(
        placeFloating(
          box,
          { width: f.offsetWidth, height: f.offsetHeight },
          { width: window.innerWidth, height: window.innerHeight },
          align,
        ),
      );
    };
    place();
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("resize", place);
    };
    // `rect` is compared by `rectKey`.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a rectangle anchor is a new object on every render; its value is `rectKey`
  }, [ref, rectKey, floating, align]);
  return position
    ? { position: "fixed", left: position.left, top: position.top }
    : { position: "fixed", left: 0, top: 0, visibility: "hidden" };
}
