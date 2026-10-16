// The Ticket Card (*01 · Components → Card component*, *04 · Ticket card variants*; UI-03): key, priority and pin, the
// pill, the title on two lines (the full title in a tooltip), the callout, the Entry line, the answer options, the
// bars, the meta row, the actions and the hover quick actions. An epic child wears the violet sleeve on top
// (*01 · Components → Epic Sleeve*, EPC-09): the epic's name, a pin when the epic is pinned, its position. Everything it says comes from a `TicketCardView`
// (`card_page.rs`); the board (or the gallery) decides the moment of interaction: selected, chain, dragging. The
// lead's markers (d0083, T-0291) bring a light hover card: the unread note, the files both children changed, the
// lead's note after the second stop; and the pill's short text below ~200 px ("Updated", a container query).
import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from "react";

import { Button, PriorityChip, StatusPill, Tooltip, useFloatingStyle } from "../components";
import { TOOLTIP_DELAY } from "../components/Tooltip";
import type { CardAction } from "../generated/CardAction";
import type { CardActionKind } from "../generated/CardActionKind";
import type { CardEntry } from "../generated/CardEntry";
import type { CardHover } from "../generated/CardHover";
import type { CardMetaItem } from "../generated/CardMetaItem";
import type { CardSleeve } from "../generated/CardSleeve";
import type { TicketCardView } from "../generated/TicketCardView";
import { Icon } from "../theme/Icon";
import { barInk, type CardState, outlineColor, RESTING, toneInk } from "./look";

export type Density = "comfortable" | "compact";

export interface TicketCardProps {
  card: TicketCardView;
  density?: Density;
  state?: CardState;
  /** A click on the card (selects it). */
  onSelect?: () => void;
  /** A button on the card: an action, an answer option, "Deliver now", the `▶ :4020` chip. */
  onAction?: (kind: CardActionKind) => void;
  /** The quick action "Open panel". */
  onOpenPanel?: () => void;
}

const ink = (fg: string, bg?: string) => ({ "--ink": fg, "--ink-bg": bg }) as CSSProperties;

/** The light hover card (*04 → Row · Unread notes & same file → HOVER*, *03 → 25*): an icon and a title, then short
 * sections, each with an optional mono eyebrow. */
export function HoverCard({ hover }: { hover: CardHover }) {
  return (
    <>
      <div className="card-hover-title">
        <span style={{ color: toneInk(hover.tone).fg }}>
          <Icon name={hover.icon} size={12} />
        </span>
        <strong>{hover.title}</strong>
      </div>
      {hover.sections.map((section) => (
        <div key={`${section.label ?? ""}${section.text}`} className="card-hover-section">
          {section.label !== null && <span className="card-hover-label">{section.label}</span>}
          <span className="card-hover-text" data-muted={section.muted || undefined}>
            {section.text}
          </span>
        </div>
      ))}
    </>
  );
}

/** The floating card, placed next to its anchor once it is mounted. */
function HoverBubble({ hover, anchor }: { hover: CardHover; anchor: RefObject<HTMLElement | null> }) {
  const card = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, card);
  return (
    <div ref={card} role="tooltip" className="card-hover" style={style}>
      <HoverCard hover={hover} />
    </div>
  );
}

/** Shows `hover` after a short rest of the pointer on (or focus in) what it wraps; Esc, leave or blur hide it. */
function WithHover({ hover, children }: { hover: CardHover | null; children: ReactNode }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  if (hover === null) return <>{children}</>;
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
  return (
    <span
      ref={anchor}
      className="card-hover-anchor"
      role="presentation"
      onMouseEnter={() => {
        show(TOOLTIP_DELAY);
      }}
      onMouseLeave={hide}
      onFocus={() => {
        show(0);
      }}
      onBlur={hide}
      onKeyDown={(e) => {
        if (e.key === "Escape") hide();
      }}
    >
      {children}
      {open && <HoverBubble hover={hover} anchor={anchor} />}
    </span>
  );
}

function EntryRow({ entry }: { entry: CardEntry }) {
  return (
    <WithHover hover={entry.hover}>
      <div className="card-entry" style={ink(toneInk(entry.tone).fg, toneInk(entry.text_tone).fg)}>
        <Icon name={entry.icon} size={12} />
        <span>{entry.text}</span>
      </div>
    </WithHover>
  );
}

