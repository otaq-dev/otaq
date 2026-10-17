// The Ticket Card v2 (*01 · Components → Ticket Card v2*, *04 · Ticket card variants → Ticket Card v2 · all
// variants*; SHL-07): the head (tool avatar when shown, key, pin, priority, one short flag), the title on two lines
// (the full title in a tooltip), the situation line with its coloured edge, the origin line, the answer options and
// the footer (tokens, last signal, one action). An epic child wears the violet sleeve on top (*Epic Sleeve v2*,
// EPC-09). Everything it says comes from a `TicketCardView` (`card_page.rs`); the board (or the gallery) decides the
// moment of interaction: hovered, selected, chain, dragging (*Interaction states*). Resting on the head shows the full
// status and what the card has no room for; the quick actions (open terminal, open panel) show on hover.
import { type CSSProperties, type ReactNode, type RefObject, useEffect, useRef, useState } from "react";

import { Button, PriorityChip, StatusPill, ToolAvatar, Tooltip, useFloatingStyle } from "../components";
import { TOOLTIP_DELAY } from "../components/Tooltip";
import type { CardActionKind } from "../generated/CardActionKind";
import type { CardFooter } from "../generated/CardFooter";
import type { CardHover } from "../generated/CardHover";
import type { CardSituation } from "../generated/CardSituation";
import type { CardSleeve } from "../generated/CardSleeve";
import type { CardStatusHover } from "../generated/CardStatusHover";
import type { TicketCardView } from "../generated/TicketCardView";
import { Icon } from "../theme/Icon";
import { type CardState, outlineColor, RESTING, toneInk } from "./look";

export interface TicketCardProps {
  card: TicketCardView;
  state?: CardState;
  /** A click on the card (selects it). */
  onSelect?: () => void;
  /** A button on the card: the footer's action or an answer option. */
  onAction?: (kind: CardActionKind) => void;
  /** The quick action "Open panel". */
  onOpenPanel?: () => void;
}

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

/** The head's hover (SHL-07 "full status on hover"): the full status ("Needs you · permission"), then the facts the
 * card has no room for ("Tokens: 241k", "Branch otaq/SHOP-25"), one per line. */
export function StatusHoverCard({ hover }: { hover: CardStatusHover }) {
  return (
    <>
      <div className="card-hover-title">
        <strong>{hover.status}</strong>
      </div>
      {hover.details.map((line) => (
        <span key={line} className="card-hover-text" data-muted>
          {line}
        </span>
      ))}
    </>
  );
}

/** The floating card, placed next to its anchor once it is mounted. */
function HoverBubble({ anchor, children }: { anchor: RefObject<HTMLElement | null>; children: ReactNode }) {
  const card = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, card);
  return (
    <div ref={card} role="tooltip" className="card-hover" style={style}>
      {children}
    </div>
  );
}

/** Shows `content` after a short rest of the pointer on (or focus in) what it wraps; Esc, leave or blur hide it.
 * Without content it wraps nothing. */
function WithHover({ content, children }: { content: ReactNode; children: ReactNode }) {
  const anchor = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  if (content === null) return <>{children}</>;
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
      {open && <HoverBubble anchor={anchor}>{content}</HoverBubble>}
    </span>
  );
}

function SituationLine({ situation }: { situation: CardSituation }) {
  const content =
    situation.hovers.length === 0
      ? null
      : situation.hovers.map((hover) => <HoverCard key={hover.title} hover={hover} />);
  return (
    <WithHover content={content}>
      <div
        className="card-situation"
        data-mono={situation.mono || undefined}
        style={
          {
            "--edge": situation.edge === "muted" ? "var(--border-strong)" : toneInk(situation.edge).fg,
            color: toneInk(situation.text_tone).fg,
          } as CSSProperties
        }
      >
        {situation.text}
      </div>
    </WithHover>
  );
}

/** The footer (*Ticket Card v2 → Footer*): the tokens and the last signal on the right while an agent runs, or the
 * card's one action as a small button. M21 has no agent name on the left (M22). */
