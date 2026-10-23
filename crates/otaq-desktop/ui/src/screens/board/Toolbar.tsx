// The board's toolbar, in both views (BRD-01; *24 · The two views*): Board · List first (`g b`, `g l`), then the
// Board's Lanes None · Epic or the List's Group by, the Fold empty columns (or Hide empty stages) switch, All tickets
// and New ticket. Every word and every change it sends is Rust's (`BoardToolbar`, `view_prefs.rs`); a click sends
// the option's change (`board.set_view`) and the board asks for its view again.
import type { KeyboardEvent } from "react";

import { Tooltip } from "../../components";
import type { BoardToolbar } from "../../generated/BoardToolbar";
import type { ToolbarGroup } from "../../generated/ToolbarGroup";
import type { ToolbarOption } from "../../generated/ToolbarOption";
import type { ViewChange } from "../../generated/ViewChange";
import { Icon } from "../../theme/Icon";

export interface ToolbarProps {
  toolbar: BoardToolbar;
  onView: (change: ViewChange) => void;
  onAllTickets: () => void;
  onNewTicket: () => void;
}

/** ←/→ on a segment: the next one, wrapping, chosen at once (a radio group). */
export function stepSegment(key: string, selected: number, count: number): number | null {
  if (count === 0) return null;
  if (key === "ArrowRight" || key === "ArrowDown") return (selected + 1) % count;
  if (key === "ArrowLeft" || key === "ArrowUp") return (selected - 1 + count) % count;
  return null;
}

function Segment({ option, onView }: { option: ToolbarOption; onView: ToolbarProps["onView"] }) {
  const button = (
    <button
      type="button"
      role="radio"
      className="board-segment"
      aria-checked={option.selected}
      tabIndex={option.selected ? 0 : -1}
      onClick={() => {
        if (!option.selected) onView(option.change);
      }}
    >
      {option.icon !== null && <Icon name={option.icon} size={14} />}
      <span>{option.label}</span>
    </button>
  );
  return option.tooltip === null ? (
    button
  ) : (
    <Tooltip text={option.tooltip} {...(option.keys === null ? {} : { keys: option.keys })}>
      {button}
    </Tooltip>
  );
}

function Segments({
  group,
  name,
  onView,
}: {
  group: ToolbarGroup;
  name: string;
  onView: ToolbarProps["onView"];
}) {
  const selected = Math.max(
    0,
    group.options.findIndex((o) => o.selected),
  );
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const next = stepSegment(e.key, selected, group.options.length);
    const option = next === null ? undefined : group.options[next];
    if (option === undefined) return;
    e.preventDefault();
    onView(option.change);
  };
  return (
    <>
      {group.label !== null && <span className="board-toolbar-label">{group.label}</span>}
      {/* eslint-disable-next-line jsx-a11y/interactive-supports-focus -- its radios take the focus */}
      <div
        className="board-segments"
        role="radiogroup"
        aria-label={group.label ?? name}
        onKeyDown={onKeyDown}
      >
        {group.options.map((option) => (
          <Segment key={option.label} option={option} onView={onView} />
        ))}
      </div>
    </>
  );
}

export function Toolbar({ toolbar, onView, onAllTickets, onNewTicket }: ToolbarProps) {
  const choices = toolbar.lanes ?? toolbar.group_by;
  return (
    <div className="board-toolbar" role="toolbar" aria-label="Board">
      <Segments group={toolbar.views} name="View" onView={onView} />
      {choices && (
        <>
          <span className="board-toolbar-divider" aria-hidden />
          <Segments group={choices} name={choices.label ?? ""} onView={onView} />
        </>
      )}
      <span className="board-toolbar-spacer" />
      <button
        type="button"
        role="switch"
        className="board-chip-button"
        aria-checked={toolbar.switch.selected}
        onClick={() => {
          onView(toolbar.switch.change);
        }}
      >
        {toolbar.switch.icon !== null && <Icon name={toolbar.switch.icon} size={12} />}
        <span>{toolbar.switch.label}</span>
      </button>
      <button type="button" className="board-chip-button" onClick={onAllTickets}>
        <span>{toolbar.all_tickets}</span>
        <Icon name="chevron-down" size={12} />
      </button>
      <Tooltip
        text={toolbar.new_ticket}
        {...(toolbar.new_ticket_keys === null ? {} : { keys: toolbar.new_ticket_keys })}
      >
        <button type="button" className="board-new-ticket" onClick={onNewTicket}>
          <Icon name="plus" size={13} />
          <span>{toolbar.new_ticket}</span>
        </button>
      </Tooltip>
    </div>
  );
}
