// The "Epics in flight" strip above the columns (EPC-10, d0080; *03 · Board — shop-web · 1440 → Epics in flight*,
// *01 · Components · Epics on the board (0080)* → Strip Header, Track Label, Track Stop; *04 → Row · Epics in flight
// strip (0080)*): the header (a click folds the strip to its one line, kept per project), then one track per epic
// in flight: its label over Backlog, then a station under each other column with its stops, cell for cell as wide
// as the columns below (the same flex rules, the Done rail's 34 px). Hovering a stop outlines its card, hovering a
// name previews the focus, a stop pulses while its card is hovered; a click on a stop selects its card, a click on
// the name selects the epic (focus is T-0274's), a double click or ↵ opens the epic panel. ←/→ walk a track. A
// "not queued" (or parked) chip shows its Queue (Resume) button on hover and can be dragged to Ready for Dev; both
// queue only that child (EPC-12, T-0270). A track is dragged by its name: onto Backlog it takes back, onto Ready
// for Dev or Dev it queues the rest (EPC-14, T-0271).
import { type KeyboardEvent, type PointerEvent, useLayoutEffect, useState } from "react";

import { Tooltip } from "../../../components";
import type { BoardColumn } from "../../../generated/BoardColumn";
import type { EpicStation } from "../../../generated/EpicStation";
import type { EpicStop } from "../../../generated/EpicStop";
import type { EpicStrip } from "../../../generated/EpicStrip";
import type { EpicTrack } from "../../../generated/EpicTrack";
import type { EpicTrackChip } from "../../../generated/EpicTrackChip";
import { Icon } from "../../../theme/Icon";
import type { StripHover } from "./highlight";
import {
  barWidth,
  fitStation,
  labelChips,
  segmentInk,
  showsNote,
  stepAlong,
  stopInk,
  stopKey,
  trackCards,
  trackCells,
} from "./layout";
import "./strip.css";

export interface StripHandlers {
  hover: (hover: StripHover) => void;
  /** A stop's card, selected as a click on the card would. */
  select: (key: string) => void;
  /** A click on a track's name (T-0274 makes it focus the epic). */
  name: (epic: string) => void;
  /** Double click or ↵ on a track's name: the epic panel. */
  open: (epic: string) => void;
  /** The button went down on a chip: its child may be dragged to Ready for Dev (EPC-12). */
  press: (key: string, e: PointerEvent) => void;
  /** The button went down on a track's name: the epic may be dragged (EPC-14); a click stays the name's. */
  pressName: (epic: string, e: PointerEvent) => void;
  /** A chip's Queue (or Resume) button: only that child, as a drop on Ready for Dev. */
  queue: (key: string) => void;
}

export interface StripProps {
  strip: EpicStrip;
  /** The board's columns, for the cells' order and the Done rail. */
  columns: readonly Pick<BoardColumn, "id" | "collapsed">[];
  folded: boolean;
  onToggleFold: () => void;
  /** The card under the pointer: its stop pulses. */
  pulse: string | null;
  handlers: StripHandlers;
}

/** The width a station had when the strip was last measured (all stations are as wide as one column). */
function useStationWidth(root: HTMLElement | null): number {
  const [width, setWidth] = useState(Number.POSITIVE_INFINITY);
  useLayoutEffect(() => {
    if (!root) return;
    const measure = () => {
      const station = root.querySelector<HTMLElement>(".strip-station");
      if (station) setWidth(station.clientWidth);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(root);
    return () => {
      observer.disconnect();
    };
  }, [root]);
  return width;
}

/** ←/→ along a track: its name, then its stops. */
function walk(e: KeyboardEvent<HTMLElement>) {
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  const track = e.currentTarget.closest(".strip-track");
  if (!track) return;
  const places = [...track.querySelectorAll<HTMLElement>("[data-strip-place]")];
  const at = places.indexOf(e.currentTarget);
  if (at < 0) return;
  e.preventDefault();
  places[stepAlong(at, places.length, e.key)]?.focus();
}

function Chip({ chip, handlers }: { chip: EpicTrackChip; handlers: StripHandlers }) {
  return (
    <Tooltip text={chip.tooltip}>
      <span
        className="strip-chip"
        data-state={chip.state}
        data-chip={chip.key}
        aria-label={chip.tooltip}
        onPointerDown={(e) => {
          // The Queue button keeps its own click.
          if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
          handlers.press(chip.key, e);
        }}
      >
        <span style={{ color: "var(--text-muted)" }}>
          <Icon name={chip.icon} size={10} />
        </span>
        <span className="strip-chip-key">{chip.label}</span>
        <button
          type="button"
          className="strip-chip-action"
          aria-label={`${chip.action} ${chip.key}`}
          onClick={() => {
            handlers.queue(chip.key);
          }}
        >
          {chip.action}
        </button>
      </span>
    </Tooltip>
  );
}

function Label({ track, handlers }: { track: EpicTrack; handlers: StripHandlers }) {
  const chips = labelChips(track.not_queued);
  return (
    <div className="strip-label" data-needs-you={track.needs_you || undefined}>
      <button
        type="button"
        className="strip-name"
        data-strip-place=""
        aria-label={`${track.key} ${track.name}`}
        onClick={() => {
          handlers.name(track.key);
        }}
        onDoubleClick={() => {
          handlers.open(track.key);
        }}
        onPointerDown={(e) => {
          if (e.button === 0) handlers.pressName(track.key, e);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            handlers.open(track.key);
            return;
          }
          walk(e);
        }}
        onPointerEnter={() => {
          handlers.hover({ kind: "preview", keys: trackCards(track) });
        }}
        onPointerLeave={() => {
          handlers.hover(null);
        }}
      >
        <span className="strip-name-icon">
          <Icon name="layers" size={12} />
        </span>
        {track.needs_you && <span className="strip-dot" />}
        <span className="strip-name-text">{track.name}</span>
        {track.external_id !== null && <span className="strip-external">{track.external_id}</span>}
        {track.pinned && (
          <span className="strip-name-icon">
            <Icon name="pin" size={10} />
          </span>
        )}
      </button>
      <span className="strip-bar" style={{ width: barWidth(track.segments.length) }} aria-hidden>
        {track.segments.map((state, i) => (
          <span key={i} style={{ background: segmentInk(state) }} />
        ))}
      </span>
      <span className="strip-count">{track.count}</span>
      {chips.shown.map((chip) => (
        <Chip key={chip.key} chip={chip} handlers={handlers} />
      ))}
      {chips.more.length > 0 && (
        <Tooltip text={chips.more.map((c) => c.tooltip).join("\n")}>
          <span className="strip-chip" data-state="not_queued">
            <span className="strip-chip-key">+{chips.more.length}</span>
          </span>
        </Tooltip>
      )}
    </div>
  );
}

