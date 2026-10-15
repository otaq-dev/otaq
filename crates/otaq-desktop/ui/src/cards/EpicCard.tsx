// The Epic Card (*01 · Components → Card component → Epic Card*; *04 → Row · epics*, *Row · epics as delivery units
// (0059)*; EPC-01): key, External ID, priority, the epic pill, the title, progress with one segment per child, the
// branch, the child list (a click selects that child), "Add ticket to epic" and the actions of its later states. It
// draws an `EpicCardView` and shares the Ticket Card's states; whether its list is folded is the page's own state.
// A row Rust marks draggable shows a grip on hover and is dragged out on its own (EPC-12; *04 → Backlog · epic ·
// drag one row out*, *03 → epic flows 3 → 8*).
import { type CSSProperties, type PointerEvent, useState } from "react";

import { Button, PriorityChip, StatusPill } from "../components";
import type { CardActionKind } from "../generated/CardActionKind";
import type { EpicCardView } from "../generated/EpicCardView";
import { Icon } from "../theme/Icon";
import { type CardState, childInk, RESTING, segmentFill } from "./look";

export interface EpicCardProps {
  card: EpicCardView;
  state?: CardState;
  /** Starts folded (the chevron unfolds it). */
  collapsed?: boolean;
  onSelect?: () => void;
  /** A child row: select that ticket. */
  onChild?: (key: string) => void;
  /** The button went down on a draggable row: the board may start dragging that child out. */
  onChildPress?: (key: string, e: PointerEvent) => void;
  onAdd?: () => void;
  onAction?: (kind: CardActionKind) => void;
}

/** Keeps a click inside the card from also selecting the card. */
const own = (f?: () => void) => (e: { stopPropagation: () => void }) => {
  e.stopPropagation();
  f?.();
};

export function EpicCard({
  card,
  state = RESTING,
  collapsed: initiallyCollapsed = false,
  onSelect,
  onChild,
  onChildPress,
  onAdd,
  onAction,
}: EpicCardProps) {
  const [collapsed, setCollapsed] = useState(initiallyCollapsed);
  return (
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-noninteractive-element-interactions -- as the Ticket Card: the board's own keys select and open cards
    <article
      className="epic-card"
      aria-label={`${card.key} ${card.title}`}
      data-selected={state.selected || undefined}
      data-chain={state.chain === "none" ? undefined : state.chain}
      data-dragging={state.dragging || undefined}
      data-hovered={state.hovered || undefined}
      data-faded={card.faded === null ? undefined : true}
      style={{ opacity: state.chain === "dimmed" ? 0.35 : (card.faded ?? undefined) }}
      onClick={onSelect}
    >
      <div className="card-head">
        {card.pinned && (
          <span className="card-pin">
            <Icon name="pin" size={11} />
          </span>
        )}
        <span className="card-key">{card.key}</span>
        {card.external_id !== null && <span className="card-external-id">{card.external_id}</span>}
        {card.priority && <PriorityChip priority={card.priority} />}
        <span className="card-spacer" />
        <StatusPill pill={card.pill} />
        <button
          type="button"
          className="epic-toggle"
          aria-expanded={!collapsed}
          aria-label={card.title}
          onClick={own(() => {
            setCollapsed((c) => !c);
          })}
        >
          <Icon name={collapsed ? "chevron-right" : "chevron-down"} size={14} />
        </button>
      </div>
      <div className="card-title" data-epic>
        {card.title}
      </div>
      <div className="epic-progress">
        <span className="epic-segments">
          {card.segments.map((s, i) => (
            // The segments are positional: one per child, in key order.
            <span key={i} className="epic-segment" style={{ background: segmentFill(s) }} />
          ))}
        </span>
        <span className="epic-progress-text">{card.progress}</span>
      </div>
      <div className="epic-branch">
        <Icon name="git-branch" size={11} />
        <span>{card.branch}</span>
      </div>
      {!collapsed && card.children.length > 0 && (
        <div className="epic-children">
          {card.children.map((child) => (
            <button
              key={child.key}
              type="button"
              className="epic-child"
              data-finished={child.state === "merged" || child.state === "closed" || undefined}
              data-draggable={child.draggable || undefined}
              style={{ "--ink": childInk(child.state) } as CSSProperties}
              onClick={own(() => onChild?.(child.key))}
              onPointerDown={
                child.draggable && onChildPress
                  ? (e) => {
                      // The row is dragged, not the whole card.
                      e.stopPropagation();
                      onChildPress(child.key, e);
                    }
                  : undefined
              }
            >
              {child.draggable && (
                <span className="epic-child-grip">
                  <Icon name="grip-vertical" size={10} />
                </span>
              )}
              <Icon name={child.icon} size={11} />
              <span className="epic-child-key">{child.key}</span>
              <span className="epic-child-title">{child.title}</span>
              <span className="epic-child-status">{child.status_label}</span>
            </button>
          ))}
        </div>
      )}
      {!collapsed && card.add !== null && (
        <button type="button" className="epic-add" onClick={own(onAdd)}>
          <Icon name="plus" size={11} />
          <span>{card.add}</span>
        </button>
      )}
      {card.actions.length > 0 && (
        <div className="card-actions">
          {card.actions.map((a) => (
            // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- only stops the click from reaching the card; the button inside is the control
            <span key={a.label} className="card-action" onClick={own()}>
              <Button
                label={a.label}
                variant={a.primary ? "primary" : "secondary"}
                compact
                {...(a.icon === null ? {} : { icon: a.icon })}
                onClick={() => onAction?.(a.kind)}
              />
            </span>
          ))}
        </div>
      )}
    </article>
  );
}
