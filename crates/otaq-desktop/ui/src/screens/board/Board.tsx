// The board (*03 · Board — shop-web*, *03 · Board & columns*; UI-02): the six columns of the shown project from
// `board.view`, refetched on the service's events, at the board's size (Rust folds Done below 1440 and Dev by
// height). A drag is the UIT-04 way (`drag.ts`): one `board.drag` at its start, then only this page works while the
// pointer moves, drawing the columns' verdicts, the lifted card and the hint or reason under it; a release on an
// allowed column sends `board.drop`, whose answer may be the Park confirmation or the approve choice. Esc cancels.
// Hovering (or selecting) a linked card highlights its chain (`chain.ts`, `ChainLayer`; T-0229). While an epic is in
// flight the "Epics in flight" strip sits above the columns (`strip/`, T-0269). The epic card, one of its rows and a
// "not queued" chip on a track are dragged the same way (EPC-12, T-0270): Rust answers what each queues, and the drop
// sends `epic.queue`, whose outcome is the queue toast; a row or a chip lifts as a small row.
import {
  type KeyboardEvent,
  type PointerEvent,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { requestCardAction } from "../../app/cardActions";
import { openOverlay } from "../../app/overlays";
import { useShortcut } from "../../app/registry";
import { openTicket, useSelection } from "../../app/selection";
import { useStore } from "../../app/store";
import { EpicCard, RESTING, TicketCard } from "../../cards";
import type { Anchor } from "../../components";
import type { ApproveChoice as Choice } from "../../generated/ApproveChoice";
import type { ApproveMenu } from "../../generated/ApproveMenu";
import type { BoardCard } from "../../generated/BoardCard";
import type { BoardColumnId } from "../../generated/BoardColumnId";
import type { BoardDropParams } from "../../generated/BoardDropParams";
import type { BoardParams } from "../../generated/BoardParams";
import type { BoardTicketParams } from "../../generated/BoardTicketParams";
import type { BoardView } from "../../generated/BoardView";
import type { CardActionKind } from "../../generated/CardActionKind";
import type { DropOutcome } from "../../generated/DropOutcome";
import type { MenuItem } from "../../generated/MenuItem";
import type { NewToast } from "../../generated/NewToast";
import type { ParkDialog } from "../../generated/ParkDialog";
import { asRequestError, useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { ApproveChoice } from "./ApproveChoice";
import { activeChain } from "./chain";
import { ChainLayer } from "./ChainLayer";
import { CardMenu, type OpenCardMenu } from "./CardMenu";
import { cardsOf, type CardHandlers, Column } from "./Column";
import {
  type ColumnSpan,
  createDrag,
  type DragState,
  type LiftedRow,
  liftedRow,
  moveFocus,
  verdictOf,
} from "./drag";
import { EmptyBoard } from "./EmptyBoard";
import { NoProjects } from "./NoProjects";
import { boardFilter, filterOf } from "./filter";
import {
  boardPrefs,
  changePrefs,
  prefsOf,
  rememberBoardScroll,
  rememberColumnScroll,
  scrollPlace,
  toggleStripFold,
} from "./prefs";
import { markCards, revealCard, type StripHover } from "./strip/highlight";
import { Strip, type StripHandlers } from "./strip/Strip";

/** The service events that change what the board shows. */
export const BOARD_EVENTS = [
  "ticket_changed",
  "card_status_changed",
  "ticket_deleted",
  "projects_changed",
  "session_state",
  "messages_waiting",
  "inbox_changed",
  "usage_changed",
  "run_changed",
  "run_stopped",
  "settings_changed",
  // A renamed Project agent: the cards it created name it (T-0243).
  "project_agent_changed",
];

/** Cards say how long ago their last signal came ("12s"); the board refetches this often to keep that true. */
const SIGNAL_REFRESH_MS = 15_000;

/** The board's size settles this long after a resize before it is sent (the Done rail, Dev's fold). */
const RESIZE_SETTLE_MS = 120;

/** A failed request as a toast: the service's own words. */
export function failureToast(error: unknown): NewToast {
  return { level: "error", title: asRequestError(error).message, text: null, link: null, ticket: null };
}

function toast(error: unknown, ask: typeof request = request) {
  ask("notices.toast", failureToast(error)).catch(() => undefined);
}

/** The board's size, settled. */
function useSize(element: HTMLElement | null): { width: number; height: number } | null {
  const [size, setSize] = useState<{ width: number; height: number } | null>(null);
  useLayoutEffect(() => {
    if (!element) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let first = true;
    const apply = () => {
      setSize((old) => {
        const next = { width: element.clientWidth, height: element.clientHeight };
        return old?.width === next.width && old.height === next.height ? old : next;
      });
    };
    // The observer reports the size once at the start (taken at once), then on each resize (taken once settled).
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      if (first) {
        first = false;
        apply();
        return;
      }
      timer = setTimeout(apply, RESIZE_SETTLE_MS);
    });
    observer.observe(element);
    return () => {
      clearTimeout(timer);
      observer.disconnect();
    };
  }, [element]);
  return size;
}

