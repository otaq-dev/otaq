// The board's moment of interaction, without the DOM (UIT-04): a drag asks Rust once, when it starts (`board.drag`,
// every column's verdict from the core's `check_move`), and from then on only this file works while the pointer or
// the arrow keys move: which column is under the card, how it looks, whether a release drops. The verdicts and
// their words are Rust's; nothing here decides a rule.
import type { BoardColumnId } from "../../generated/BoardColumnId";
import type { BoardScrollChip } from "../../generated/BoardScrollChip";
import type { BoardTicketParams } from "../../generated/BoardTicketParams";
import type { DragAnswer } from "../../generated/DragAnswer";
import type { DragVerdict } from "../../generated/DragVerdict";
import type { EpicCardView } from "../../generated/EpicCardView";
import type { EpicTrack } from "../../generated/EpicTrack";

/** The door's request call, or a fake one in the tests. */
export type Ask = (method: string, params?: unknown) => Promise<unknown>;

/** How far the pointer moves with the button down before a press becomes a drag (a shorter press is a click). */
export const DRAG_THRESHOLD = 4;

/** A column's left and right edge on screen, measured when the drag starts. */
export interface ColumnSpan {
  id: BoardColumnId;
  left: number;
  right: number;
}

/** The column under `x`; the gaps between columns belong to none. */
export function columnAt(spans: readonly ColumnSpan[], x: number): BoardColumnId | null {
  return spans.find((s) => x >= s.left && x < s.right)?.id ?? null;
}

/** How `column` looks for the dragged card; `undefined` until Rust's answer is in, or over no column. */
export function verdictOf(answer: DragAnswer | null, column: BoardColumnId | null): DragVerdict | undefined {
  if (answer === null || column === null) return undefined;
  return answer.columns.find((c) => c.column === column)?.verdict;
}

/** The drag the board draws: the card, the column under it and Rust's answer once it came. */
export interface DragState {
  key: string;
  over: BoardColumnId | null;
  answer: DragAnswer | null;
  /** Picked up with Space: ←/→ move it, Space drops. */
  keyboard: boolean;
}

/** What a release did. */
export type Release =
  /** Too short to be a drag: a click on the card. */
  | { kind: "click"; key: string }
  /** Dropped on a column whose verdict allows it: the board sends `board.drop`. */
  | { kind: "drop"; key: string; column: BoardColumnId }
  /** Nothing: its own column, a forbidden one, no column, or Rust hadn't answered yet. The card flies back. */
  | { kind: "none" };

export interface DragCallbacks {
  /** The drag started, changed column, got its answer, or ended (`null`). Not called for every pointer move. */
  onState: (state: DragState | null) => void;
  onError?: (error: unknown) => void;
}

export interface Drag {
  /** The button went down on a card. With `clicks` false a short press is no click (a track's name has its own). */
  press: (key: string, x: number, y: number, clicks?: boolean) => void;
  /** The pointer moved; `spans` measures the columns once, when the press turns into a drag. Whether a pointer drag
   * is on (the caller then moves the lifted card to the pointer). */
  move: (x: number, y: number, spans: () => readonly ColumnSpan[]) => boolean;
  release: () => Release;
  /** Esc, or the pointer lost: the card flies back. */
  cancel: () => void;
  /** Space on a focused card in `column`: picks it up without the pointer. */
  lift: (key: string, column: BoardColumnId) => void;
  /** ←/→ while lifted: the next column in `order`, stopping at the ends. */
  step: (delta: -1 | 1, order: readonly BoardColumnId[]) => void;
  state: () => DragState | null;
}

