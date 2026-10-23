// Lanes: Epic on the Board (EPL-01; `s24-06`): the columns' headers once on top, then one lane per epic across the
// same columns, "No epic" last. A lane's head has the fold chevron, the epic's name, "PAY-123 · 5 tickets" (or "· from
// the Project agent"), the bar of its merged children and its lead's status; a folded lane is its head alone (with a
// dot when something in it needs you). The head is its epic: dragging it is dragging the epic (the board's one drag,
// `board.drag` / `board.drop`, so 0080's queue, take back and Park all run as before), a short press opens the epic,
// and its right click is the epic's menu. Everything it says is `BoardLane`'s. The header cells are the columns' drop
// targets, so a drop anywhere down a column lands on it; while a card is dragged the header and its cells take the
// verdict's look.
import { type PointerEvent, useState } from "react";

import { LeadStatus } from "../../components/LeadStatus";
import { Tooltip } from "../../components";
import type { BoardColumn } from "../../generated/BoardColumn";
import type { BoardDoneLine } from "../../generated/BoardDoneLine";
import type { BoardDropTarget } from "../../generated/BoardDropTarget";
import type { BoardEpicFocus } from "../../generated/BoardEpicFocus";
import type { BoardLane } from "../../generated/BoardLane";
import type { BoardLaneCell } from "../../generated/BoardLaneCell";
import type { BoardLaneHead } from "../../generated/BoardLaneHead";
import type { ChainView } from "../../generated/ChainView";
import type { DragVerdict } from "../../generated/DragVerdict";
import type { ViewChange } from "../../generated/ViewChange";
import { Icon } from "../../theme/Icon";
import { membersIn } from "./chain";
import {
  type CardHandlers,
  CardList,
  ColumnMenu,
  DoneLink,
  FoldLine,
  HeaderTitle,
  Rail,
  runsOf,
  verdictLook,
} from "./Column";

/** What a lane head does on the board. */
export interface LaneHandlers {
  /** The button went down on a lane head: its epic may be dragged. */
  press: (epic: string, e: PointerEvent) => void;
  /** The head's right-click menu: its epic's. */
  menu: (epic: string, at: { left: number; top: number }) => void;
  /** ↵ on a focused head: opens the epic. */
  open: (epic: string) => void;
}

/** The columns a lane shows a cell for: all but folded Done (the "Done 4 →" line). */
export function laneColumns(columns: readonly BoardColumn[]): BoardColumn[] {
  return columns.filter((c) => !(c.collapsed && c.rail === null));
}

/** The keys the arrow keys walk in each column with lanes: the open lanes' cards, lane by lane. */
export function laneWalk(columns: readonly BoardColumn[], lanes: readonly BoardLane[]): string[][] {
  return columns.map((column) =>
    lanes
      .filter((l) => !l.folded)
      .flatMap((l) => l.cells.find((c) => c.column === column.id)?.cards.map((card) => card.card.key) ?? []),
  );
}

/** A lane's head, as the board and the gallery draw it. */
export function LaneHeadRow({
  head,
  epic,
  folded,
  foldLabel,
  dimmed = false,
  onFold,
  onPress,
  onMenu,
  onOpen,
}: {
  head: BoardLaneHead;
  /** The epic's key: the head is draggable; `null` for "No epic". */
  epic: string | null;
  folded: boolean;
  foldLabel: string;
  dimmed?: boolean;
  onFold: () => void;
  onPress?: (e: PointerEvent) => void;
  onMenu?: (at: { left: number; top: number }) => void;
  onOpen?: () => void;
}) {
  return (
    <div
      className="board-lane-head"
      data-epic={epic ?? undefined}
      data-dimmed={dimmed || undefined}
      role="button"
      aria-label={head.name}
      tabIndex={epic === null ? -1 : 0}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget || e.key !== "Enter" || onOpen === undefined) return;
        e.preventDefault();
        onOpen();
      }}
      onPointerDown={(e) => {
        if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
        onPress?.(e);
      }}
      onContextMenu={(e) => {
        if (onMenu === undefined) return;
        e.preventDefault();
        onMenu({ left: e.clientX, top: e.clientY });
      }}
    >
      <Tooltip text={foldLabel}>
        <button
          type="button"
          className="board-lane-fold"
          aria-label={foldLabel}
          aria-expanded={!folded}
          onClick={onFold}
        >
          <Icon name={folded ? "chevron-right" : "chevron-down"} size={14} />
        </button>
      </Tooltip>
      {epic !== null && (
        <span className="board-lane-icon">
          <Icon name="layers" size={14} />
        </span>
      )}
      <span className="board-lane-name">{head.name}</span>
      {head.pinned && (
        <span className="board-lane-pin">
          <Icon name="pin" size={12} />
        </span>
      )}
      <span className="board-lane-detail">{head.detail}</span>
      {head.progress !== null && (
        <Tooltip text={head.progress.label}>
          <span className="board-lane-progress" role="img" aria-label={head.progress.label}>
            <span
              className="board-lane-progress-fill"
              style={{
                width: `${String(head.progress.total === 0 ? 0 : (100 * head.progress.merged) / head.progress.total)}%`,
              }}
            />
          </span>
        </Tooltip>
      )}
      {folded && head.needs_you && <span className="board-lane-dot" aria-hidden />}
      <span className="board-spacer" />
      {head.lead !== null && <LeadStatus lead={head.lead} form="full" />}
    </div>
  );
}

