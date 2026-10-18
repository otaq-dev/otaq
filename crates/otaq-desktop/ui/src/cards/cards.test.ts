import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { CardHover } from "../generated/CardHover";
import type { EpicCardView } from "../generated/EpicCardView";
import type { TicketCardView } from "../generated/TicketCardView";
import { EpicCard, TicketCard } from ".";
import { HoverCard, StatusHoverCard } from "./TicketCard";
import {
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

// The shape of *Needs you + running + 2 dependencies* as `card_page.rs` serves it (Ticket Card v2).
const SHOP_22: TicketCardView = {
  key: "SHOP-22",
  key_label: "SHOP-22",
  avatar: null,
  pinned: false,
  priority: null,
  flag: { palette: "attn", icon: "hand", label: "Needs you" },
  title: "Fix flaky login redirect",
  situation: {
    text: "Allow rm -rf .next? · also running :4022 · after SHOP-18, SHOP-19",
    edge: "attention",
    text_tone: "secondary",
    mono: false,
    hovers: [],
  },
  origin: null,
  options: [],
  footer: {
    agent: null,
    tokens: null,
    signal: null,
    action: { kind: "open", label: "Answer", icon: null, primary: false },
  },
  hover: { status: "Needs you · permission", details: ["Tokens: 64k"] },
  sleeve: null,
  outline: "attention",
  faded: null,
  fresh: false,
  open_panel: "Open panel",
  open_panel_keys: "↵",
  open_terminal: "Open terminal",
};

const WISHLIST: EpicCardView = {
  key: "SHOP-40",
  pinned: false,
  external_id: "PAY-123",
  priority: null,
  pill: { palette: "epic", icon: "layers", label: "Epic" },
  title: "Wishlist",
  segments: ["merged", "working", "not_started"],
  progress: "1 of 3 merged",
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
  it("tones, outlines, epic children and gallery looks", () => {
    for (const tone of TONES) {
      const { fg, bg } = toneInk(tone);
      expect(fg, tone).toMatch(/^var\(--/);
      expect(bg, tone).toMatch(/^var\(--/);
    }
    expect(new Set(TONES.map((t) => toneInk(t).fg)).size).toBe(TONES.length);
    for (const outline of OUTLINES) expect(outlineColor(outline)).toMatch(/^var\(--/);
    for (const state of CHILD_STATES) {
      expect(childInk(state)).toMatch(/^var\(--/);
      expect(segmentFill(state)).toMatch(/^var\(--/);
    }
    const states = LOOKS.map((look) => JSON.stringify(stateOfLook(look)));
    expect(new Set(states).size).toBe(LOOKS.length);
  });
});

describe("the Ticket Card v2 draws its view", () => {
  it("head flag, two-line title, situation line, the footer's one action", () => {
    const out = html(createElement(TicketCard, { card: SHOP_22 }));
    expect(out).toContain('<span class="card-key">SHOP-22</span>');
    expect(out).toContain('data-palette="attn"');
    expect(out).toContain("<span>Needs you</span>");
    expect(out).toContain('class="card-title">Fix flaky login redirect</div>');
    expect(out).toContain("Allow rm -rf .next? · also running :4022 · after SHOP-18, SHOP-19");
    expect(out).not.toContain("data-mono");
    expect(out).toContain('class="card-footer"');
    expect(out).toContain("<span>Answer</span>");
    expect(out).toContain("--card-outline:var(--st-attn)");
    expect(out).not.toContain("card-quick");
    expect(out).not.toContain("tool-avatar");
  });

  it("each part only when the view has it: no flag (no empty chip), no line, no origin, no footer", () => {
    const out = html(
      createElement(TicketCard, { card: { ...SHOP_22, flag: null, situation: null, footer: null } }),
    );
    expect(out).not.toContain("status-pill");
    expect(out).not.toContain("card-situation");
    expect(out).not.toContain("card-origin");
    expect(out).not.toContain("card-footer");
    expect(out).not.toContain("card-pin");
    expect(out).not.toContain("priority-chip");
    expect(out).not.toContain("card-options");
  });

  it("the head's optional parts: tool avatar (when shown), pin, priority", () => {
    const out = html(
      createElement(TicketCard, {
        card: {
          ...SHOP_22,
          avatar: {
            tool: "claude_code",
            letters: "CC",
            background: "tool-cc",
            foreground: "tool-cc-fg",
            hover: "Claude Code · Opus",
          },
          pinned: true,
          priority: {
            number: 1,
            short: "P1",
            word: "Urgent",
            label: "P1 · Urgent",
            icon: "chevrons-up",
            ink: "inverted",
          },
        },
      }),
    );
    const head = out.slice(out.indexOf('class="card-head"'), out.indexOf('class="card-title"'));
    expect(head).toMatch(/tool-avatar.*CC.*card-key.*card-pin.*priority-chip.*status-pill/s);
  });

  it("the hover shows the full status and what the card has no room for, from the head", () => {
    const out = html(createElement(TicketCard, { card: SHOP_22 }));
    expect(out).toMatch(
      /^<article[^>]*><span class="card-hover-anchor" role="presentation"><div class="card-head">/,
    );
    const hover = html(createElement(StatusHoverCard, { hover: SHOP_22.hover }));
    expect(hover).toContain("<strong>Needs you · permission</strong>");
    expect(hover).toContain('data-muted="true">Tokens: 64k</span>');
  });

  it("the situation line: mono only for a command; the origin line in violet", () => {
    if (!SHOP_22.situation) throw new Error("fixture");
    const situation = { ...SHOP_22.situation, text: "Allow rm -rf .next?", mono: true };
    const out = html(
      createElement(TicketCard, {
        card: { ...SHOP_22, situation, origin: "Created by Checkout slowness" },
      }),
    );
    expect(out).toContain('data-mono="true"');
    expect(out).toContain("--edge:var(--st-attn)");
    expect(out).toContain('<div class="card-origin">Created by Checkout slowness</div>');
  });

  it("the footer's action is a label only, filled when the view says primary", () => {
    const footer = {
      agent: null,
      tokens: null,
      signal: null,
      action: { kind: "review" as const, label: "Review", icon: "eye" as const, primary: true },
    };
    const out = html(createElement(TicketCard, { card: { ...SHOP_22, footer } }));
    expect(out).toContain('data-variant="primary"');
    expect(out.slice(out.indexOf('class="card-footer"'))).not.toContain('class="icon"');
    expect(html(createElement(TicketCard, { card: SHOP_22 }))).toContain('data-variant="secondary"');
  });

  it("the footer's agent part: neutral when set by you, violet when an agent picked it (M22)", () => {
    const footer = {
      agent: { text: "Fable test · set by you", picked: false, tooltip: null },
      tokens: null,
      signal: null,
      action: null,
    };
    const out = html(createElement(TicketCard, { card: { ...SHOP_22, footer } }));
    expect(out).toContain('<span class="card-footer-agent">Fable test · set by you</span>');
    expect(out.slice(out.indexOf('class="card-footer"'))).not.toContain("card-spacer");
    const picked = {
      ...footer,
      agent: { text: "Light · picked by Checkout slowness", picked: true, tooltip: "docs-only change" },
    };
    expect(html(createElement(TicketCard, { card: { ...SHOP_22, footer: picked } }))).toContain(
      '<span class="card-footer-agent" data-picked="true">Light · picked by Checkout slowness</span>',
    );
  });

  it("a running agent's footer: tokens and the last signal, amber when old", () => {
    const footer = {
      agent: null,
      tokens: { text: "182k", tooltip: null },
      signal: { text: "4m", stale: true },
      action: null,
    };
    const out = html(createElement(TicketCard, { card: { ...SHOP_22, footer } }));
    expect(out).toContain("182k");
    expect(out).toContain('data-stale="true">4m</span>');
  });

  it("states: selected, chain-dimmed, hovered with the quick actions", () => {
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("selected") }))).toContain(
      'data-selected="true"',
    );
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("dimmed") }))).toContain(
      'data-chain="dimmed"',
    );
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("dragging") }))).toContain(
      'data-dragging="true"',
    );
    expect(html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("chain") }))).toContain(
      'data-chain="highlighted"',
    );
    const hovered = html(createElement(TicketCard, { card: SHOP_22, state: stateOfLook("hovered") }));
    expect(hovered).toContain('class="card-quick"');
    expect(hovered).toContain('aria-label="Open panel"');
  });

  it("a Done card fades", () => {
    expect(html(createElement(TicketCard, { card: { ...SHOP_22, faded: 0.55 } }))).toContain(
      "--card-faded:0.55",
    );
  });

  it("an epic child wears the sleeve: name, the epic's pin, position; it dims as one", () => {
    const sleeve = { epic_key: "SHOP-40", name: "Wishlist", position: "2 of 4", pinned: false };
    const out = html(createElement(TicketCard, { card: { ...SHOP_22, sleeve } }));
    expect(out).toMatch(/^<div class="card-sleeve"/);
    expect(out).toContain('<span class="card-sleeve-name">Wishlist</span>');
    expect(out).toContain('<span class="card-sleeve-position">2 of 4</span>');
    expect(out).toContain("--card-outline:var(--st-attn)");
    expect(out).not.toContain("card-sleeve-pin");
    const pinned = html(
      createElement(TicketCard, { card: { ...SHOP_22, sleeve: { ...sleeve, pinned: true } } }),
    );
    expect(pinned).toContain('class="card-sleeve-pin"');
    expect(pinned).not.toContain('class="card-pin"');
    const dimmed = html(
      createElement(TicketCard, { card: { ...SHOP_22, sleeve }, state: stateOfLook("dimmed") }),
    );
    expect(dimmed).toMatch(/^<div class="card-sleeve" data-chain="dimmed"/);
    expect(html(createElement(TicketCard, { card: SHOP_22 }))).not.toContain("card-sleeve");
  });
});