function Meta({ item, onAction }: { item: CardMetaItem; onAction?: TicketCardProps["onAction"] }) {
  const chip = (
    <span className="card-meta-item" style={ink(toneInk(item.tone).fg)}>
      <Icon name={item.icon} size={11} />
      <span>{item.text}</span>
    </span>
  );
  // The `▶ :4020` chip opens Open / Stop (RUN-01).
  if (item.icon === "play")
    return (
      <button
        type="button"
        className="card-run-chip"
        onClick={(e) => {
          e.stopPropagation();
          onAction?.("run_chip");
        }}
      >
        {chip}
      </button>
    );
  if (item.hover !== null) return <WithHover hover={item.hover}>{chip}</WithHover>;
  return item.tooltip === null ? chip : <Tooltip text={item.tooltip}>{chip}</Tooltip>;
}

function Actions({
  actions,
  onAction,
}: {
  actions: readonly CardAction[];
  onAction?: TicketCardProps["onAction"];
}) {
  return (
    <div className="card-actions">
      {actions.map((a) => (
        // The card's own click must not fire: the button's wrapper stops it.
        // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- only stops the click from reaching the card; the button inside is the control
        <span
          key={a.label}
          className="card-action"
          onClick={(e) => {
            e.stopPropagation();
          }}
        >
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
  );
}

function QuickActions({ card, onOpenPanel }: { card: TicketCardView; onOpenPanel?: () => void }) {
  return (
    <div className="card-quick">
      <span className="card-quick-icon">
        <Icon name="square-terminal" size={13} />
      </span>
      <Tooltip text={card.open_panel} keys={card.open_panel_keys}>
        <button
          type="button"
          className="card-quick-button"
          aria-label={card.open_panel}
          onClick={(e) => {
            e.stopPropagation();
            onOpenPanel?.();
          }}
        >
          <Icon name="panel-right" size={13} />
        </button>
      </Tooltip>
    </div>
  );
}

/** The sleeve's eyebrow: the name gives way with …, the position never does (*1280 × 800 → Sleeve*). */
function SleeveBand({ sleeve }: { sleeve: CardSleeve }) {
  return (
    <div className="card-sleeve-band">
      <Icon name="layers" size={11} />
      <span className="card-sleeve-name">{sleeve.name}</span>
      {sleeve.pinned && (
        <span className="card-sleeve-pin">
          <Icon name="pin" size={10} />
        </span>
      )}
      <span className="card-spacer" />
      <span className="card-sleeve-position">{sleeve.position}</span>
    </div>
  );
}

function Compact({ card }: { card: TicketCardView }) {
  const { flag, value } = card.compact;
  return (
    <>
      <div className="card-head">
        <span className="card-key">{card.key_label}</span>
        {flag && (
          <Tooltip text={flag.label}>
            <span className="status-pill card-flag" data-palette={flag.palette}>
              <Icon name={flag.icon} size={10} label={flag.label} />
            </span>
          </Tooltip>
        )}
        <span className="card-spacer" />
        {value && (
          <span className="card-compact-value" style={ink(toneInk(value.tone).fg)}>
            {value.icon !== null && <Icon name={value.icon} size={10} />}
            {value.text}
          </span>
        )}
      </div>
      <Tooltip text={card.title}>
        <div className="card-title">{card.title}</div>
      </Tooltip>
    </>
  );
}

function Comfortable({ card, onAction }: Pick<TicketCardProps, "card" | "onAction">) {
  return (
    <>
      <div className="card-head">
        <span className="card-key">{card.key_label}</span>
        {/* *04 → Row · priority & Ready for Dev*: the priority sits after the key, then the pin. */}
        {card.priority && <PriorityChip priority={card.priority} />}
        {card.pinned && (
          <span className="card-pin">
            <Icon name="pin" size={11} />
          </span>
        )}
        <span className="card-spacer" />
        {card.pill && card.pill_short === null && <StatusPill pill={card.pill} />}
        {/* "Updated by lead" reads "Updated" in a narrow column (*Notes · short texts*). */}
        {card.pill && card.pill_short !== null && (
          <>
            <span className="card-pill-long">
              <StatusPill pill={card.pill} />
            </span>
            <span className="card-pill-short">
              <StatusPill pill={{ ...card.pill, label: card.pill_short }} />
            </span>
          </>
        )}
      </div>
      <Tooltip text={card.title}>
        <div className="card-title">{card.title}</div>
      </Tooltip>
      {card.callout && (
        <div
          className="card-callout"
          style={ink(toneInk(card.callout.tone).fg, toneInk(card.callout.tone).bg)}
        >
          <Icon name={card.callout.icon} size={12} />
          <span className="card-callout-text" data-mono={card.callout.mono || undefined}>
            {card.callout.text}
          </span>
        </div>
      )}
      {card.entry && <EntryRow entry={card.entry} />}
      {card.options.length > 0 && (
        <div className="card-options">
          {card.options.map((option) => (
            <button
              key={option.number}
              type="button"
              className="card-option"
              data-picked={option.picked || undefined}
              data-other={option.other || undefined}
              onClick={(e) => {
                e.stopPropagation();
                onAction?.(option.action);
              }}
            >
              <span className="card-option-number">{option.number}</span>
              <span className="card-option-label">{option.label}</span>
              {option.recommended !== null && (
                <span className="card-option-rec">
                  <Icon name="star" size={9} />
                  {option.recommended}
                </span>
              )}
            </button>
          ))}
        </div>
      )}
      {card.bars.map((bar) => {
        const colors = barInk(bar.look);
        return (
          <div key={bar.text} className="card-bar" data-look={bar.look} style={ink(colors.fg, colors.bg)}>
            <Icon name={bar.icon} size={12} />
            <span className="card-bar-text">{bar.text}</span>
            {bar.action && (
              <button
                type="button"
                className="card-bar-action"
                onClick={(e) => {
                  e.stopPropagation();
                  if (bar.action) onAction?.(bar.action.kind);
                }}
              >
                {bar.action.label}
              </button>
            )}
          </div>
        );
      })}
      {card.meta.length > 0 && (
        <div className="card-meta">
          {card.meta.map((item) => (
            <Meta key={`${item.icon}-${item.text}`} item={item} {...(onAction ? { onAction } : {})} />
          ))}
        </div>
      )}
      {card.actions.length > 0 && <Actions actions={card.actions} {...(onAction ? { onAction } : {})} />}
    </>
  );
}

export function TicketCard({
  card,
  density = "comfortable",
  state = RESTING,
  onSelect,
  onAction,
  onOpenPanel,
}: TicketCardProps) {
  const compact = density === "compact";
  const opacity = state.chain === "dimmed" ? 0.35 : (card.faded ?? undefined);
  const sleeve = card.sleeve;
  const article = (
    // The card is a pointer target; the board's keyboard moves the selection (T-0220), so it isn't a tab stop.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events -- the board's own keys select and open cards (SDD §21.3); a card isn't a button
    <article
      className="ticket-card"
      aria-label={`${card.key_label} ${card.title}`}
      data-density={density}
      data-selected={state.selected || undefined}
      data-chain={state.chain === "none" ? undefined : state.chain}
      data-dragging={state.dragging || undefined}
      data-hovered={state.hovered || undefined}
      data-faded={card.faded === null ? undefined : true}
      data-fresh={card.fresh || undefined}
      style={
        {
          "--card-outline": outlineColor(card.outline),
          // In a sleeve the whole sleeve dims or fades with the card.
          opacity: sleeve === null ? opacity : undefined,
        } as CSSProperties
      }
      onClick={sleeve === null ? onSelect : undefined}
    >
      {compact ? <Compact card={card} /> : <Comfortable card={card} {...(onAction ? { onAction } : {})} />}
      {!compact && (onOpenPanel !== undefined || state.hovered) && (
        <QuickActions card={card} {...(onOpenPanel ? { onOpenPanel } : {})} />
      )}
    </article>
  );
  if (sleeve === null) return article;
  return (
    // The band selects the card too, so the sleeve takes the click.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- the board's own keys select and open cards (SDD §21.3); a card isn't a button
    <div className="card-sleeve" data-density={density} style={{ opacity }} onClick={onSelect}>
      <SleeveBand sleeve={sleeve} />
      {article}
    </div>
  );
}