function Stop({
  stop,
  short,
  pulse,
  handlers,
}: {
  stop: EpicStop;
  short: boolean;
  pulse: boolean;
  handlers: StripHandlers;
}) {
  return (
    <Tooltip text={stop.tooltip}>
      <button
        type="button"
        className="strip-stop"
        data-strip-place=""
        data-state={stop.state}
        data-needs-you={stop.needs_you || undefined}
        data-pulse={pulse || undefined}
        tabIndex={-1}
        aria-label={stop.tooltip}
        onClick={() => {
          handlers.select(stop.key);
        }}
        onKeyDown={walk}
        onPointerEnter={() => {
          handlers.hover({ kind: "stop", key: stop.key });
        }}
        onPointerLeave={() => {
          handlers.hover(null);
        }}
        onFocus={() => {
          handlers.hover({ kind: "stop", key: stop.key });
        }}
        onBlur={() => {
          handlers.hover(null);
        }}
      >
        <span style={{ color: stopInk(stop.state, stop.needs_you) }}>
          <Icon name={stop.icon} size={11} />
        </span>
        <span className="strip-stop-key">{stopKey(stop)}</span>
        {showsNote(stop, short) && <span className="strip-stop-note">{stop.note}</span>}
        {!short && stop.new !== null && <span className="strip-new">{stop.new}</span>}
      </button>
    </Tooltip>
  );
}

function Station({
  station,
  width,
  pulse,
  handlers,
}: {
  station: EpicStation;
  width: number;
  pulse: string | null;
  handlers: StripHandlers;
}) {
  const fit = fitStation(station.stops, width);
  const hidden = station.stops.slice(fit.shown);
  return (
    <div className="strip-station" data-column={station.column}>
      {station.stops.slice(0, fit.shown).map((stop) => (
        <Stop key={stop.key} stop={stop} short={fit.short} pulse={stop.key === pulse} handlers={handlers} />
      ))}
      {hidden.length > 0 && (
        <Tooltip text={hidden.map((s) => s.tooltip).join("\n")}>
          <span className="strip-more" data-pulse={hidden.some((s) => s.key === pulse) || undefined}>
            +{hidden.length}
          </span>
        </Tooltip>
      )}
    </div>
  );
}

function Track({
  track,
  columns,
  width,
  pulse,
  handlers,
}: {
  track: EpicTrack;
  columns: StripProps["columns"];
  width: number;
  pulse: string | null;
  handlers: StripHandlers;
}) {
  return (
    <div className="strip-track" role="group" aria-label={`${track.key} ${track.name}`}>
      {trackCells(columns, track).map((cell) => {
        switch (cell.kind) {
          case "label":
            return <Label key={cell.column} track={track} handlers={handlers} />;
          case "station":
            return (
              <Station
                key={cell.column}
                station={cell.station}
                width={width}
                pulse={pulse}
                handlers={handlers}
              />
            );
          case "rail":
            return (
              <div key={cell.column} className="strip-rail" data-column={cell.column}>
                {cell.station.merged !== null && (
                  <span className="strip-rail-merged">{cell.station.merged}</span>
                )}
              </div>
            );
        }
      })}
    </div>
  );
}

export function Strip({ strip, columns, folded, onToggleFold, pulse, handlers }: StripProps) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const width = useStationWidth(folded ? null : root);
  return (
    <div className="strip" ref={setRoot} data-folded={folded || undefined}>
      <button
        type="button"
        className="strip-header"
        aria-expanded={!folded}
        aria-label={folded ? strip.folded : strip.title}
        onClick={onToggleFold}
      >
        <span className="strip-title">{strip.title}</span>
        {strip.needs_you !== null && <span className="strip-needs-you">{strip.needs_you}</span>}
        {folded &&
          strip.tracks.map((track) => (
            <span key={track.key} className="strip-mini" data-needs-you={track.needs_you || undefined}>
              <Icon name="layers" size={10} />
              <span>{track.name}</span>
            </span>
          ))}
        <span className="strip-spacer" />
        {!folded && <span className="strip-hint">{strip.hint}</span>}
        <Icon name={folded ? "chevrons-up-down" : "chevrons-down-up"} size={12} />
      </button>
      {!folded &&
        strip.tracks.map((track) => (
          <Track
            key={track.key}
            track={track}
            columns={columns}
            width={width}
            pulse={pulse}
            handlers={handlers}
          />
        ))}
    </div>
  );
}
