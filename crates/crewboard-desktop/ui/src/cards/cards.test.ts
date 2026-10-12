import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { EpicCardView } from "../generated/EpicCardView";
import type { TicketCardView } from "../generated/TicketCardView";
import { EpicCard, TicketCard } from ".";
import {
  BAR_LOOKS,
  barInk,
  CHILD_STATES,
  childInk,
  LOOKS,
  OUTLINES,
  outlineColor,
  segmentFill,
  stateOfLook,
  toneInk,
  TONES,
} from "./look";

const html = (element: ReactElement) => renderToStaticMarkup(element);

// The shape of *Working · needs you* as `card_page.rs` serves it.
const SHOP_22: TicketCardView = {
  key: "SHOP-22",
  key_label: "SHOP-22",
  pinned: false,
  priority: null,
  title: "Fix flaky login redirect",
  pill: { palette: "attn", icon: "hand", label: "Needs you" },
  callout: { icon: "hand", tone: "attention", text: "Allow rm -rf .next?", mono: true },
  entry: null,
  options: [],
  bars: [{ look: "messages", icon: "message-square-dot", text: "1 message waiting", action: null }],
  epic: "Wishlist",
  meta: [
    { icon: "activity", text: "12s", tone: "muted", tooltip: null },
    { icon: "play", text: ":4024", tone: "running", tooltip: null },
  ],
  outline: "attention",
  actions: [{ kind: "resume", label: "Resume", icon: "play", primary: true }],
  faded: null,
  open_panel: "Open panel",
  open_panel_keys: "↵",
  compact: {
    flag: { palette: "attn", icon: "hand", label: "Needs you" },
    value: { icon: null, text: "12s", tone: "muted" },
  },
};

const WISHLIST: EpicCardView = {
  key: "SHOP-40",
  pinned: false,
  external_id: "PAY-123",
  priority: null,
  pill: { palette: "epic", icon: "layers", label: "Epic" },
  title: "Wishlist",
  segments: ["merged", "working", "not_started"],
  progress: "1 / 3 merged",
  branch: "crewboard/SHOP-40 · on main",
  children: [
    {
      key: "SHOP-40.1",
      title: "Wishlist API",
      state: "merged",
      icon: "circle-check",
      status_label: "Merged",
    },
  ],
  add: "Add ticket to epic",
  actions: [],
  faded: null,
};

describe("every union the card draws has a drawing", () => {
  it("tones, outlines, bars, epic children and gallery looks", () => {
    for (const tone of TONES) {
      const { fg, bg } = toneInk(tone);
      expect(fg, tone).toMatch(/^var\(--/);
      expect(bg, tone).toMatch(/^var\(--/);
    }
    expect(new Set(TONES.map((t) => toneInk(t).fg)).size).toBe(TONES.length);
    for (const outline of OUTLINES) expect(outlineColor(outline)).toMatch(/^var\(--/);
    for (const look of BAR_LOOKS) expect(barInk(look).bg).toMatch(/^var\(--/);
    for (const state of CHILD_STATES) {
      expect(childInk(state)).toMatch(/^var\(--/);
      expect(segmentFill(state)).toMatch(/^var\(--/);
    }
    const states = LOOKS.map((look) => JSON.stringify(stateOfLook(look)));
    expect(new Set(states).size).toBe(LOOKS.length);
  });
});

describe("the Ticket Card draws its view", () => {
  it("comfortable: key, pill with icon and label, two-line title, callout, bar, meta and actions", () => {
    const out = html(createElement(TicketCard, { card: SHOP_22 }));
    expect(out).toContain('<span class="card-key">SHOP-22</span>');
    expect(out).toContain('data-palette="attn"');
    expect(out).toContain("<span>Needs you</span>");
    expect(out).toContain('class="card-title">Fix flaky login redirect</div>');
    expect(out).toContain("Allow rm -rf .next?");
    expect(out).toContain('data-mono="true"');
    expect(out).toContain("1 message waiting");
    expect(out).toContain('class="card-run-chip"');
    expect(out).toContain("<span>Resume</span>");
    expect(out).toContain("--card-outline:var(--st-attn)");
    expect(out).not.toContain("card-quick");
  });

  it("states: selected, chain-dimmed, hovered with the quick actions", () => {
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("selected") }))).toContain(
      'data-selected="true"',
    );
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("dimmed") }))).toContain(
      "opacity:0.35",
    );
    const hovered = html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("hovered") }));
    expect(hovered).toContain('class="card-quick"');
    expect(hovered).toContain('aria-label="Open panel"');
  });

  it("compact: the flag is the pill's icon alone, one value, a one-line title, no callout", () => {
    const out = html(createElement(TicketCard, { card: SHOP_22, density: "compact" }));
    expect(out).toContain('data-density="compact"');
    expect(out).toContain('class="status-pill card-flag" data-palette="attn"');
    expect(out).toContain('aria-label="Needs you"');
    expect(out).not.toContain("<span>Needs you</span>");
    expect(out).toContain(">12s</span>");
    expect(out).not.toContain("Allow rm -rf");
    expect(out).not.toContain("card-meta");
  });

  it("a Done card fades", () => {
    expect(html(createElement(TicketCard, { card: { ...SHOP_22, faded: 0.55 } }))).toContain("opacity:0.55");
  });
});

describe("the Epic Card draws its view", () => {
  it("External ID, progress segments, branch, children and Add ticket to epic", () => {
    const out = html(createElement(EpicCard, { card: WISHLIST }));
    expect(out).toContain('<span class="card-external-id">PAY-123</span>');
    expect(out.match(/class="epic-segment"/g)).toHaveLength(3);
    expect(out).toContain("1 / 3 merged");
    expect(out).toContain("SHOP-40.1");
    expect(out).toContain("<span>Add ticket to epic</span>");
  });

  it("folded: no child list and no add row", () => {
    const out = html(createElement(EpicCard, { card: WISHLIST, collapsed: true }));
    expect(out).not.toContain("epic-child");
    expect(out).not.toContain("Add ticket to epic");
    expect(out).toContain('aria-expanded="false"');
  });
});
