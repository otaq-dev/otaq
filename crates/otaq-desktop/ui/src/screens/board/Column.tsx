// One board column (*03 · Board — shop-web → Body → Board*): the header with icon, title and count (Done's fold
// button, Backlog's plus), Ready for Dev's hint, the cards (Dev: the floated cards, the "WORKING · VALIDATING"
// section and the "N more · working" fold; Review: its two sections, below), the empty hint, Backlog's "Add ticket"
// row, and the "7 above · 6 need you" chip of a column scrolled down (*Very full column*). Done folded is the 34 px
// rail. The Review column (*Row · Review column (0086 · V)*, d0087) holds "READY FOR REVIEW" and "APPROVED": each
// label folds its section (the count stays), an empty section says so, and Approved is a drop target of its own: a
// Ready for Review card dragged over it lights it with the hint, and while such a card is dragged an empty Approved
// says how. Approved's "doesn't watch PRs" line is on its cards (Card v2), so the section doesn't repeat it.
// Everything it says is `BoardColumn`'s; while a card is dragged over it, it takes the verdict's outline. Under the
// chain highlight (T-0229) its cards take their chain look, and the above chip, Dev's fold and the Done rail turn
// accent for members they hide. In Done an epic's merged children fold into one line (EPC-16, *01 · Components →
// Done Fold*): a click shows their cards, another folds them; which folds are open is this screen's only. While an
// epic is focused (EPC-18, Q55) the other cards and Done lines stay in place, dimmed.
import {
  type KeyboardEvent,
  memo,
  type PointerEvent,
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { type ChainState, EpicCard, RESTING, TicketCard } from "../../cards";
import { Tooltip } from "../../components";
import type { BoardCard } from "../../generated/BoardCard";
import type { BoardColumn } from "../../generated/BoardColumn";
import type { BoardColumnId } from "../../generated/BoardColumnId";
import type { BoardDropTarget } from "../../generated/BoardDropTarget";
import type { BoardDoneFold } from "../../generated/BoardDoneFold";
import type { BoardEpicFocus } from "../../generated/BoardEpicFocus";
import type { BoardReviewSection } from "../../generated/BoardReviewSection";
import type { BoardReviewSectionId } from "../../generated/BoardReviewSectionId";
import type { BoardScrollChip } from "../../generated/BoardScrollChip";
import type { CardActionKind } from "../../generated/CardActionKind";
import type { ChainView } from "../../generated/ChainView";
import type { DragVerdict } from "../../generated/DragVerdict";
import { Icon } from "../../theme/Icon";
import { chainStateOf, membersIn } from "./chain";
import { type AboveChip, aboveChip } from "./drag";

/** What a card does on the board; the same functions for every card, so the card lists don't redraw. */
export interface CardHandlers {
  press: (key: string, column: BoardColumnId, e: PointerEvent) => void;
  /** The button went down on a draggable row of an epic card: that child may be dragged out (EPC-12). */
  pressChild: (key: string, e: PointerEvent) => void;
  key: (key: string, column: BoardColumnId, e: KeyboardEvent) => void;
  open: (key: string) => void;
  action: (key: string, kind: CardActionKind) => void;
  addToEpic: (epic: string) => void;
  /** The card's right-click menu, at the pointer (or under the card, from the keyboard). */
  menu: (key: string, at: { left: number; top: number }) => void;
  /** The pointer came onto (`true`) or left (`false`) the card: its chain lights up. */
  hover: (key: string, over: boolean) => void;
}

/** The column icon's colour (*03*: each column's icon in its status colour). */
const ICON_INK: Record<BoardColumnId, string> = {
  backlog: "var(--st-queued)",
  ready_for_dev: "var(--st-queued)",
  dev: "var(--st-working)",
  review: "var(--st-review)",
  done: "var(--st-done)",
};

interface CardItemProps {
  card: BoardCard;
  column: BoardColumnId;
  selected: boolean;
  chain: ChainState;
  lifted: boolean;
  /** Another epic is focused: the card stays, faded. */
  dimmed: boolean;
  focusable: boolean;
  handlers: CardHandlers;
}

const CardItem = memo(function CardItem({
  card,
  column,
  selected,
  chain,
  lifted,
  dimmed,
  focusable,
  handlers,
}: CardItemProps) {
  const key = card.card.key;
  const state = selected || chain !== "none" ? { ...RESTING, selected, chain } : RESTING;
  return (
    <div
      className="board-card"
      data-card={key}
      data-needs-you={(card.kind === "ticket" && card.needs_you) || undefined}
      data-lifted={lifted || undefined}
      data-dimmed={dimmed || undefined}
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
      onContextMenu={(e) => {
        e.preventDefault();
        handlers.menu(key, { left: e.clientX, top: e.clientY });
      }}
      onPointerEnter={() => {
        handlers.hover(key, true);
      }}
      onPointerLeave={() => {
        handlers.hover(key, false);
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
          onChildPress={handlers.pressChild}
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
  chain: ChainView | null;
  lifted: string | null;
  /** The card that is the column's tab stop (the selected one, else the first). */
  tabStop: string | null;
  /** The cards a focused epic keeps at full strength; `null` without a focus. */
  strong: ReadonlySet<string> | null;
  handlers: CardHandlers;
}

const CardList = memo(function CardList({
  cards,
  column,
  selected,
  chain,
  lifted,
  tabStop,
  strong,
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
          chain={chainStateOf(chain, card.card.key)}
          lifted={card.card.key === lifted}
          dimmed={strong !== null && !strong.has(card.card.key)}
          focusable={card.card.key === tabStop}
          handlers={handlers}
        />
      ))}
    </>
  );
});

/** The column a card is in, by its key; `null` for a key that is no card (an epic's row, a track's chip). */
export function columnOfCard(columns: readonly BoardColumn[], key: string): BoardColumnId | null {
  return columns.find((c) => cardsOf(c).some((card) => card.card.key === key))?.id ?? null;
}

/** Every card of a column in board order, the folded ones included. */
export function cardsOf(column: BoardColumn): BoardCard[] {
  return column.dev
    ? [...column.dev.top, ...column.dev.working]
    : [...column.cards, ...column.sections.flatMap((s) => s.cards)];
}

/** The cards the keyboard walks: every card but those behind a Done fold or in a folded Review section. */
export function walkOf(
  column: BoardColumn,
  foldedSections: readonly BoardReviewSectionId[] = [],
): BoardCard[] {
  const folded = new Set([
    ...column.folds.flatMap((f) => f.keys),
    ...column.sections
      .filter((s) => foldedSections.includes(s.id))
      .flatMap((s) => s.cards.map((c) => c.card.key)),
  ]);
  return cardsOf(column).filter((c) => !folded.has(c.card.key));
}

/** A column's cards cut at its folds: runs of cards, and each fold with its cards where the first of them is. */
export type Run =
  { kind: "cards"; cards: BoardCard[] } | { kind: "fold"; fold: BoardDoneFold; cards: BoardCard[] };

export function runsOf(cards: readonly BoardCard[], folds: readonly BoardDoneFold[]): Run[] {
  const runs: Run[] = [];
  for (const card of cards) {
    const fold = folds.find((f) => f.keys.includes(card.card.key));
    const last = runs.at(-1);
    if (fold) {
      if (last?.kind === "fold" && last.fold === fold) last.cards.push(card);
      else runs.push({ kind: "fold", fold, cards: [card] });
    } else if (last?.kind === "cards") last.cards.push(card);
    else runs.push({ kind: "cards", cards: [card] });
  }
  return runs;
}

function FoldLine({
  fold,
  open,
  chain,
  dimmed,
  onToggle,
}: {
  fold: BoardDoneFold;
  open: boolean;
  chain: boolean;
  dimmed: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      type="button"
      className="board-done-fold"
      data-epic={fold.epic}
      data-chain={chain || undefined}
      data-dimmed={dimmed || undefined}
      aria-expanded={open}
      onClick={onToggle}
    >
      <Icon name="layers" size={12} />
      <span className="board-done-fold-label">{fold.label}</span>
      <Icon name={open ? "chevron-up" : "chevron-down"} size={13} />
    </button>
  );
}

export interface ColumnProps {
  column: BoardColumn;
  chipWords: BoardScrollChip;
  /** The column's look under a dragged card; `undefined` while nothing is dragged over it. */
  verdict?: DragVerdict | undefined;
  /** A card is being dragged (anywhere). */
  dragging: boolean;
  selected: string | null;
  /** The highlighted chain, if any (T-0229). */
  chain?: ChainView | null;
  lifted: string | null;
  /** The focused epic (EPC-18): its cards stay, the others dim. */
  focus?: BoardEpicFocus | null;
  handlers: CardHandlers;
  /** The column's own empty hint and "Add ticket" row; off while the empty board's card covers the board. */
  showEmptyHint?: boolean;
  /** Where this column was scrolled when the project was last shown. */
  scrollTop: number;
  onScroll: (column: BoardColumnId, top: number) => void;
  onToggleDone: () => void;
  onUnfold: () => void;
  onAdd: () => void;
  /** Review: the sections folded by the user (kept per project, `prefs.ts`). */
  foldedSections?: readonly BoardReviewSectionId[];
  onToggleSection?: (section: BoardReviewSectionId) => void;
  /** Review, while a card is dragged: the Approved section's verdict, and whether the card is over it. */
  approved?: { verdict: DragVerdict | undefined; over: boolean } | undefined;
  /** The column's element (and Review's Approved section's), measured when a drag starts. */
  element: (target: BoardDropTarget, el: HTMLElement | null) => void;
}

function verdictLook(verdict: DragVerdict | undefined): string | undefined {
  return verdict === undefined || verdict.kind === "home" ? undefined : verdict.kind;
}

/** How a Review section looks under the dragged card: lit (or red) only while the card is over it. */
export function sectionLook(
  drag: { verdict: DragVerdict | undefined; over: boolean } | undefined,
): string | undefined {
  return drag?.over ? verdictLook(drag.verdict) : undefined;
}

interface ReviewSectionProps extends Omit<CardListProps, "cards"> {
  section: BoardReviewSection;
  folded: boolean;
  /** Approved only, while a card is dragged. */
  drag: { verdict: DragVerdict | undefined; over: boolean } | undefined;
  onToggle: () => void;
  element: ColumnProps["element"];
}

function ReviewSection({ section, folded, drag, onToggle, element, ...list }: ReviewSectionProps) {
  const look = sectionLook(drag);
  const hint = look === "allowed" && drag?.verdict?.kind === "allowed" ? drag.verdict.hint : null;
  // A Ready for Review card is the one Approved takes: while it is dragged, an empty Approved says how.
  const empty =
    drag?.verdict?.kind === "allowed" && section.drag_hint !== null ? section.drag_hint : section.empty;
  return (
    <div
      className="board-review-section"
      data-section={section.id}
      data-verdict={look}
      ref={
        section.id === "approved"
          ? (el) => {
              element("approved", el);
            }
          : undefined
      }
    >
      <Tooltip text={folded ? section.unfold : section.fold}>
        <button type="button" className="board-review-label" aria-expanded={!folded} onClick={onToggle}>
          <Icon name={folded ? "chevron-right" : "chevron-down"} size={11} />
          <span className="board-review-title">{section.title}</span>
          <span className="board-review-count">{section.count}</span>
        </button>
      </Tooltip>
      {hint !== null && <div className="board-review-hint">{hint}</div>}
      {!folded && (
        <>
          <CardList cards={section.cards} {...list} />
          {section.cards.length === 0 && <div className="board-review-empty">{empty}</div>}
          {section.hidden !== null && <div className="board-hidden">{section.hidden}</div>}
        </>
      )}
    </div>
  );
}

function DoneRail({
  column,
  members,
  onToggle,
  element,
}: Pick<ColumnProps, "column" | "element"> & { members: string[]; onToggle: () => void }) {
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
        {/* The chain's members folded in the rail (*Chain highlight → SHOP-17 chip*). */}
        {members.map((key) => (
          <span key={key} className="board-rail-chain">
            {key}
          </span>
        ))}
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
  chain = null,
  lifted,
  focus = null,
  handlers,
  showEmptyHint = true,
  scrollTop,
  onScroll,
  onToggleDone,
  onUnfold,
  onAdd,
  foldedSections = [],
  onToggleSection,
  approved,
  element,
}: ColumnProps) {
  const scroller = useRef<HTMLDivElement>(null);
  const [chip, setChip] = useState<AboveChip | null>(null);
  const [aboveKeys, setAboveKeys] = useState<string[]>([]);
  const [openFolds, setOpenFolds] = useState<ReadonlySet<string>>(() => new Set());
  const strong = useMemo(() => (focus ? new Set(focus.keys) : null), [focus]);
  const all = cardsOf(column);
  const walk = walkOf(column, foldedSections);
  const tabStop = walk.some((c) => c.card.key === selected) ? selected : (walk[0]?.card.key ?? null);

  // The chip counts the cards scrolled out above the view, and how many of them need the user.
  const measure = useCallback(() => {
    const box = scroller.current;
    if (!box) return;
    const above = [...box.querySelectorAll<HTMLElement>("[data-card]")].filter(
      (el) => el.offsetTop + el.offsetHeight <= box.scrollTop,
    );
    setChip(
      aboveChip(
        chipWords,
        above.map((el) => el.dataset.needsYou !== undefined),
      ),
    );
    setAboveKeys((old) => {
      const next = above.map((el) => el.dataset.card ?? "");
      return old.length === next.length && old.every((k, i) => k === next[i]) ? old : next;
    });
  }, [chipWords]);

  // Back where the user left it when the project is shown again.
  useLayoutEffect(() => {
    const box = scroller.current;
    if (box && box.scrollTop !== scrollTop) box.scrollTop = scrollTop;
    measure();
    // Only when the column (project) changes, not on each scroll the column reports itself.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- scrollTop is read once per column shown
  }, [column.id, measure]);

  if (column.collapsed)
    return (
      <DoneRail
        column={column}
        members={membersIn(
          chain,
          all.map((c) => c.card.key),
        )}
        onToggle={onToggleDone}
        element={element}
      />
    );

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
      {chip !== null && (
        <div className="board-above">
          <button
            type="button"
            className="board-chip"
            data-attention={chip.needYou || undefined}
            data-chain={membersIn(chain, aboveKeys).length > 0 || undefined}
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
              chain={chain}
              lifted={lifted}
              tabStop={tabStop}
              strong={strong}
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
              chain={chain}
              lifted={lifted}
              tabStop={tabStop}
              strong={strong}
              handlers={handlers}
            />
            {column.dev.fold !== null && (
              <div className="board-fold">
                <button
                  type="button"
                  className="board-chip"
                  data-chain={membersIn(chain, column.dev.folded).length > 0 || undefined}
                  onClick={onUnfold}
                >
                  <span>{column.dev.fold}</span>
                  <Icon name="chevron-down" size={12} />
                </button>
              </div>
            )}
          </>
        ) : column.sections.length > 0 ? (
          column.sections.map((section) => (
            <ReviewSection
              key={section.id}
              section={section}
              folded={foldedSections.includes(section.id)}
              drag={section.id === "approved" ? approved : undefined}
              onToggle={() => {
                onToggleSection?.(section.id);
              }}
              element={element}
              column={column.id}
              selected={selected}
              chain={chain}
              lifted={lifted}
              tabStop={tabStop}
              strong={strong}
              handlers={handlers}
            />
          ))
        ) : (
          runsOf(cardsOf(column), column.folds).map((run) => {
            const list = (
              <CardList
                key={run.kind === "fold" ? `${run.fold.epic}:cards` : `run:${run.cards[0]?.card.key ?? ""}`}
                cards={run.cards}
                column={column.id}
                selected={selected}
                chain={chain}
                lifted={lifted}
                tabStop={tabStop}
                strong={strong}
                handlers={handlers}
              />
            );
            if (run.kind === "cards") return list;
            const { epic } = run.fold;
            const open = openFolds.has(epic);
            return [
              <FoldLine
                key={`${epic}:fold`}
                fold={run.fold}
                open={open}
                chain={!open && membersIn(chain, run.fold.keys).length > 0}
                dimmed={focus !== null && !focus.epics.includes(epic)}
                onToggle={() => {
                  setOpenFolds((old) => {
                    const next = new Set(old);
                    if (!next.delete(epic)) next.add(epic);
                    return next;
                  });
                }}
              />,
              open && list,
            ];
          })
        )}
        {empty && showEmptyHint && column.sections.length === 0 && (
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
