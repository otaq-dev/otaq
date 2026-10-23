import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import type { BoardColumnId } from "../../../generated/BoardColumnId";
import type { EpicStop } from "../../../generated/EpicStop";
import type { EpicStrip } from "../../../generated/EpicStrip";
import type { EpicTrack } from "../../../generated/EpicTrack";
import { stripFolded, stripFolds, toggleStripFold } from "./fold";
import {
  barWidth,
  fitStation,
  labelChips,
  NARROW_COLUMN,
  segmentInk,
  showsNote,
  stepAlong,
  stopInk,
  stopKey,
  stopWidth,
  trackCards,
  trackCells,
  trackStops,
} from "./layout";
import { Strip, type StripHandlers } from "./Strip";

afterEach(() => {
  stripFolds.set({});
});

function stop(
  key: string,
  state: EpicStop["state"],
  note: string | null,
  extra: Partial<EpicStop> = {},
): EpicStop {
  const short = key.replace(/^SHOP-/, "");
  const label = note === null ? short : `${short} ${note}`;
  return {
    key,
    state,
    icon: "hourglass",
    note,
    label,
    short_label: extra.needs_you ? label : short,
    tooltip: `${key} · ${note ?? state}`,
    needs_you: false,
    new: null,
    new_short: null,
    ...extra,
  };
}

const STATIONS: BoardColumnId[] = ["ready_for_dev", "dev", "review", "done"];

/** The 04 strip row's Wishlist: 40.5 new and waiting, 40.2 asks you, 40.4 conflict, 40.1 merged, 40.3 not queued. */
const WISHLIST: EpicTrack = {
  key: "SHOP-40",
  name: "Wishlist",
  external_id: "PAY-123",
  count: "1/5",
  segments: ["merged", "needs_you", "not_queued", "conflict", "waits"],
  pinned: false,
  needs_you: true,
  not_queued: [
    {
      key: "SHOP-40.3",
      label: "40.3",
      state: "not_queued",
      icon: "circle-dashed",
      tooltip: "SHOP-40.3 · not queued",
      action: "Queue",
    },
  ],
  stations: STATIONS.map((column) => ({
    column,
    stops:
      column === "ready_for_dev"
        ? [stop("SHOP-40.5", "waits", "waits 40.2", { new: "new · added by 40.2" })]
        : column === "dev"
          ? [
              stop("SHOP-40.2", "needs_you", "asks you", { needs_you: true }),
              stop("SHOP-40.4", "conflict", "conflict"),
            ]
          : column === "done"
            ? [stop("SHOP-40.1", "merged", null)]
            : [],
    merged: column === "done" ? "1 merged" : null,
    check: null,
  })),
  lead: null,
  lead_asks: false,
};

const STRIP: EpicStrip = {
  title: "Epics in flight · 1",
  needs_you: "· 1 needs you",
  hint: "Hover a ticket to find its card · Click an epic name to focus on it",
  folded: "Epics in flight 1 · 1 needs you",
  tracks: [WISHLIST],
};

const COLUMNS = (["backlog", ...STATIONS] as BoardColumnId[]).map((id) => ({ id, collapsed: false }));
const RAIL = COLUMNS.map((c) => ({ ...c, collapsed: c.id === "done" }));

const NOOP: StripHandlers = {
  hover: () => undefined,
  select: () => undefined,
  name: () => undefined,
  menu: () => undefined,
  open: () => undefined,
  press: () => undefined,
  pressName: () => undefined,
  queue: () => undefined,
};

function html(
  folded: boolean,
  columns = COLUMNS,
  pulse: string | null = null,
  focus: readonly string[] | null = null,
  strip: EpicStrip = STRIP,
): string {
  return renderToStaticMarkup(
    createElement(Strip, {
      strip,
      columns,
      folded,
      onToggleFold: () => undefined,
      pulse,
      focus,
      handlers: NOOP,
    }),
  );
}