function Footer({ footer, onAction }: { footer: CardFooter; onAction?: TicketCardProps["onAction"] }) {
  const { tokens, signal, action } = footer;
  const tokenValue = tokens && (
    <span className="card-footer-value">
      <Icon name="coins" size={11} />
      {tokens.text}
    </span>
  );
  return (
    <div className="card-footer">
      <span className="card-spacer" />
      {tokens &&
        (tokens.tooltip === null ? tokenValue : <Tooltip text={tokens.tooltip}>{tokenValue}</Tooltip>)}
      {signal && (
        <span className="card-footer-value" data-stale={signal.stale || undefined}>
          {signal.text}
        </span>
      )}
      {action && (
        // The card's own click must not fire: the button's wrapper stops it.
        // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- only stops the click from reaching the card; the button inside is the control
        <span
          className="card-action"
          onClick={(e) => {
            e.stopPropagation();
          }}
        >
          {/* The v2 footer's button is a label only (*Ticket Card v2 → Footer → Action*). */}
          <Button
            label={action.label}
            variant={action.primary ? "primary" : "secondary"}
            compact
            onClick={() => onAction?.(action.kind)}
          />
        </span>
      )}
    </div>
  );
}

function QuickActions({ card, onOpenPanel }: { card: TicketCardView; onOpenPanel?: () => void }) {
  return (
    <div className="card-quick">
      <Tooltip text={card.open_terminal}>
        <span className="card-quick-icon">
          <Icon name="square-terminal" size={13} />
        </span>
      </Tooltip>
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

function Body({ card, onAction }: Pick<TicketCardProps, "card" | "onAction">) {
  return (
    <>
      <WithHover content={<StatusHoverCard hover={card.hover} />}>
        <div className="card-head">
          {card.avatar && <ToolAvatar avatar={card.avatar} />}
          <span className="card-key">{card.key_label}</span>
          {card.pinned && (
            <span className="card-pin">
              <Icon name="pin" size={12} />
            </span>
          )}
          {card.priority && <PriorityChip priority={card.priority} />}
          <span className="card-spacer" />
          {card.flag && <StatusPill pill={card.flag} />}
        </div>
      </WithHover>
      <Tooltip text={card.title}>
        <div className="card-title">{card.title}</div>
      </Tooltip>
      {card.situation && <SituationLine situation={card.situation} />}
      {card.origin !== null && <div className="card-origin">{card.origin}</div>}
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
      {card.footer && <Footer footer={card.footer} {...(onAction ? { onAction } : {})} />}
    </>
  );
}

export function TicketCard({ card, state = RESTING, onSelect, onAction, onOpenPanel }: TicketCardProps) {
  const sleeve = card.sleeve;
  const chain = state.chain === "none" ? undefined : state.chain;
  // A Done card's opacity; *Dimmed* (outside the hovered chain) overrides it in the stylesheet.
  const faded = card.faded === null ? undefined : ({ "--card-faded": card.faded } as CSSProperties);
  const article = (
    // The card is a pointer target; the board's keyboard moves the selection (T-0220), so it isn't a tab stop.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events -- the board's own keys select and open cards (SDD §21.3); a card isn't a button
    <article
      className="ticket-card"
      aria-label={`${card.key_label} ${card.title}`}
      data-selected={state.selected || undefined}
      data-chain={chain}
      data-dragging={state.dragging || undefined}
      data-hovered={state.hovered || undefined}
      data-faded={card.faded === null ? undefined : true}
      data-fresh={card.fresh || undefined}
      data-outline={card.outline}
      style={
        {
          "--card-outline": outlineColor(card.outline),
          // In a sleeve the whole sleeve dims or fades with the card.
          ...(sleeve === null ? faded : {}),
        } as CSSProperties
      }
      onClick={sleeve === null ? onSelect : undefined}
    >
      <Body card={card} {...(onAction ? { onAction } : {})} />
      {(onOpenPanel !== undefined || state.hovered) && (
        <QuickActions card={card} {...(onOpenPanel ? { onOpenPanel } : {})} />
      )}
    </article>
  );
  if (sleeve === null) return article;
  return (
    // The band selects the card too, so the sleeve takes the click.
    // eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- the board's own keys select and open cards (SDD §21.3); a card isn't a button
    <div
      className="card-sleeve"
      data-chain={chain}
      data-faded={faded && true}
      style={faded}
      onClick={onSelect}
    >
      <SleeveBand sleeve={sleeve} />
      {article}
    </div>
  );
}
