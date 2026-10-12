// One board column (*03 · Board — shop-web → Body → Board*): the header with icon, title and count (Done's fold
// button, Backlog's plus), Ready for Dev's hint, Approved's note, the cards (Dev: the floated cards, the
// "WORKING · VALIDATING" section and the "N more · working" fold), the empty hint, Backlog's "Add ticket" row, and
// the "7 above · 6 need you" chip of a column scrolled down (*Very full column*). Done folded is the 34 px rail.
// Everything it says is `BoardColumn`'s; while a card is dragged over it, it takes the verdict's outline.
import {
  type KeyboardEvent,
  memo,
  type PointerEvent,
  useCallback,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { EpicCard, RESTING, TicketCard } from "../../cards";
import { Tooltip } from "../../components";
import type { BoardCard } from "../../generated/BoardCard";
import type { BoardColumn } from "../../generated/BoardColumn";
import type { BoardColumnId } from "../../generated/BoardColumnId";
import type { BoardScrollChip } from "../../generated/BoardScrollChip";
import type { CardActionKind } from "../../generated/CardActionKind";
import type { DragVerdict } from "../../generated/DragVerdict";
import { Icon } from "../../theme/Icon";
import { type AboveChip, aboveChip } from "./drag";

/** What a card does on the board; the same functions for every card, so the card lists don't redraw. */
export interface CardHandlers {
  press: (key: string, column: BoardColumnId, e: PointerEvent) => void;
  key: (key: string, column: BoardColumnId, e: KeyboardEvent) => void;
  open: (key: string) => void;
  action: (key: string, kind: CardActionKind) => void;
  addToEpic: (epic: string) => void;
}

/** The column icon's colour (*03*: each column's icon in its status colour). */
const ICON_INK: Record<BoardColumnId, string> = {
  backlog: "var(--st-queued)",
  ready_for_dev: "var(--st-queued)",
  dev: "var(--st-working)",
  ready_for_review: "var(--st-review)",
  approved: "var(--st-approved)",
  done: "var(--st-done)",
};

interface CardItemProps {
  card: BoardCard;
  column: BoardColumnId;
  selected: boolean;
  lifted: boolean;
  focusable: boolean;
  handlers: CardHandlers;
}

const CardItem = memo(function CardItem({
  card,
  column,
  selected,
  lifted,
  focusable,
  handlers,
}: CardItemProps) {
  const key = card.card.key;
  const state = selected ? { ...RESTING, selected: true } : RESTING;
  return (
    <div
      className="board-card"
      data-card={key}
      data-needs-you={(card.kind === "ticket" && card.needs_you) || undefined}
      data-lifted={lifted || undefined}
      role="button"
      tabIndex={focusable ? 0 : -1}
      aria-label={`${key} ${card.card.title}`}
      aria-pressed={selected}
      onPointerDown={(e) => {
        handlers.press(key, column, e);
      }}
      onKeyDown={(e) => {
        handlers.key(key, column, e);
      }}
    >
      {card.kind === "ticket" ? (
        <TicketCard
          card={card.card}
          state={state}
          onAction={(kind) => {
            handlers.action(key, kind);
          }}
          onOpenPanel={() => {
            handlers.open(key);
          }}
        />
      ) : (
        <EpicCard
          card={card.card}
          state={state}
          onChild={handlers.open}
          onAdd={() => {
            handlers.addToEpic(key);
          }}
          onAction={(kind) => {
            handlers.action(key, kind);
          }}
        />
      )}
    </div>
  );
});

interface CardListProps {
  cards: readonly BoardCard[];
  column: BoardColumnId;
  selected: string | null;
  lifted: string | null;
  /** The card that is the column's tab stop (the selected one, else the first). */
  tabStop: string | null;
  handlers: CardHandlers;
}

const CardList = memo(function CardList({
  cards,
  column,
  selected,
  lifted,
  tabStop,
  handlers,
}: CardListProps) {
  return (
    <>
      {cards.map((card) => (
        <CardItem
          key={card.card.key}
          card={card}
          column={column}
          selected={card.card.key === selected}
          lifted={card.card.key === lifted}
          focusable={card.card.key === tabStop}
          handlers={handlers}
        />
      ))}
    </>
  );
});

/** Every card of a column in board order, as the keyboard walks them. */
export function cardsOf(column: BoardColumn): BoardCard[] {
  return column.dev ? [...column.dev.top, ...column.dev.working] : column.cards;
}

export interface ColumnProps {
  column: BoardColumn;
  chipWords: BoardScrollChip;
  /** The column's look under a dragged card; `undefined` while nothing is dragged over it. */
  verdict?: DragVerdict | undefined;
  /** A card is being dragged (anywhere). */
  dragging: boolean;
  selected: string | null;
  lifted: string | null;
  handlers: CardHandlers;
  /** The column's own empty hint and "Add ticket" row; off while the empty board's card covers the board. */
  showEmptyHint?: boolean;
  /** Where this column was scrolled when the project was last shown. */
  scrollTop: number;
  onScroll: (column: BoardColumnId, top: number) => void;
  onToggleDone: () => void;
  onUnfold: () => void;
  onAdd: () => void;
  /** The column's element, measured when a drag starts. */
  element: (column: BoardColumnId, el: HTMLElement | null) => void;
}

function verdictLook(verdict: DragVerdict | undefined): string | undefined {
  return verdict === undefined || verdict.kind === "home" ? undefined : verdict.kind;
}

function DoneRail({
  column,
  onToggle,
  element,
}: Pick<ColumnProps, "column" | "element"> & { onToggle: () => void }) {
  return (
    <Tooltip text={column.toggle ?? column.title}>
      <button
        type="button"
        className="board-rail"
        ref={(el) => {
          element(column.id, el);
        }}
        aria-label={`${column.title} ${column.count}`}
        onClick={onToggle}
      >
        <span style={{ color: ICON_INK[column.id] }}>
          <Icon name={column.icon} size={13} />
        </span>
        <span className="board-rail-count">{column.count}</span>
        <span className="board-rail-title">{column.title}</span>
      </button>
    </Tooltip>
  );
}

export function Column({
  column,
  chipWords,
  verdict,
  dragging,
  selected,
  lifted,
  handlers,
  showEmptyHint = true,
  scrollTop,
  onScroll,
  onToggleDone,
  onUnfold,
  onAdd,
  element,
}: ColumnProps) {
  const scroller = useRef<HTMLDivElement>(null);
  const [chip, setChip] = useState<AboveChip | null>(null);
  const all = cardsOf(column);
  const tabStop = all.some((c) => c.card.key === selected) ? selected : (all[0]?.card.key ?? null);

  // The chip counts the cards scrolled out above the view, and how many of them need the user.
  const measure = useCallback(() => {
    const box = scroller.current;
    if (!box) return;
    const above = [...box.querySelectorAll<HTMLElement>("[data-card]")]
      .filter((el) => el.offsetTop + el.offsetHeight <= box.scrollTop)
      .map((el) => el.dataset.needsYou !== undefined);
    setChip(aboveChip(chipWords, above));
  }, [chipWords]);

  // Back where the user left it when the project is shown again.
  useLayoutEffect(() => {
    const box = scroller.current;
    if (box && box.scrollTop !== scrollTop) box.scrollTop = scrollTop;
    measure();
    // Only when the column (project) changes, not on each scroll the column reports itself.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- scrollTop is read once per column shown
  }, [column.id, measure]);

  if (column.collapsed) return <DoneRail column={column} onToggle={onToggleDone} element={element} />;

  const empty = all.length === 0;
  return (
    <section
      className="board-column"
      ref={(el) => {
        element(column.id, el);
      }}
      aria-label={column.title}
      data-column={column.id}
      data-verdict={dragging ? verdictLook(verdict) : undefined}
    >
      <header className="board-column-header">
        <span className="board-column-icon" style={{ color: ICON_INK[column.id] }}>
          <Icon name={column.icon} size={13} />
        </span>
        <span className="board-column-title">{column.title}</span>
        <span className="board-column-count">{column.count}</span>
        <span className="board-spacer" />
        {column.add !== null && (
          <Tooltip text={column.add}>
            <button type="button" className="board-column-button" aria-label={column.add} onClick={onAdd}>
              <Icon name="plus" size={14} />
            </button>
          </Tooltip>
        )}
        {column.toggle !== null && (
          <Tooltip text={column.toggle}>
            <button
              type="button"
              className="board-column-button"
              aria-label={column.toggle}
              onClick={onToggleDone}
            >
              <Icon name="chevrons-left-right" size={13} />
            </button>
          </Tooltip>
        )}
      </header>
      {column.hint !== null && <div className="board-column-hint">{column.hint}</div>}
      {column.note !== null && (
        <div className="board-column-note">
          <Icon name={column.note.icon} size={12} />
          <span>{column.note.text}</span>
        </div>
      )}
      {chip !== null && (
        <div className="board-above">
          <button
            type="button"
            className="board-chip"
            data-attention={chip.needYou || undefined}
            onClick={() => {
              scroller.current?.scrollTo({ top: 0, behavior: "smooth" });
            }}
          >
            <Icon name="chevron-up" size={12} />
            <span>{chip.text}</span>
          </button>
        </div>
      )}
      <div
        className="board-column-cards"
        ref={scroller}
        onScroll={(e) => {
          onScroll(column.id, e.currentTarget.scrollTop);
          measure();
        }}
      >
        {column.dev ? (
          <>
            <CardList
              cards={column.dev.top}
              column={column.id}
              selected={selected}
              lifted={lifted}
              tabStop={tabStop}
              handlers={handlers}
            />
            {column.dev.working_count !== "0" && (
              <div className="board-section">
                <span className="board-section-title">{column.dev.section}</span>
                <span className="board-section-count">{column.dev.working_count}</span>
              </div>
            )}
            <CardList
              cards={column.dev.working}
              column={column.id}
              selected={selected}
              lifted={lifted}
              tabStop={tabStop}
              handlers={handlers}
            />
            {column.dev.fold !== null && (
              <div className="board-fold">
                <button type="button" className="board-chip" onClick={onUnfold}>
                  <span>{column.dev.fold}</span>
                  <Icon name="chevron-down" size={12} />
                </button>
              </div>
            )}
          </>
        ) : (
          <CardList
            cards={column.cards}
            column={column.id}
            selected={selected}
            lifted={lifted}
            tabStop={tabStop}
            handlers={handlers}
          />
        )}
        {empty && showEmptyHint && (
          <div className="board-empty-column">
            <Icon name={column.icon} size={18} />
            <span>{column.empty_hint}</span>
          </div>
        )}
        {column.hidden !== null && <div className="board-hidden">{column.hidden}</div>}
        {column.add !== null && showEmptyHint && (
          <button type="button" className="board-add" onClick={onAdd}>
            <Icon name="plus" size={13} />
            <span>{column.add}</span>
          </button>
        )}
      </div>
    </section>
  );
}