describe("the focus (EPC-18, T-0274)", () => {
  const SEARCH: EpicTrack = { ...WISHLIST, key: "SHOP-45", name: "Search", external_id: "PAY-130" };
  const both: EpicStrip = { ...STRIP, tracks: [WISHLIST, SEARCH] };

  it("outlines the focused track's name and dims the other tracks", () => {
    const markup = html(false, COLUMNS, null, ["SHOP-40"], both);
    expect(markup).toContain('data-track="SHOP-40"');
    expect(markup).not.toMatch(/data-track="SHOP-40" data-dimmed/);
    expect(markup).toMatch(/data-track="SHOP-45" data-dimmed="true"/);
    expect(markup).toContain('data-focused="true"');
    expect(markup).toMatch(/data-epic="SHOP-40"[^>]*aria-pressed="true"/);
  });

  it("dims nothing without a focus", () => {
    const markup = html(false, COLUMNS, null, null, both);
    expect(markup).not.toContain("data-dimmed");
    expect(markup).not.toContain("data-focused");
  });
});

describe("column alignment", () => {
  it("puts the label over Backlog and a station under every other column, in board order", () => {
    const cells = trackCells(COLUMNS, WISHLIST);
    expect(cells.map((c) => [c.kind, c.column])).toEqual([
      ["label", "backlog"],
      ["station", "ready_for_dev"],
      ["station", "dev"],
      ["station", "review"],
      ["station", "done"],
    ]);
    const dev = cells[2];
    expect(dev?.kind === "station" && dev.station.stops.map((s) => s.key)).toEqual([
      "SHOP-40.2",
      "SHOP-40.4",
    ]);
  });

  it("draws a rail cell under every rail, an empty stage of a long flow too (FLW-12)", () => {
    const columns = [
      { id: "backlog", collapsed: false },
      { id: "ready_for_dev", collapsed: false },
      { id: "grill", collapsed: true },
      { id: "dev", collapsed: false },
      { id: "done", collapsed: true },
    ];
    expect(trackCells(columns, WISHLIST).map((c) => [c.kind, c.column])).toEqual([
      ["label", "backlog"],
      ["station", "ready_for_dev"],
      ["rail", "grill"],
      ["station", "dev"],
      ["rail", "done"],
    ]);
  });

  it("shows an epic at Review epic as its own stop on its track (FLW-19, Q273)", () => {
    const review: EpicStop = {
      ...stop("SHOP-12", "in_review", null),
      icon: "user-check",
      label: "Review epic",
      short_label: "Review epic",
      tooltip: "SHOP-12 Plugin API · Review epic",
    };
    expect(stopKey(review)).toBe("Review epic");
    const track: EpicTrack = {
      ...WISHLIST,
      key: "SHOP-12",
      name: "Plugin API",
      not_queued: [],
      stations: STATIONS.map((column) => ({
        column,
        stops: column === "review" ? [review] : [],
        merged: null,
        check: null,
      })),
    };
    const strip: EpicStrip = {
      title: "Epics in flight · 1",
      needs_you: null,
      hint: "",
      folded: "Epics in flight 1",
      tracks: [track],
    };
    const html = renderToStaticMarkup(
      createElement(Strip, {
        strip,
        columns: COLUMNS,
        folded: false,
        onToggleFold: () => undefined,
        pulse: null,
        focus: null,
        handlers: NOOP,
      }),
    );
    expect(html).toMatch(/data-column="review">.*data-state="in_review".*Review epic</);
  });

  it("follows Done to its rail, with its n merged", () => {
    const done = trackCells(RAIL, WISHLIST)[4];
    expect(done?.kind).toBe("rail");
    expect(done?.kind === "rail" && done.station.merged).toBe("1 merged");
    const markup = html(false, RAIL);
    expect(markup).toContain('class="strip-rail" data-column="done"');
    expect(markup).toContain("1 merged");
    expect(markup).not.toContain('aria-label="SHOP-40.1');
  });

  it("draws Done's station as a check while Done is its rail (SHL-06)", () => {
    const checked: EpicStrip = {
      ...STRIP,
      tracks: [
        {
          ...WISHLIST,
          stations: WISHLIST.stations.map((s) => (s.column === "done" ? { ...s, check: "circle-check" } : s)),
        },
      ],
    };
    const markup = html(false, RAIL, null, null, checked);
    expect(markup).toContain('class="strip-rail-check" data-merged="true"');
    expect(markup).not.toContain('class="strip-rail-merged"');
  });

  it("gives a column the track has no station for an empty one", () => {
    const cells = trackCells(COLUMNS, { stations: [] });
    expect(cells[1]).toEqual({
      kind: "station",
      column: "ready_for_dev",
      station: { column: "ready_for_dev", stops: [], merged: null, check: null },
    });
  });

  it("draws a cell per column, the stops in their station", () => {
    const markup = html(false);
    expect(markup.match(/class="strip-station"/g)).toHaveLength(4);
    expect(markup).toContain('class="strip-label" data-needs-you="true"');
    const dev = markup.slice(markup.indexOf('data-column="dev"'), markup.indexOf('data-column="review"'));
    expect(dev).toContain("asks you");
    expect(dev).toContain("40.4");
  });
});

