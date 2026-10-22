// Popover (*01 · Components → Overlays & feedback*; e.g. *02 → Header · agent limits ON + inline popover*): a raised
// panel next to its trigger, kept inside the window. Esc or a press outside it (and outside its trigger) closes it.
import { type ReactNode, type RefObject, useEffect, useRef } from "react";

import { type Anchor, useFloatingStyle, usePressOutside } from "./Floating";

export interface PopoverProps {
  /** What the popover is, for screen readers. */
  label: string;
  anchor: Anchor;
  align?: "start" | "end";
  onClose: () => void;
  children: ReactNode;
}

/** Calls `onClose` on Esc, before any other key handler of the page (the panel's Close). */
export function useEscape(onClose: () => void): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    // Capturing, so the popover's Esc comes before every other key handler of the page.
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("keydown", onKey, true);
    };
  }, [onClose]);
}

/**
 * Calls `onClose` on Esc and on a press outside `inside` (and outside `anchor`, whose own click toggles): for a box
 * that isn't a floating layer (the quick switcher). A floating layer gets the press from `useFloatingStyle`'s
 * `onDismiss` and needs only `useEscape`.
 */
export function useDismiss(
  inside: RefObject<HTMLElement | null>,
  anchor: Anchor | undefined,
  onClose: () => void,
): void {
  useEscape(onClose);
  usePressOutside(inside, anchor && "current" in anchor ? anchor : null, onClose);
}

export function Popover({ label, anchor, align, onClose, children }: PopoverProps) {
  const floating = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, floating, align, onClose);
  useEscape(onClose);
  return (
    <div ref={floating} className="popover" role="dialog" aria-label={label} style={style}>
      {children}
    </div>
  );
}