/** Refetches `load`'s view on a timer too, for the cards' signal ages. */
function useTick(ms: number): number {
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => {
      setTick((t) => t + 1);
    }, ms);
    return () => {
      clearInterval(timer);
    };
  }, [ms]);
  return tick;
}

/** Opens the Park confirmation; its button sends `board.park` (CLS-06). */
export function confirmPark(dialog: ParkDialog, ask: typeof request = request): void {
  openOverlay("confirm", {
    title: dialog.title,
    body: dialog.message,
    rows: dialog.kept,
    confirmLabel: dialog.confirm,
    confirmIcon: dialog.confirm_icon,
    destructive: false,
    onConfirm: () => {
      const params: BoardTicketParams = { key: dialog.key };
      ask("board.park", params).catch((error: unknown) => {
        toast(error, ask);
      });
    },
  });
}

/** An approve choice opens its dialog (T-0226's Local merge, T-0227's Open PR / Push to PR). */
export function openApproveChoice(choice: Choice, ticket: string): void {
  switch (choice) {
    case "local_merge":
      openOverlay("local_merge", { ticket });
      return;
    case "open_pr":
    case "push_to_pr":
      openOverlay("open_pr", { ticket });
      return;
  }
}

/** What happens after a drop, as Rust answered it. */
export function afterDrop(
  outcome: DropOutcome,
  showApprove: (menu: ApproveMenu) => void,
  ask: typeof request = request,
): void {
  switch (outcome.kind) {
    case "nothing":
    case "moved":
      return;
    case "park":
      confirmPark(outcome.dialog, ask);
      return;
    case "approve":
      showApprove(outcome.menu);
      return;
    case "not_yet": {
      const notice: NewToast = {
        level: "success",
        title: outcome.message,
        text: null,
        link: null,
        ticket: null,
      };
      ask("notices.toast", notice).catch(() => undefined);
      return;
    }
  }
}

/** Sends a drop to Rust and carries out its answer. */
export function dropCard(
  key: string,
  column: BoardColumnId,
  showApprove: (menu: ApproveMenu) => void,
  ask: typeof request = request,
): Promise<void> {
  const params: BoardDropParams = { key, column };
  return ask("board.drop", params).then(
    (outcome) => {
      afterDrop(outcome as DropOutcome, showApprove, ask);
    },
    (error: unknown) => {
      toast(error, ask);
    },
  );
}

/** A row of the epic card, or a "not queued" chip, while it is dragged out (*03 → epic flows 3 → 8 · Dragged row*). */
function LiftedChild({ row }: { row: LiftedRow }) {
  return (
    <div className="board-ghost-row">
      <Icon name="grip-vertical" size={12} />
      <span className="board-ghost-row-key">{row.key}</span>
      {row.title !== null && <span className="board-ghost-row-title">{row.title}</span>}
    </div>
  );
}

function Lifted({ card }: { card: BoardCard }) {
  const state = { ...RESTING, dragging: true };
  return card.kind === "ticket" ? (
    <TicketCard card={card.card} state={state} />
  ) : (
    <EpicCard card={card.card} state={state} collapsed />
  );
}