describe("the narrow-stop rule", () => {
  const waits = stop("SHOP-40.3", "waits", "waits 40.2");
  const asks = stop("SHOP-40.2", "needs_you", "asks you", { needs_you: true });

  it("keeps the words at full width", () => {
    expect(fitStation([waits], 219)).toEqual({ short: false, shown: 1 });
    expect(showsNote(waits, false)).toBe(true);
  });

  it("drops the words below ~200 px per column, but a needs-you stop keeps asks you", () => {
    expect(fitStation([waits], NARROW_COLUMN - 7)).toEqual({ short: true, shown: 1 });
    expect(showsNote(waits, true)).toBe(false);
    expect(showsNote(asks, true)).toBe(true);
    expect(stopKey(waits)).toBe("40.3");
    expect(stopKey(stop("SHOP-40.1", "merged", null))).toBe("40.1");
  });

  it("a stop the lead added reads “new · by Wishlist lead”, and “new” when narrow (T-0291)", () => {
    const added = stop("SHOP-40.5", "waits", "waits 40.3", {
      new: "new · by Wishlist lead",
      new_short: "new",
    });
    expect(stopWidth(added, true)).toBeGreaterThan(stopWidth({ ...added, new_short: null }, true));
    expect(stopWidth(added, false)).toBeGreaterThan(stopWidth(added, true));
  });

  it("drops the words when the stops don't fit, then shows what fits and a +n", () => {
    const many = ["1", "2", "3"].map((n) => stop(`SHOP-40.${n}`, "waits", "waits 40.9"));
    expect(fitStation(many, 219)).toEqual({ short: true, shown: 3 });
    const crowd = ["1", "2", "3", "4", "5", "6"].map((n) => stop(`SHOP-40.${n}`, "working", null));
    const fit = fitStation(crowd, 219);
    expect(fit.short).toBe(true);
    expect(fit.shown).toBe(2);
    expect(fitStation(crowd.slice(0, 1), 10)).toEqual({ short: true, shown: 1 });
  });

  it("the new flag shows only at full width", () => {
    const markup = html(false);
    expect(markup).toContain("new · added by 40.2");
  });
});

describe("fold state", () => {
  it("folds and unfolds per project", () => {
    expect(stripFolded(stripFolds.get(), "SHOP")).toBe(false);
    toggleStripFold("SHOP");
    expect(stripFolded(stripFolds.get(), "SHOP")).toBe(true);
    expect(stripFolded(stripFolds.get(), "PAY")).toBe(false);
    toggleStripFold("SHOP");
    expect(stripFolded(stripFolds.get(), "SHOP")).toBe(false);
  });

  it("folded, the strip is its one line with a chip per epic", () => {
    const markup = html(true);
    expect(markup).toContain('aria-expanded="false"');
    expect(markup).toContain('aria-label="Epics in flight 1 · 1 needs you"');
    expect(markup).toContain('class="strip-mini" data-needs-you="true"');
    expect(markup).not.toContain("strip-track");
    expect(markup).not.toContain(STRIP.hint);
    const open = html(false);
    expect(open).toContain('aria-expanded="true"');
    expect(open).toContain(STRIP.hint);
  });
});

