// Popover (*01 · Components → Overlays & feedback*; e.g. *02 → Header · agent limits ON + inline popover*): a raised
// panel next to its trigger, kept inside the window. Esc or a press outside it (and outside its trigger) closes it.
import { type ReactNode, type RefObject, useEffect, useRef } from "react";

import { type Anchor, useFloatingStyle } from "./Floating";

export interface PopoverProps {
  /** What the popover is, for screen readers. */
  label: string;
  anchor: Anchor;
  align?: "start" | "end";
  onClose: () => void;
  children: ReactNode;
}

/** Calls `onClose` on Esc and on a press outside `inside` (and outside `anchor`, whose own click toggles). */
export function useDismiss(
  inside: RefObject<HTMLElement | null>,
  anchor: Anchor | undefined,
  onClose: () => void,
): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // The popover's Esc, before any handler behind it (the panel's Close).
      e.stopPropagation();
      onClose();
    };
    const onPress = (e: PointerEvent) => {
      const target = e.target as Node | null;
      const trigger = anchor && "current" in anchor ? anchor.current : null;
      if (inside.current?.contains(target) || trigger?.contains(target)) return;
      onClose();
    };
    // Capturing, so the popover's Esc comes before every other key handler of the page.
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("pointerdown", onPress);
    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("pointerdown", onPress);
    };
  }, [inside, anchor, onClose]);
}

export function Popover({ label, anchor, align, onClose, children }: PopoverProps) {
  const floating = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, floating, align);
  useDismiss(floating, anchor, onClose);
  return (
    <div ref={floating} className="popover" role="dialog" aria-label={label} style={style}>
      {children}
    </div>
  );
}
