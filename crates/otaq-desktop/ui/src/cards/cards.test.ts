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
  sleeve: null,
  meta: [
    { icon: "activity", text: "12s", tone: "muted", tooltip: null },
    { icon: "play", text: ":4024", tone: "running", tooltip: null },
  ],
  outline: "attention",
  actions: [{ kind: "resume", label: "Resume", icon: "play", primary: true }],
  faded: null,
  fresh: false,
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
  branch: "otaq/SHOP-40 · on main",
  children: [
    {
      key: "SHOP-40.1",
      title: "Wishlist API",
      state: "merged",
      icon: "circle-check",
      status_label: "Merged",
      draggable: false,
      origin: null,
    },
  ],
  add: "Add ticket to epic",
  actions: [],
  faded: null,
  lead: null,
  lead_asks: false,
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

  it("an epic child wears the sleeve: name, the epic's pin, position; it dims as one", () => {
    const sleeve = { epic_key: "SHOP-40", name: "Wishlist", position: "2 of 4", pinned: false };
    const out = html(createElement(TicketCard, { card: { ...SHOP_22, sleeve } }));
    expect(out).toMatch(/^<div class="card-sleeve"/);
    expect(out).toContain('<span class="card-sleeve-name">Wishlist</span>');
    expect(out).toContain('<span class="card-sleeve-position">2 of 4</span>');
    expect(out).toContain("--card-outline:var(--st-attn)");
    expect(out).not.toContain("card-epic-chip");
    expect(out).not.toContain("card-sleeve-pin");
    const pinned = html(
      createElement(TicketCard, { card: { ...SHOP_22, sleeve: { ...sleeve, pinned: true } } }),
    );
    expect(pinned).toContain('class="card-sleeve-pin"');
    expect(pinned).not.toContain('class="card-pin"');
    const dimmed = html(
      createElement(TicketCard, { card: { ...SHOP_22, sleeve }, state: stateOfLook("dimmed") }),
    );
    expect(dimmed).toMatch(/^<div class="card-sleeve"[^>]*opacity:0.35/);
    expect(html(createElement(TicketCard, { card: SHOP_22 }))).not.toContain("card-sleeve");
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

  it("a draggable row has a grip; a merged one has none (T-0270)", () => {
    const merged = WISHLIST.children[0];
    if (!merged) throw new Error("fixture");
    const card = {
      ...WISHLIST,
      children: [merged, { ...merged, key: "SHOP-40.2", state: "not_started" as const, draggable: true }],
    };
    const out = html(createElement(EpicCard, { card }));
    expect(out.match(/data-draggable="true"/g)).toHaveLength(1);
    expect(out.match(/epic-child-grip/g)).toHaveLength(1);
  });
});

describe("the Epic Card's lead (T-0289)", () => {
  const LEAD: EpicCardView["lead"] = {
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

  it("the Lead row, the rows added by the lead and the turquoise border", () => {
    const child = WISHLIST.children[0];
    if (!child) throw new Error("a child");
    const out = html(
      createElement(EpicCard, {
        card: {
          ...WISHLIST,
          lead: LEAD,
          lead_asks: true,
          children: [{ ...child, origin: "added by Wishlist lead" }],
        },
      }),
    );
    expect(out).toContain('data-lead-asks="true"');
    expect(out).toContain('<div class="epic-lead">');
    expect(out).toContain("Wishlist lead");
    expect(out).toContain("<span>added by Wishlist lead</span>");
  });

  it("no Lead row without one, no origin on rows the user added", () => {
    const out = html(createElement(EpicCard, { card: WISHLIST }));
    expect(out).not.toContain("epic-lead");
    expect(out).not.toContain("epic-child-origin");
  });
});