describe("hover, keys and looks", () => {
  it("previews the epic's cards, walks the name then the stops", () => {
    expect(trackCards(WISHLIST)).toEqual(["SHOP-40", "SHOP-40.5", "SHOP-40.2", "SHOP-40.4", "SHOP-40.1"]);
    expect(trackStops(WISHLIST)).toHaveLength(4);
    expect(stepAlong(0, 5, "ArrowRight")).toBe(1);
    expect(stepAlong(4, 5, "ArrowRight")).toBe(4);
    expect(stepAlong(0, 5, "ArrowLeft")).toBe(0);
  });

  it("pulses the stop of the hovered card", () => {
    expect(html(false, COLUMNS, "SHOP-40.4")).toMatch(/data-pulse="true"[^>]*aria-label="SHOP-40.4/);
    expect(html(false)).not.toContain("data-pulse");
  });

  it("a needs-you stop is turquoise; the bar has a segment per child", () => {
    expect(stopInk("working", true)).toBe("var(--st-attn)");
    expect(stopInk("in_review", false)).toBe("var(--st-review)");
    // T-0291: back at work after the lead's note, violet; its mini-bar segment is a working one.
    expect(stopInk("lead_note", false)).toBe("var(--st-investigate)");
    expect(segmentInk("lead_note")).toBe("var(--st-working)");
    expect(segmentInk("merged")).toBe("var(--st-approved)");
    expect(segmentInk("not_queued")).toBe("var(--border-strong)");
    expect(barWidth(4)).toBe(30);
    expect(
      html(false)
        .match(/class="strip-bar"[^>]*>(<span[^>]*><\/span>)+/)?.[0]
        .match(/<span/g),
    ).toHaveLength(5);
  });

  it("a not-queued chip carries its Queue button and can be dragged (T-0270)", () => {
    const markup = html(false);
    expect(markup).toContain('data-chip="SHOP-40.3"');
    expect(markup).toMatch(
      /<button[^>]*class="strip-chip-action"[^>]*aria-label="Queue SHOP-40.3"[^>]*>Queue<\/button>/,
    );
  });

  it("shows one not-queued chip and a +n for the rest", () => {
    const chip = WISHLIST.not_queued[0];
    if (!chip) throw new Error("fixture");
    const three = [chip, { ...chip, key: "SHOP-40.6" }, { ...chip, key: "SHOP-40.7" }];
    const { shown, more } = labelChips(three);
    expect(shown).toHaveLength(1);
    expect(more.map((c) => c.key)).toEqual(["SHOP-40.6", "SHOP-40.7"]);
  });
});

describe("the lead's line (T-0289)", () => {
  const lead: EpicTrack["lead"] = {
    epic: "SHOP-40",
    name: "Wishlist lead",
    state: "asks you",
    short: "asks you",
    full: "Wishlist lead · asks you",
    tone: "asks_you",
    icon: "bot",
    action: null,
    tooltip: { title: "Wishlist lead asks you", lines: [] },
    opens_tab: true,
  };

  it("a track with a lead grows and shows it in the short form; asks you turns the whole label turquoise", () => {
    const markup = html(false, COLUMNS, null, null, {
      ...STRIP,
      tracks: [{ ...WISHLIST, lead, lead_asks: true }],
    });
    expect(markup).toMatch(/data-track="SHOP-40" data-lead="true"/);
    expect(markup).toContain('data-lead-asks="true"');
    expect(markup).toContain('<div class="strip-label-lead">');
    expect(markup).toContain('<span class="lead-status-state">asks you</span>');
    expect(markup).not.toContain("lead-status-name");
  });

  it("a track without a lead keeps one line", () => {
    const markup = html(false);
    expect(markup).not.toContain("data-lead");
    expect(markup).not.toContain("strip-label-lead");
  });
});