export function Board() {
  const { project, ticket: selected } = useSelection();
  const prefs = prefsOf(useStore(boardPrefs), project ?? "");
  const filter = filterOf(useStore(boardFilter), project);
  const [element, setElement] = useState<HTMLDivElement | null>(null);
  const [hovered, setHovered] = useState<string | null>(null);
  const [stripHover, setStripHover] = useState<StripHover>(null);
  const boardEl = useRef<HTMLDivElement | null>(null);
  const size = useSize(element);
  const tick = useTick(SIGNAL_REFRESH_MS);

  const load = useCallback(() => {
    if (project === null || size === null) return new Promise<never>(() => undefined);
    const params: BoardParams = {
      project,
      width: size.width,
      height: size.height,
      done_collapsed: prefs.doneCollapsed,
      dev_unfolded: prefs.devUnfolded,
      filter: { tokens: filter.tokens, text: filter.text },
    };
    return (request("board.view", params) as Promise<BoardView>).then((board) => ({ project, board }));
    // `tick` refetches for the signal ages.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- tick is a refetch trigger, not read
  }, [project, size, prefs.doneCollapsed, prefs.devUnfolded, filter, tick]);
  const loaded = useServiceView(load, BOARD_EVENTS).view;
  // Another project's board is never drawn while the shown one loads.
  const view = loaded?.project === project ? loaded.board : undefined;

  // The drag: state for what the columns draw, the pointer only in a ref (the lifted card follows it unrendered).
  const [drag, setDrag] = useState<DragState | null>(null);
  const [approve, setApprove] = useState<{ menu: ApproveMenu; anchor: Anchor } | null>(null);
  const [cardMenu, setCardMenu] = useState<OpenCardMenu | null>(null);
  const ghost = useRef<HTMLDivElement>(null);
  const columns = useRef(new Map<BoardColumnId, HTMLElement>());
  const pointer = useRef({ x: 0, y: 0 });

  const showApprove = useCallback((menu: ApproveMenu) => {
    const approved = columns.current.get("approved")?.getBoundingClientRect();
    const anchor = approved
      ? { left: approved.left + 8, top: approved.top + 40, width: approved.width - 16, height: 0 }
      : { left: pointer.current.x, top: pointer.current.y, width: 0, height: 0 };
    setApprove({ menu, anchor });
  }, []);

  const controller = useMemo(
    () =>
      createDrag(request, {
        onState: setDrag,
        onError: (error) => {
          toast(error);
        },
      }),
    [],
  );

  const placeGhost = useCallback((x: number, y: number) => {
    pointer.current = { x, y };
    if (ghost.current) ghost.current.style.transform = `translate(${String(x)}px, ${String(y)}px)`;
  }, []);

  const spans = useCallback(
    (): ColumnSpan[] =>
      [...columns.current.entries()].map(([id, el]) => {
        const r = el.getBoundingClientRect();
        return { id, left: r.left, right: r.right };
      }),
    [],
  );

  const release = useCallback(() => {
    const done = controller.release();
    if (done.kind === "click") openTicket(done.key);
    if (done.kind === "drop") void dropCard(done.key, done.column, showApprove);
  }, [controller, showApprove]);

  // The pointer is followed on the window while pressed, so a drag keeps going over anything.
  useEffect(() => {
    const move = (e: globalThis.PointerEvent) => {
      if (!controller.move(e.clientX, e.clientY, spans)) return;
      e.preventDefault();
      placeGhost(e.clientX, e.clientY);
    };
    const up = () => {
      release();
    };
    const lost = () => {
      controller.cancel();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", lost);
    window.addEventListener("blur", lost);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", lost);
      window.removeEventListener("blur", lost);
    };
  }, [controller, spans, release, placeGhost]);

  // Esc while dragging: the card flies back (before the panel's Close takes the key).
  const cancelDrag = useCallback(() => {
    if (controller.state() === null) return false;
    controller.cancel();
    return true;
  }, [controller]);
  useShortcut("close_panel", cancelDrag);

  const order = useMemo(() => view?.columns.map((c) => c.id) ?? [], [view]);
  const keys = useMemo(() => view?.columns.map((c) => cardsOf(c).map((card) => card.card.key)) ?? [], [view]);
  const keysRef = useRef(keys);
  const orderRef = useRef(order);
  useLayoutEffect(() => {
    keysRef.current = keys;
    orderRef.current = order;
  }, [keys, order]);

  // The right-click menu: its items are asked when it opens, so they follow the ticket as it is now.
  const openMenu = useCallback(
    (key: string, at: { left: number; top: number }) => {
      if (controller.state() !== null) return;
      const params: BoardTicketParams = { key };
      (request("board.menu", params) as Promise<MenuItem[]>).then(
        (items) => {
          setCardMenu({ key, items, anchor: { left: at.left, top: at.top, width: 0, height: 0 } });
        },
        (error: unknown) => {
          toast(error);
        },
      );
    },
    [controller],
  );

  const handlers: CardHandlers = useMemo(
    () => ({
      press: (key, _column, e: PointerEvent) => {
        // Buttons on the card (actions, options, chips) keep their own clicks.
        if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
        controller.press(key, e.clientX, e.clientY);
        placeGhost(e.clientX, e.clientY);
      },
      pressChild: (key, e: PointerEvent) => {
        if (e.button !== 0) return;
        controller.press(key, e.clientX, e.clientY);
        placeGhost(e.clientX, e.clientY);
      },
      key: (key, column, e: KeyboardEvent) => {
        if (e.target !== e.currentTarget) return;
        const lifted = controller.state();
        if (e.key === " ") {
          e.preventDefault();
          if (lifted === null) controller.lift(key, column);
          else release();
          return;
        }
        if (e.key === "ArrowLeft" || e.key === "ArrowRight" || e.key === "ArrowUp" || e.key === "ArrowDown") {
          e.preventDefault();
          if (lifted !== null) {
            if (e.key === "ArrowLeft" || e.key === "ArrowRight")
              controller.step(e.key === "ArrowLeft" ? -1 : 1, orderRef.current);
            return;
          }
          const col = orderRef.current.indexOf(column);
          const row = keysRef.current[col]?.indexOf(key) ?? -1;
          const next = moveFocus(keysRef.current, col, row, e.key);
          const target = next && keysRef.current[next.col]?.[next.row];
          if (target) document.querySelector<HTMLElement>(`[data-card="${CSS.escape(target)}"]`)?.focus();
          return;
        }
        if (e.key === "Enter") {
          e.preventDefault();
          openTicket(key);
          return;
        }
        // The keyboard's menu key (or ⇧F10) opens the right-click menu under the card.
        if (e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey)) {
          e.preventDefault();
          const r = e.currentTarget.getBoundingClientRect();
          openMenu(key, { left: r.left + 12, top: r.top + 24 });
        }
      },
      open: (key) => {
        openTicket(key);
      },
      action: (key, kind: CardActionKind) => {
        openTicket(key);
        requestCardAction(key, kind);
      },
      addToEpic: (epic) => {
        openOverlay("new_ticket", { epic });
      },
      menu: openMenu,
      hover: (key, over) => {
        setHovered((old) => (over ? key : old === key ? null : old));
      },
    }),
    [controller, release, placeGhost, openMenu],
  );

  // ⇧⌘D: start the selected ticket, as if it was dropped on Dev. On an epic (its card, or its track's name) it queues
  // the children still in Backlog; on a child row only that child (EPC-12, d0058).
  const startSelected = useCallback(() => {
    if (selected === null) return false;
    void dropCard(selected, "dev", showApprove);
    return true;
  }, [selected, showApprove]);
  useShortcut("start_dev", startSelected);

  const element$ = useCallback((id: BoardColumnId, el: HTMLElement | null) => {
    if (el) columns.current.set(id, el);
    else columns.current.delete(id);
  }, []);

  const place = scrollPlace(project ?? "");
  // The board's own sideways scroll comes back with its project too.
  const hasView = view !== undefined;
  useLayoutEffect(() => {
    const el = boardEl.current;
    if (el && hasView) el.scrollLeft = scrollPlace(project ?? "").left;
  }, [element, project, hasView]);
  const onScroll = useCallback(
    (id: BoardColumnId, top: number) => {
      if (project !== null) rememberColumnScroll(project, id, top);
    },
    [project],
  );

  const draggedCard = useMemo(() => {
    if (drag === null || !view) return null;
    for (const c of view.columns) {
      const found = cardsOf(c).find((card) => card.card.key === drag.key);
      if (found) return found;
    }
    return null;
  }, [drag, view]);
  const draggedRow = useMemo(() => {
    if (drag === null || !view || draggedCard !== null) return null;
    const epics = view.columns.flatMap((c) =>
      cardsOf(c).flatMap((card) => (card.kind === "epic" ? [card.card] : [])),
    );
    return liftedRow(drag.key, epics, view.strip?.tracks ?? []);
  }, [drag, view, draggedCard]);

  // The strip's hover marks the cards (a stop's outline, a name's preview), again whenever the cards are redrawn.
  const hasStrip = view?.strip != null;
  useLayoutEffect(() => {
    if (!element || !hasStrip || drag !== null) return;
    return markCards(element, stripHover);
  }, [element, stripHover, view, hasStrip, drag]);

  const stripHandlers: StripHandlers = useMemo(
    () => ({
      hover: (hover) => {
        setStripHover(hover);
        if (hover?.kind === "stop" && boardEl.current) revealCard(boardEl.current, hover.key);
      },
      select: (key) => {
        openTicket(key);
      },
      // Focusing the epic is T-0274's; until then a click on the name selects the epic.
      name: (epic) => {
        openTicket(epic);
      },
      open: (epic) => {
        openTicket(epic);
      },
      press: (key, e) => {
        controller.press(key, e.clientX, e.clientY);
        placeGhost(e.clientX, e.clientY);
      },
      queue: (key) => {
        void dropCard(key, "ready_for_dev", showApprove);
      },
    }),
    [controller, placeGhost, showApprove],
  );

  const overVerdict = drag ? verdictOf(drag.answer, drag.over) : undefined;
  const chain = useMemo(
    () => activeChain(view?.chains ?? [], hovered, selected, drag !== null),
    [view, hovered, selected, drag],
  );

  // Keyboard drags draw the lifted card over the column it is in.
  useLayoutEffect(() => {
    if (!drag?.keyboard || drag.over === null || !ghost.current) return;
    const r = columns.current.get(drag.over)?.getBoundingClientRect();
    if (r) ghost.current.style.transform = `translate(${String(r.left + 24)}px, ${String(r.top + 80)}px)`;
  }, [drag]);

  return (
    <div
      className="board"
      ref={(el) => {
        boardEl.current = el;
        setElement(el);
      }}
      data-dragging={drag !== null || undefined}
      onScroll={(e) => {
        if (project !== null) rememberBoardScroll(project, e.currentTarget.scrollLeft);
      }}
      // A card that moved away under the pointer never reports leaving; leaving the board ends its highlight.
      onPointerLeave={() => {
        setHovered(null);
      }}
    >
      {view?.strip && (
        <Strip
          strip={view.strip}
          columns={view.columns}
          folded={prefs.stripFolded}
          onToggleFold={() => {
            if (project !== null) toggleStripFold(project);
          }}
          pulse={hovered}
          handlers={stripHandlers}
        />
      )}
      <div className="board-columns">
        {view?.columns.map((column) => (
          <Column
            key={`${project ?? ""}:${column.id}`}
            column={column}
            chipWords={view.scroll_chip}
            verdict={drag ? verdictOf(drag.answer, column.id) : undefined}
            dragging={drag !== null}
            selected={selected}
            chain={chain}
            lifted={drag?.key ?? null}
            handlers={handlers}
            showEmptyHint={!view.empty}
            scrollTop={place.columns[column.id] ?? 0}
            onScroll={onScroll}
            onToggleDone={() => {
              if (project !== null) changePrefs(project, (p) => ({ ...p, doneCollapsed: !column.collapsed }));
            }}
            onUnfold={() => {
              if (project !== null) changePrefs(project, (p) => ({ ...p, devUnfolded: true }));
            }}
            onAdd={() => {
              openOverlay("new_ticket");
            }}
            element={element$}
          />
        ))}
      </div>
      {view?.empty && <EmptyBoard view={view.empty} />}
      {project === null && <NoProjects />}
      <ChainLayer chain={chain} board={element} layout={view} />
      <div className="board-ghost" ref={ghost} hidden={drag === null}>
        {draggedCard && (
          <div className="board-ghost-card">
            <Lifted card={draggedCard} />
          </div>
        )}
        {draggedRow && <LiftedChild row={draggedRow} />}
        {overVerdict?.kind === "allowed" && overVerdict.hint !== "" && (
          <div className="board-drop-hint">
            <Icon name={overVerdict.icon} size={11} />
            <span>{overVerdict.hint}</span>
          </div>
        )}
        {overVerdict?.kind === "forbidden" && (
          <div className="board-not-allowed" role="status">
            <Icon name="ban" size={18} />
            <span className="board-not-allowed-title">{overVerdict.title}</span>
            {overVerdict.detail !== null && (
              <span className="board-not-allowed-detail">{overVerdict.detail}</span>
            )}
          </div>
        )}
        {overVerdict && overVerdict.kind !== "home" && (
          <span className="board-cursor" data-verdict={overVerdict.kind}>
            <Icon name={overVerdict.kind === "allowed" ? "plus" : "ban"} size={12} />
          </span>
        )}
      </div>
      {cardMenu && (
        <CardMenu
          menu={cardMenu}
          onError={(error) => {
            toast(error);
          }}
          onClose={() => {
            setCardMenu(null);
          }}
        />
      )}
      {approve && (
        <ApproveChoice
          menu={approve.menu}
          anchor={approve.anchor}
          onChoose={(choice) => {
            setApprove(null);
            openApproveChoice(choice, approve.menu.key);
          }}
          onCancel={() => {
            setApprove(null);
          }}
        />
      )}
    </div>
  );
}
