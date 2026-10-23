// Enter and leave animations (OT-6), played with the Web Animations API so no layer's markup changes. Durations and
// easings come from the page's motion tokens (`theme/motion.css`). Reduced motion turns every slide and scale into a
// short fade.
//
// Leaving: React removes a closed layer at once, so `useEnterExit` leaves an inert copy in its place, plays the leave
// animation on the copy and removes it after. A layer that must keep drawing while it leaves (the sheet and its
// terminals) stays mounted instead, with `usePresence`.
import { type RefObject, useCallback, useLayoutEffect, useState } from "react";

/** How a layer comes and goes. */
export type MotionPreset = "panel" | "dialog" | "backdrop" | "floating" | "toast";

/** Each preset's hidden end; the shown end is opacity 1 with no transform. */
const HIDDEN: Record<MotionPreset, Keyframe> = {
  // A drawer: all the way in from the window's right edge, solid the whole way (a fade made it read as appearing).
  panel: { transform: "translateX(100%)" },
  dialog: { opacity: 0, transform: "scale(0.96)" },
  backdrop: { opacity: 0 },
  floating: { opacity: 0, transform: "translateY(-4px)" },
  toast: { opacity: 0, transform: "translateY(8px)" },
};

type Duration = "--motion-fast" | "--motion-base" | "--motion-panel";

/** Each preset's duration in and out: the panel travels its whole width, so it takes longer in than out. */
const DURATION: Record<MotionPreset, { in: Duration; out: Duration }> = {
  panel: { in: "--motion-panel", out: "--motion-base" },
  dialog: { in: "--motion-base", out: "--motion-base" },
  backdrop: { in: "--motion-base", out: "--motion-base" },
  floating: { in: "--motion-fast", out: "--motion-fast" },
  toast: { in: "--motion-base", out: "--motion-base" },
};

/** What leaves gathers speed on its way off the edge; the rest eases in and out. */
const LEAVE_EASING: Partial<Record<MotionPreset, string>> = { panel: "--ease-in" };

