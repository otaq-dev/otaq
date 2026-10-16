// The strip's plain parts (EPC-10, T-0269): which cell of a track sits over which column (the Track Label over
// Backlog, a station per other column, the 34 px Done rail's station), how many stops a station shows and whether
// they drop their words (*Note · Narrow window*: below ~200 px per column a stop keeps icon + key, a needs-you stop
// keeps "asks you"), the fold, and the colours of each state. Everything the strip says is Rust's (`EpicStrip`).
import type { BoardColumn } from "../../../generated/BoardColumn";
import type { BoardColumnId } from "../../../generated/BoardColumnId";
import type { EpicStation } from "../../../generated/EpicStation";
import type { EpicStop } from "../../../generated/EpicStop";
import type { EpicStopState } from "../../../generated/EpicStopState";
import type { EpicTrack } from "../../../generated/EpicTrack";
import type { EpicTrackChip } from "../../../generated/EpicTrackChip";

/** One cell of a track, in the order and width of the board's columns. */
export type TrackCell =
  | { kind: "label"; column: BoardColumnId }
  | { kind: "station"; column: BoardColumnId; station: EpicStation }
  /** Done folded to its 34 px rail: the station shows only "n merged". */
  | { kind: "rail"; column: BoardColumnId; station: EpicStation };

const EMPTY = (column: BoardColumnId): EpicStation => ({ column, stops: [], merged: null });

/** The cells of `track` over `columns` (the page's columns, in board order): the label over Backlog, each other
 * column its station (an empty one if the track has none for it), a collapsed column its rail. */
export function trackCells(
  columns: readonly Pick<BoardColumn, "id" | "collapsed">[],
  track: Pick<EpicTrack, "stations">,
): TrackCell[] {
  return columns.map(({ id, collapsed }) => {
    if (id === "backlog") return { kind: "label", column: id };
    const station = track.stations.find((s) => s.column === id) ?? EMPTY(id);
    return collapsed ? { kind: "rail", column: id, station } : { kind: "station", column: id, station };
  });
}

/** Below this many px per column a stop drops its words (*Note · Narrow window*). */
export const NARROW_COLUMN = 200;

// A stop's parts as the Track Stop draws them, to tell whether a station's stops fit (an estimate: the stops are
// mono keys and short words, so a width per character is close enough).
const STOP_FRAME = 16 + 11 + 4; // the padding and border, the icon, the gap after it
const KEY_CHAR = 6.7; // 11 px mono
const NOTE_CHAR = 5.9; // 11 px UI
const NEW_CHAR = 5.6; // 10 px UI, semibold
const NEW_FRAME = 4 + 8; // the gap before the flag, its padding
const STATION_LEAD = 8; // the route before the first stop
const STOP_GAP = 6;
/** The "+2" chip after the stops that don't fit. */
export const MORE_WIDTH = 30;

/** The key a stop shows ("40.3"): its label without the note. */
export function stopKey(stop: Pick<EpicStop, "label" | "note">): string {
  return stop.note === null
    ? stop.label
    : stop.label.slice(0, stop.label.length - stop.note.length).trimEnd();
}

/** Whether a stop shows its note: always at full width; when short only a needs-you one ("asks you"). */
export function showsNote(stop: Pick<EpicStop, "note" | "needs_you">, short: boolean): boolean {
  return stop.note !== null && (!short || stop.needs_you);
}

/** About how wide a stop is drawn. */
export function stopWidth(stop: EpicStop, short: boolean): number {
  let width = STOP_FRAME + stopKey(stop).length * KEY_CHAR;
  if (showsNote(stop, short) && stop.note !== null) width += 4 + stop.note.length * NOTE_CHAR;
  const flag = short ? stop.new_short : stop.new;
  if (flag !== null) width += NEW_FRAME + flag.length * NEW_CHAR;
  return width;
}

function rowWidth(stops: readonly EpicStop[], short: boolean): number {
  return stops.reduce((sum, stop) => sum + stopWidth(stop, short) + STOP_GAP, STATION_LEAD);
}

/** How a station draws its stops in `width` px: `short` drops their words, `shown` is how many fit (the rest are
 * a "+n" chip, their tooltips on it). */
export interface StationFit {
  short: boolean;
  shown: number;
}

export function fitStation(stops: readonly EpicStop[], width: number): StationFit {
  if (width >= NARROW_COLUMN && rowWidth(stops, false) <= width) return { short: false, shown: stops.length };
  if (rowWidth(stops, true) <= width) return { short: true, shown: stops.length };
  let shown = stops.length;
  while (shown > 1 && rowWidth(stops.slice(0, shown), true) + MORE_WIDTH > width) shown -= 1;
  return { short: true, shown };
}

/** How many "not queued" chips the label shows; the rest are a "+n" chip. */
export const LABEL_CHIPS = 1;

export function labelChips(
  chips: readonly EpicTrackChip[],
  max = LABEL_CHIPS,
): { shown: EpicTrackChip[]; more: EpicTrackChip[] } {
  return { shown: chips.slice(0, max), more: chips.slice(max) };
}

/** The keys of a track's cards: what stays bright while its name is hovered (the focus preview); the others dim. */
export function trackCards(track: Pick<EpicTrack, "key" | "stations">): string[] {
  return [track.key, ...track.stations.flatMap((s) => s.stops.map((stop) => stop.key))];
}

/** The keyboard's way along a track: the name (index 0), then its stops in board order. */
export function trackStops(track: Pick<EpicTrack, "stations">): string[] {
  return track.stations.flatMap((s) => s.stops.map((stop) => stop.key));
}

/** ←/→ from `index` among `count` places of a track (the name and its stops), stopping at the ends. */
export function stepAlong(index: number, count: number, key: "ArrowLeft" | "ArrowRight"): number {
  return key === "ArrowLeft" ? Math.max(index - 1, 0) : Math.min(index + 1, count - 1);
}

/** A stop's (or chip's) icon colour. A needs-you stop is turquoise whatever its state. */
export function stopInk(state: EpicStopState, needsYou: boolean): string {
  if (needsYou) return "var(--st-attn)";
  switch (state) {
    case "not_queued":
    case "closed":
      return "var(--text-muted)";
    case "next":
    case "waits":
      return "var(--st-queued)";
    case "working":
      return "var(--st-working)";
    case "needs_you":
      return "var(--st-attn)";
    case "parked":
    case "conflict":
      return "var(--st-paused)";
    case "in_review":
      return "var(--st-review)";
    case "merged":
      return "var(--st-done)";
    case "lead_note":
      return "var(--st-investigate)";
  }
}

/** A mini-bar segment's colour (*Track Label → Bar*). */
export function segmentInk(state: EpicStopState): string {
  if (state === "not_queued" || state === "closed") return "var(--border-strong)";
  if (state === "merged") return "var(--st-approved)";
  // Back at work after the lead's note: a working segment.
  if (state === "lead_note") return "var(--st-working)";
  return stopInk(state, false);
}

/** The mini bar's width: 6 px a segment, 2 px between (the frame's 30 px for four). */
export function barWidth(segments: number): number {
  return Math.max(segments * 8 - 2, 0);
}
