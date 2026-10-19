// Menus (*01 · Components → Overlays & feedback → Context menu*) and dropdown lists: the rows come from Rust
// (`MenuItem`, e.g. `priority.rs`'s `menu_items`), with an icon, a label, a shortcut, a danger tone, a disabled
// state with a hint, separators and group headers. The menu takes focus when it opens: ↑/↓ move (skipping headers
// and disabled rows, wrapping), Home/End jump, ↵ or Space chooses, Esc, Tab or a click outside closes.
import { type CSSProperties, type KeyboardEvent, useEffect, useId, useRef, useState } from "react";

import type { MenuItem } from "../generated/MenuItem";
import { Icon } from "../theme/Icon";
import { type Anchor, useFloatingStyle } from "./Floating";
import { chooseMenu, menuKey, selectable } from "./logic";

export type MenuClose = "key" | "outside";

export interface MenuProps {
  /** What the menu is, for screen readers ("Card actions"). */
  label: string;
  items: readonly MenuItem[];
  onChoose: (id: string) => void;
  /**
   * The menu should close: after a choice or Esc / Tab (`"key"`: focus may go back to the trigger), or because focus
   * left it (`"outside"`: leave focus where it went).
   */
  onClose: (how: MenuClose) => void;
  /** A dropdown's list: rows are options, the checked one is the current value. */
  list?: boolean;
  width?: number;
  style?: CSSProperties;
}

/** The menu panel itself, where its owner puts it. Most owners want `AnchoredMenu`. */
export function Menu({ label, items, onChoose, onClose, list = false, width = 260, style }: MenuProps) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const [highlighted, setHighlighted] = useState<number | null>(null);
  useEffect(() => {
    root.current?.focus();
  }, []);
  const choose = (index: number | null) => {
    const chosen = chooseMenu(items, index);
    if (chosen === null) return;
    onChoose(chosen);
    onClose("key");
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const result = menuKey(items, highlighted, e.key);
    switch (result.kind) {
      case "highlight":
        setHighlighted(result.index);
        break;
      case "choose":
        onChoose(result.id);
        onClose("key");
        break;
      case "close":
        onClose("key");
        break;
      case "none":
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  };
  const row = (n: number) => `${id}-${String(n)}`;
  return (
    <div
      ref={root}
      className="menu"
      role="menu"
      aria-label={label}
      tabIndex={-1}
      aria-activedescendant={highlighted === null ? undefined : row(highlighted)}
      style={{ width, ...style }}
      onKeyDown={onKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) onClose("outside");
      }}
    >
      {items.map((item, i) => {
        const separator = item.separated && i > 0 && <div role="separator" className="menu-separator" />;
        if (item.header) {
          return (
            <div key={`h-${item.label}`} role="presentation">
              {separator}
              <div className="menu-header">{item.label}</div>
            </div>
          );
        }
        return (
          <div key={item.id} role="presentation">
            {separator}
            <button
              id={row(i)}
              type="button"
              tabIndex={-1}
              role={list ? "menuitemradio" : "menuitem"}
              aria-checked={list ? item.checked : undefined}
              aria-disabled={item.disabled || undefined}
              className="menu-item"
              data-danger={item.danger || undefined}
              data-highlighted={i === highlighted || undefined}
              // Keeps focus on the menu, so the press isn't lost to its blur.
              onMouseDown={(e) => {
                e.preventDefault();
              }}
              onMouseMove={() => {
                if (selectable(item) && i !== highlighted) setHighlighted(i);
              }}
              onClick={() => {
                choose(i);
              }}
            >
              <span className="menu-item-line">
                {item.icon !== null && <Icon name={item.icon} />}
                <span className="menu-item-label">{item.label}</span>
                {item.checked && <Icon name="check" size={13} label="current" />}
                {item.shortcut !== null && <span className="menu-item-keys">{item.shortcut}</span>}
              </span>
              {item.hint !== null && (
                <span className="menu-item-hint" data-indented={item.icon !== null || undefined}>
                  {item.hint}
                </span>
              )}
            </button>
          </div>
        );
      })}
    </div>
  );
}

export interface AnchoredMenuProps extends MenuProps {
  /** The element it opens from (under it), or the pointer's point for a context menu. */
  anchor: Anchor;
  align?: "start" | "end";
}

/**
 * A menu under its trigger, or at the pointer, kept inside the window. A press outside it (and outside its trigger,
 * whose own click decides) closes it too: a press on the window's drag region (the header) moves no focus, so the
 * menu's blur alone would leave it open.
 */
export function AnchoredMenu({ anchor, align, ...menu }: AnchoredMenuProps) {
  const floating = useRef<HTMLDivElement>(null);
  const close = useRef(menu.onClose);
  useEffect(() => {
    close.current = menu.onClose;
  });
  const style = useFloatingStyle(anchor, floating, align, () => {
    close.current("outside");
  });
  useEffect(() => {
    const onPress = (e: PointerEvent) => {
      const target = e.target as Node | null;
      const trigger = "current" in anchor ? anchor.current : null;
      if (floating.current?.contains(target) || trigger?.contains(target)) return;
      close.current("outside");
    };
    document.addEventListener("pointerdown", onPress, true);
    return () => {
      document.removeEventListener("pointerdown", onPress, true);
    };
  }, [anchor]);
  return (
    <div ref={floating} className="floating" style={style}>
      <Menu {...menu} />
    </div>
  );
}