export function createDrag(ask: Ask, callbacks: DragCallbacks): Drag {
  let pressed: { key: string; x: number; y: number; clicks: boolean } | null = null;
  let spans: readonly ColumnSpan[] = [];
  let state: DragState | null = null;

  const set = (next: DragState | null) => {
    state = next;
    callbacks.onState(next);
  };

  const start = (key: string, over: BoardColumnId | null, keyboard: boolean) => {
    set({ key, over, answer: null, keyboard });
    // The one request of the drag (UIT-04).
    const params: BoardTicketParams = { key };
    ask("board.drag", params).then(
      (answer) => {
        if (state?.key === key) set({ ...state, answer: answer as DragAnswer });
      },
      (error: unknown) => {
        callbacks.onError?.(error);
      },
    );
  };

  const end = (): Release => {
    const current = state;
    pressed = null;
    spans = [];
    if (current !== null) set(null);
    if (current === null || current.over === null) return { kind: "none" };
    return verdictOf(current.answer, current.over)?.kind === "allowed"
      ? { kind: "drop", key: current.key, column: current.over }
      : { kind: "none" };
  };

  return {
    press(key, x, y, clicks = true) {
      pressed = { key, x, y, clicks };
    },
    move(x, y, measure) {
      if (state === null) {
        if (pressed === null || Math.hypot(x - pressed.x, y - pressed.y) < DRAG_THRESHOLD) return false;
        spans = measure();
        start(pressed.key, columnAt(spans, x), false);
      }
      if (state === null || state.keyboard) return state !== null;
      const over = columnAt(spans, x);
      if (over !== state.over) set({ ...state, over });
      return true;
    },
    release() {
      if (state === null) {
        const click = pressed;
        pressed = null;
        return click?.clicks === true ? { kind: "click", key: click.key } : { kind: "none" };
      }
      return end();
    },
    cancel() {
      pressed = null;
      spans = [];
      if (state !== null) set(null);
    },
    lift(key, column) {
      pressed = null;
      start(key, column, true);
    },
    step(delta, order) {
      if (state === null) return;
      const at = state.over === null ? -1 : order.indexOf(state.over);
      const next = order[Math.min(order.length - 1, Math.max(0, at + delta))] ?? null;
      if (next !== state.over) set({ ...state, over: next });
    },
    state: () => state,
  };
}

/** A dragged epic track, by its name (EPC-14): the lifted handle draws its name and External ID. */
export function liftedTrack(key: string, tracks: readonly EpicTrack[]): EpicTrack | null {
  return tracks.find((t) => t.key === key) ?? null;
}

/** A dragged child that is no card (EPC-12): a row of the epic card (with its title) or a "not queued" chip. */
export interface LiftedRow {
  key: string;
  title: string | null;
}

/**
 * What the lifted ghost draws for `key` when it is no card on the board: a row of one of the `epics` cards, or a
 * chip on one of the `tracks`; `null` for anything else (a card draws itself).
 */
export function liftedRow(
  key: string,
  epics: readonly EpicCardView[],
  tracks: readonly EpicTrack[],
): LiftedRow | null {
  for (const epic of epics) {
    const row = epic.children.find((c) => c.key === key && c.draggable);
    if (row) return { key, title: row.title };
  }
  return tracks.some((t) => t.not_queued.some((c) => c.key === key)) ? { key, title: null } : null;
}

/** `"{n} above"` with `n` filled in. */
export function fill(template: string, n: number): string {
  return template.replace("{n}", String(n));
}

export interface AboveChip {
  text: string;
  /** Some of them need the user: the turquoise chevron. */
  needYou: boolean;
}

/**
 * The chip of a scrolled column: "7 above · 6 need you", or "3 above"; `null` when no card is above the view.
 * `above` is, for each card scrolled out above, whether it needs the user.
 */
export function aboveChip(words: BoardScrollChip, above: readonly boolean[]): AboveChip | null {
  if (above.length === 0) return null;
  const needYou = above.filter(Boolean).length;
  const count = fill(words.above, above.length);
  return {
    text: needYou === 0 ? count : `${count} · ${fill(words.need_you, needYou)}`,
    needYou: needYou > 0,
  };
}

/** Where focus goes from card `row` of column `col` on an arrow key, over the columns' card keys; `null` stays. */
export function moveFocus(
  columns: readonly (readonly string[])[],
  col: number,
  row: number,
  key: "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight",
): { col: number; row: number } | null {
  if (key === "ArrowUp" || key === "ArrowDown") {
    const next = row + (key === "ArrowUp" ? -1 : 1);
    return next >= 0 && next < (columns[col]?.length ?? 0) ? { col, row: next } : null;
  }
  const step = key === "ArrowLeft" ? -1 : 1;
  for (let c = col + step; c >= 0 && c < columns.length; c += step) {
    const length = columns[c]?.length ?? 0;
    if (length > 0) return { col: c, row: Math.min(row, length - 1) };
  }
  return null;
}