export function reducedMotion(): boolean {
  return (
    typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

/** A CSS time ("120ms", "0.2s") in milliseconds; 0 when it isn't one. */
export function cssTimeMs(value: string): number {
  const v = value.trim();
  const n = parseFloat(v);
  if (Number.isNaN(n)) return 0;
  return v.endsWith("ms") ? n : v.endsWith("s") ? n * 1000 : 0;
}

/** Plays `preset` on `el`, in or out; `null` where the page can't animate or the duration is 0. */
export function playMotion(el: HTMLElement, preset: MotionPreset, direction: "in" | "out"): Animation | null {
  if (typeof el.animate !== "function") return null;
  const reduce = reducedMotion();
  const style = getComputedStyle(el);
  const duration = cssTimeMs(style.getPropertyValue(reduce ? "--motion-fast" : DURATION[preset][direction]));
  if (duration === 0) return null;
  const hidden = reduce ? { opacity: 0 } : HIDDEN[preset];
  const shown: Keyframe = {
    ...("opacity" in hidden && { opacity: 1 }),
    ...("transform" in hidden && { transform: "none" }),
  };
  const easingToken = direction === "in" ? "--ease-out" : (LEAVE_EASING[preset] ?? "--ease-in-out");
  const easing = style.getPropertyValue(easingToken).trim() || "ease";
  return el.animate(direction === "in" ? [hidden, shown] : [shown, hidden], {
    duration,
    easing: reduce ? "linear" : easing,
    // A leaving copy stays hidden for the frame before it is removed.
    fill: direction === "out" ? "forwards" : "none",
  });
}

/** A part of the layer with its own preset: the dialog box inside its backdrop. */
export interface InnerMotion {
  selector: string;
  preset: MotionPreset;
}

function playAll(
  el: HTMLElement,
  preset: MotionPreset,
  direction: "in" | "out",
  inner?: InnerMotion,
): Animation[] {
  const parts: [HTMLElement, MotionPreset][] = [[el, preset]];
  const child = inner ? el.querySelector<HTMLElement>(inner.selector) : null;
  if (inner && child) parts.push([child, inner.preset]);
  return parts.flatMap(([part, p]) => playMotion(part, p, direction) ?? []);
}

/** Puts an inert copy of `el` where it was and plays the leave animation on it, then removes it. */
function leaveCopy(el: HTMLElement, parent: Node, preset: MotionPreset, inner?: InnerMotion): void {
  // Already gone with its parent (a whole screen closed): nothing is left to see.
  if (!el.isConnected && !parent.isConnected) return;
  const copy = el.cloneNode(true) as HTMLElement;
  for (const node of [copy, ...copy.querySelectorAll("[id]")]) node.removeAttribute("id");
  copy.inert = true;
  copy.setAttribute("aria-hidden", "true");
  copy.setAttribute("data-leaving", "");
  copy.style.pointerEvents = "none";
  if (el.isConnected) el.after(copy);
  else parent.appendChild(copy);
  const animations = playAll(copy, preset, "out", inner);
  if (animations.length === 0) {
    copy.remove();
    return;
  }
  const remove = () => {
    copy.remove();
  };
  Promise.all(animations.map((a) => a.finished)).then(remove, remove);
}

/**
 * Plays `preset` in when the element behind `ref` mounts, and out on a copy left in its place when it unmounts. The
 * copy is a picture only (inert, hidden from assistive tech, no ids), so tests and code that look for the layer don't
 * find it after it closed.
 */
export function useEnterExit(
  ref: RefObject<HTMLElement | null>,
  preset: MotionPreset,
  inner?: InnerMotion,
): void {
  // A layout effect: its cleanup runs while the element is still in the page, before React removes it.
  useLayoutEffect(() => {
    const el = ref.current;
    const parent = el?.parentNode;
    if (!el || !parent) return;
    playAll(el, preset, "in", inner);
    return () => {
      leaveCopy(el, parent, preset, inner);
    };
    // Played once per mount; a preset is fixed for a layer.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the enter plays on mount and the leave on unmount only
  }, []);
}

/**
 * Keeps a layer mounted while it leaves: `mounted` stays true after `open` turns false until `done` is called (by
 * `playLeave` when the leave animation ends). Opening again meanwhile keeps it, without a new enter.
 */
export function usePresence(open: boolean): { mounted: boolean; leaving: boolean; done: () => void } {
  const [mounted, setMounted] = useState(open);
  // Opening mounts at once (set while rendering, React's "storing information from previous renders").
  if (open && !mounted) setMounted(true);
  const done = useCallback(() => {
    setMounted(false);
  }, []);
  return { mounted: mounted || open, leaving: mounted && !open, done };
}

/** How long a layer waits for its content before it comes in anyway, in ms. */
export const ENTER_WAIT = 400;

/**
 * Plays `preset` in on `el` once its content is ready: the sheet's slot shows `[data-loading]` while the ticket
 * loads, and sliding in an empty panel that then fills read as a jump. Until then (at most `ENTER_WAIT`) the layer is
 * hidden. Returns the cleanup: it stops waiting and shows the layer.
 */
export function playEnterWhenReady(el: HTMLElement, preset: MotionPreset): () => void {
  const loading = () => el.querySelector(":scope > [data-loading]") !== null;
  if (!loading()) {
    playMotion(el, preset, "in");
    return () => {};
  }
  el.style.visibility = "hidden";
  let waiting = true;
  const enter = () => {
    if (!waiting) return;
    stop();
    playMotion(el, preset, "in");
  };
  const observer = new MutationObserver(() => {
    if (!loading()) enter();
  });
  observer.observe(el, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["data-loading"],
  });
  const timer = window.setTimeout(enter, ENTER_WAIT);
  function stop() {
    waiting = false;
    observer.disconnect();
    window.clearTimeout(timer);
    el.style.visibility = "";
  }
  return () => {
    if (waiting) stop();
  };
}

/** Plays the leave on a mounted layer and calls `done` when it ends (at once when nothing plays). */
export function playLeave(el: HTMLElement | null, preset: MotionPreset, done: () => void): () => void {
  const animation = el ? playMotion(el, preset, "out") : null;
  if (!animation) {
    done();
    return () => {};
  }
  let cancelled = false;
  animation.finished.then(
    () => {
      if (!cancelled) done();
    },
    () => {},
  );
  return () => {
    cancelled = true;
    animation.cancel();
  };
}