interface LaneCellProps {
  column: BoardColumn;
  cell: BoardLaneCell | undefined;
  verdict: DragVerdict | undefined;
  dragging: boolean;
  selected: string | null;
  chain: ChainView | null;
  lifted: string | null;
  focus: BoardEpicFocus | null;
  strong: ReadonlySet<string> | null;
  handlers: CardHandlers;
}

/** One lane's part of a column: its cards, Done's epic lines, or nothing under a folded column. */
function LaneCell({
  column,
  cell,
  verdict,
  dragging,
  selected,
  chain,
  lifted,
  focus,
  strong,
  handlers,
}: LaneCellProps) {
  const [open, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  if (column.collapsed) return <div className="board-lane-cell" data-collapsed />;
  const cards = cell?.cards ?? [];
  const tabStop = cards.some((c) => c.card.key === selected) ? selected : (cards[0]?.card.key ?? null);
  return (
    <div
      className="board-lane-cell"
      data-column={column.id}
      data-verdict={dragging ? verdictLook(verdict) : undefined}
    >
      {runsOf(cards, cell?.folds ?? []).map((run) => {
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
        const shown = open.has(epic);
        return [
          <FoldLine
            key={`${epic}:fold`}
            fold={run.fold}
            open={shown}
            chain={!shown && membersIn(chain, run.fold.keys).length > 0}
            dimmed={focus !== null && !focus.epics.includes(epic)}
            onToggle={() => {
              setOpen((old) => {
                const next = new Set(old);
                if (!next.delete(epic)) next.add(epic);
                return next;
              });
            }}
          />,
          shown && list,
        ];
      })}
    </div>
  );
}

export interface LanesProps {
  columns: readonly BoardColumn[];
  lanes: readonly BoardLane[];
  /** The drag's verdict for a column; `undefined` while nothing is dragged. */
  verdictOf: (column: BoardDropTarget) => DragVerdict | undefined;
  dragging: boolean;
  selected: string | null;
  chain: ChainView | null;
  lifted: string | null;
  focus: BoardEpicFocus | null;
  handlers: CardHandlers;
  laneHandlers: LaneHandlers;
  onView: (change: ViewChange) => void;
  onAdd: () => void;
  /** Done folded: its "Done 4 →" line, after the headers. */
  doneLine: BoardDoneLine | null;
  /** The header cells are the columns' drop targets. */
  element: (target: BoardDropTarget, el: HTMLElement | null) => void;
}

export function Lanes({
  columns,
  lanes,
  verdictOf,
  dragging,
  selected,
  chain,
  lifted,
  focus,
  handlers,
  laneHandlers,
  onView,
  onAdd,
  doneLine,
  element,
}: LanesProps) {
  const shown = laneColumns(columns);
  const strong = focus ? new Set(focus.keys) : null;
  return (
    <div className="board-lanes">
      <div className="board-lane-headers">
        {shown.map((column) =>
          column.collapsed ? (
            <Rail key={column.id} column={column} members={[]} onView={onView} element={element} />
          ) : (
            <section
              key={column.id}
              className="board-lane-header"
              aria-label={column.title}
              data-column={column.id}
              data-kind={column.kind}
              data-verdict={dragging ? verdictLook(verdictOf(column.id)) : undefined}
              ref={(el) => {
                element(column.id, el);
              }}
            >
              <header className="board-column-header">
                <HeaderTitle column={column} />
                <span className="board-spacer" />
                {column.add !== null && (
                  <Tooltip text={column.add}>
                    <button
                      type="button"
                      className="board-column-button"
                      aria-label={column.add}
                      onClick={onAdd}
                    >
                      <Icon name="plus" size={14} />
                    </button>
                  </Tooltip>
                )}
                <ColumnMenu column={column} onView={onView} />
              </header>
              {column.hint !== null && <div className="board-column-hint">{column.hint}</div>}
            </section>
          ),
        )}
        {doneLine !== null && <DoneLink line={doneLine} onView={onView} element={element} />}
      </div>
      {lanes.map((lane) => (
        <section
          key={lane.id}
          className="board-lane"
          aria-label={lane.head.name}
          data-lane={lane.id}
          data-folded={lane.folded || undefined}
        >
          <LaneHeadRow
            head={lane.head}
            epic={lane.epic}
            folded={lane.folded}
            foldLabel={lane.fold_label}
            dimmed={focus !== null && (lane.epic === null || !focus.epics.includes(lane.epic))}
            onFold={() => {
              onView(lane.fold);
            }}
            onPress={
              lane.epic === null
                ? undefined
                : (e) => {
                    if (lane.epic !== null) laneHandlers.press(lane.epic, e);
                  }
            }
            onMenu={
              lane.epic === null
                ? undefined
                : (at) => {
                    if (lane.epic !== null) laneHandlers.menu(lane.epic, at);
                  }
            }
            onOpen={
              lane.epic === null
                ? undefined
                : () => {
                    if (lane.epic !== null) laneHandlers.open(lane.epic);
                  }
            }
          />
          {!lane.folded && (
            <div className="board-lane-cells">
              {shown.map((column) => (
                <LaneCell
                  key={column.id}
                  column={column}
                  cell={lane.cells.find((c) => c.column === column.id)}
                  verdict={verdictOf(column.id)}
                  dragging={dragging}
                  selected={selected}
                  chain={chain}
                  lifted={lifted}
                  focus={focus}
                  strong={strong}
                  handlers={handlers}
                />
              ))}
            </div>
          )}
        </section>
      ))}
    </div>
  );
}
