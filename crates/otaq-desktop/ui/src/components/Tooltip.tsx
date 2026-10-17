// Tooltip (*01 · Components → Overlays & feedback*): always dark, in both themes; the action's shortcut after the
// text, or a bold title with detail lines and a dimmer note (*02 → Header · running chip tooltip*, *Activity ·
// hover*). A short tooltip may lead with a small coloured icon (*05 → c · Copied state*: a green check before
// "Copied"). It shows after a short hover or on keyboard focus, and goes on leave, blur or Esc.
import { type ReactNode, type RefObject, useEffect, useId, useRef, useState } from "react";

import type { IconName } from "../generated/IconName";
import { Icon } from "../theme/Icon";
import { useFloatingStyle } from "./Floating";

/** How long the pointer rests before a tooltip shows, in ms. */
export const TOOLTIP_DELAY = 400;

export interface TooltipProps {
  text: string;
  /** The shortcut, as a Kbd-like hint after the text. */
  keys?: string | undefined;
  /** An icon before the text, in the tooltip's success green (a check before "Copied"). */
  icon?: IconName | undefined;
  /** Lines under a bold `text`; the last may be a dimmer `note`. */
  detail?: readonly string[];
  note?: string;
  children: ReactNode;
}

function Bubble({
  id,
  anchor,
  text,
  keys,
  icon,
  detail,
  note,
}: Omit<TooltipProps, "children"> & {
  id: string;
  anchor: RefObject<HTMLElement | null>;
}) {
  const bubble = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, bubble);
  const rich = detail !== undefined || note !== undefined;
  return (
    <div ref={bubble} id={id} role="tooltip" className="tooltip" data-rich={rich || undefined} style={style}>
      {rich ? (
        <>
          <strong>{text}</strong>
          {detail?.map((line) => (
            <span key={line}>{line}</span>
          ))}
          {note !== undefined && <span className="tooltip-note">{note}</span>}
        </>
      ) : (
        <>
          {icon !== undefined && <Icon name={icon} size={12} />}
          <span>{text}</span>
          {keys !== undefined && <kbd className="tooltip-keys">{keys}</kbd>}
        </>
      )}
    </div>
  );
}

/** Wraps the element it explains. */
export function Tooltip({ children, ...bubble }: TooltipProps) {
  const id = useId();
  const anchor = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const show = (delay: number) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      setOpen(true);
    }, delay);
  };
  const hide = () => {
    window.clearTimeout(timer.current);
    setOpen(false);
  };
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);
  return (
    <span
      ref={anchor}
      className="tooltip-anchor"
      // Only listens for hover and focus of what it wraps; the wrapped element keeps its own role.
      role="presentation"
      onMouseEnter={() => {
        show(TOOLTIP_DELAY);
      }}
      onMouseLeave={hide}
      onFocus={() => {
        show(0);
      }}
      onBlur={hide}
    >
      {children}
      {open && <Bubble id={id} anchor={anchor} {...bubble} />}
    </span>
  );
}
