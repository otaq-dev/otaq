// The usage limit banner with one line per tool (*11 → Row · Limit banner per tool (0086)*, *Move waiting to…*, *Not
// available*; USG-10, AGT-08, d0086): the tool's avatar, its title and text, Resume <tool> or Sign in…, and Move waiting
// to… ▾ with its list of named agents on other tools. A limit line is neutral, a tool that isn't available amber, never
// turquoise (d0077). Every word, which buttons show and the list come from Rust (`limit_banner.rs`); a press goes out
// as the line's `ToolProblemAction` or `limits.move_waiting`.
import { type KeyboardEvent, type RefObject, useEffect, useRef, useState } from "react";

import { Button, ToolAvatar, useFloatingStyle } from "../../components";
import { menuKey, selectable } from "../../components/logic";
import type { LimitLine } from "../../generated/LimitLine";
import type { MoveWaitingMenu } from "../../generated/MoveWaitingMenu";
import type { ToolProblemAction } from "../../generated/ToolProblemAction";
import { Icon } from "../../theme/Icon";
import { moveMenuItems } from "./notices";

/** What the lines' presses do: the app's own (`BannerRows`), or the gallery's recorder. */
export interface LimitLineHandlers {
  onAction: (action: ToolProblemAction) => void;
  onMove: (tool: string, agent: string) => void;
}

export interface MoveWaitingListProps {
  menu: MoveWaitingMenu;
  onChoose: (agent: string) => void;
  /** After a choice or Esc / Tab (`"key"`: focus goes back to the button), or because focus left the list. */
  onClose: (how: "key" | "outside") => void;
  /** Takes focus when it opens; the gallery's open sample doesn't. */
  takeFocus?: boolean;
}

/** The open list: "MOVE 4 WAITING TICKETS TO", then each agent with its avatar, name and "Codex · gpt-5.5". */
export function MoveWaitingList({ menu, onChoose, onClose, takeFocus = true }: MoveWaitingListProps) {
  const root = useRef<HTMLDivElement>(null);
  const items = moveMenuItems(menu);
  const [highlighted, setHighlighted] = useState<number | null>(null);
  useEffect(() => {
    if (takeFocus) root.current?.focus();
  }, [takeFocus]);
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
  return (
    <div
      ref={root}
      className="menu notices-move-list"
      role="menu"
      aria-label={menu.title}
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) onClose("outside");
      }}
    >
      <div className="menu-header" role="presentation">
        {menu.title}
      </div>
      {menu.items.map((item, n) => {
        // The title is row 0 of the keyboard's rows.
        const i = n + 1;
        return (
          <button
            key={item.agent}
            type="button"
            role="menuitem"
            tabIndex={-1}
            className="menu-item notices-move-item"
            data-highlighted={i === highlighted || undefined}
            onMouseDown={(e) => {
              e.preventDefault();
            }}
            onMouseMove={() => {
              if (selectable(items[i]) && i !== highlighted) setHighlighted(i);
            }}
            onClick={() => {
              onChoose(item.agent);
              onClose("key");
            }}
          >
            <span className="menu-item-line">
              <ToolAvatar avatar={item.avatar} />
              <span className="notices-move-name">{item.name}</span>
              <span className="notices-move-detail">{item.detail}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** Move waiting to… ▾ and, while open, its list under it. */
export function MoveWaitingButton({
  menu,
  onMove,
}: {
  menu: MoveWaitingMenu;
  onMove: (tool: string, agent: string) => void;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="button notices-move"
        data-variant="secondary"
        data-compact
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name="refresh-cw" />
        <span>{menu.label}</span>
        <Icon name="chevron-down" size={11} />
      </button>
      {open && (
        <AnchoredList
          anchor={trigger}
          menu={menu}
          onChoose={(agent) => {
            onMove(menu.tool, agent);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") trigger.current?.focus();
          }}
        />
      )}
    </>
  );
}

function AnchoredList({
  anchor,
  ...list
}: MoveWaitingListProps & { anchor: RefObject<HTMLButtonElement | null> }) {
  const floating = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, floating, "end", () => {
    list.onClose("outside");
  });
  return (
    <div ref={floating} className="floating" style={style}>
      <MoveWaitingList {...list} />
    </div>
  );
}

/** One tool's line. */
export function LimitLineRow({ line, onAction, onMove }: { line: LimitLine } & LimitLineHandlers) {
  return (
    <div className="banner notices-line" data-tone={line.tone} data-tool={line.tool} role="status">
      {line.avatar ? (
        <ToolAvatar avatar={line.avatar} />
      ) : (
        <span className="banner-icon">
          <Icon name={line.icon} size={15} />
        </span>
      )}
      <span className="banner-title">{line.title}</span>
      <span className="banner-text">{line.text}</span>
      {line.buttons.map((button) => (
        <Button
          key={button.label}
          compact
          variant={button.primary ? "primary" : "secondary"}
          label={button.label}
          icon={button.icon}
          disabled={!button.enabled}
          onClick={() => {
            onAction(button.action);
          }}
        />
      ))}
      {line.move_waiting && <MoveWaitingButton menu={line.move_waiting} onMove={onMove} />}
    </div>
  );
}

/** The banner's lines, top first, in the order the tools hit their limits. */
export function LimitLines({ lines, onAction, onMove }: { lines: readonly LimitLine[] } & LimitLineHandlers) {
  return (
    <div className="notices-lines">
      {lines.map((line) => (
        <LimitLineRow key={line.tool + line.title} line={line} onAction={onAction} onMove={onMove} />
      ))}
    </div>
  );
}