describe("the Epic Card draws its view", () => {
  it("External ID, progress segments, branch, children and Add ticket to epic", () => {
    const out = html(createElement(EpicCard, { card: WISHLIST }));
    expect(out).toContain('<span class="card-external-id">PAY-123</span>');
    expect(out.match(/class="epic-segment"/g)).toHaveLength(3);
    expect(out).toContain("1 of 3 merged");
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

describe("the children's cards with a lead (T-0291)", () => {
  const SAME_FILE: CardHover = {
    icon: "files",
    tone: "muted",
    title: "same file as SHOP-40.4",
    sections: [
      { label: null, text: "lib/wishlist.ts", muted: false },
      {
        label: null,
        text: "Both changed it so far. Doesn't wake the lead; neither child is paused.",
        muted: true,
      },
    ],
  };

  it("the unread note and the same file: one situation line with its hovers", () => {
    const out = html(
      createElement(TicketCard, {
        card: {
          ...SHOP_22,
          situation: {
            text: "1 unread note from Wishlist lead · same file as SHOP-40.4",
            edge: "muted",
            text_tone: "secondary",
            mono: false,
            hovers: [SAME_FILE],
          },
        },
      }),
    );
    expect(out).toContain("1 unread note from Wishlist lead · same file as SHOP-40.4");
    // The head's status hover and the situation line's own.
    expect(out.match(/class="card-hover-anchor"/g)).toHaveLength(2);
    expect(out).toMatch(/<span class="card-hover-anchor" role="presentation"><div class="card-situation"/);
    expect(out).not.toContain('class="card-hover"');
  });

  it("new · added by Wishlist lead: the violet outline; an agent-made card: the origin line", () => {
    const out = html(
      createElement(TicketCard, {
        card: { ...SHOP_22, outline: "epic", origin: "Created by Checkout slowness" },
      }),
    );
    expect(out).toContain("--card-outline:var(--st-investigate)");
    expect(out).toContain('<div class="card-origin">Created by Checkout slowness</div>');
  });

  it("the hover card: its title, eyebrows and the quiet footnote", () => {
    const out = html(
      createElement(HoverCard, {
        hover: {
          icon: "bot",
          tone: "epic",
          title: "Wishlist lead",
          sections: [
            { label: "LEAD'S NOTE · SENT AFTER ROUND 3", text: "“Fix the test.”", muted: false },
            { label: "LEAD'S VIEW", text: "The agent keeps changing the API.", muted: true },
          ],
        },
      }),
    );
    expect(out).toContain("<strong>Wishlist lead</strong>");
    expect(out).toContain('<span class="card-hover-label">LEAD&#x27;S NOTE · SENT AFTER ROUND 3</span>');
    expect(out).toContain('data-muted="true">The agent keeps changing the API.</span>');
    expect(html(createElement(HoverCard, { hover: SAME_FILE }))).toContain("lib/wishlist.ts");
  });
});
