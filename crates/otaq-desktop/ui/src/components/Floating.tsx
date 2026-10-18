// Placing a floating layer (menu, popover, tooltip) next to its anchor: fixed to the window, so a scrolling or
// clipping parent never cuts it, and drawn inside the anchor's subtree, so the anchor's theme applies to it.
import { type CSSProperties, type RefObject, useEffect, useLayoutEffect, useRef, useState } from "react";

import { placeFloating, type Rect } from "./logic";
import { useEnterExit } from "./motion";

/** What a floating layer sits next to: an element, or a point's rectangle (a context menu at the pointer). */
export type Anchor = RefObject<HTMLElement | null> | Rect;

function isRef(anchor: Anchor): anchor is RefObject<HTMLElement | null> {
  return "current" in anchor;
}

/**
 * The fixed position of `floating` next to `anchor` (see `placeFloating`), measured before the first paint. Hidden
 * until measured, so it never flashes at the window's corner.
 *
 * A scroll anywhere behind the layer moves its anchor: a layer that can close (`onScrollAway`: a menu, a dropdown, a
 * popover) closes then, like a press outside and like a native popup; one that can't (a tooltip) follows its anchor.
 * A scroll inside the layer itself (a long list) is its own and changes nothing.
 *
 * It fades in, dropping 4 px into place, and back out the same way when it closes (OT-6).
 */
export function useFloatingStyle(
  anchor: Anchor,
  floating: RefObject<HTMLElement | null>,
  align: "start" | "end" = "start",
  onScrollAway?: () => void,
): CSSProperties {
  useEnterExit(floating, "floating");
  const [position, setPosition] = useState<{ left: number; top: number }>();
  // A rectangle is a new object on each render: compare it by value.
  const ref = isRef(anchor) ? anchor : null;
  const rect = isRef(anchor) ? null : anchor;
  const rectKey = rect
    ? `${String(rect.left)},${String(rect.top)},${String(rect.width)},${String(rect.height)}`
    : "";
  // The latest callback, so an owner's inline function doesn't re-place the layer on every render.
  const scrollAway = useRef(onScrollAway);
  useEffect(() => {
    scrollAway.current = onScrollAway;
  });
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
    const onScroll = (e: Event) => {
      if (e.target instanceof Node && f.contains(e.target)) return;
      if (scrollAway.current) scrollAway.current();
      else place();
    };
    place();
    window.addEventListener("resize", place);
    // Capturing: a scroll doesn't bubble, so this is how a scroll in any scrolling parent reaches the document.
    document.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("resize", place);
      document.removeEventListener("scroll", onScroll, true);
    };
    // `rect` is compared by `rectKey`.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a rectangle anchor is a new object on every render; its value is `rectKey`
  }, [ref, rectKey, floating, align]);
  return position
    ? { position: "fixed", left: position.left, top: position.top }
    : { position: "fixed", left: 0, top: 0, visibility: "hidden" };
}
