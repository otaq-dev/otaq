import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { BoardCard } from "../../generated/BoardCard";
import type { BoardColumn } from "../../generated/BoardColumn";
import type { ChainView } from "../../generated/ChainView";
import type { TicketCardView } from "../../generated/TicketCardView";
import { activeChain, arrow, chainStateOf, inView, membersIn } from "./chain";
import { Column, type ColumnProps } from "./Column";

/** *03 → Chain highlight · hover or select SHOP-20*. */
const SHOP_20: ChainView = {
  key: "SHOP-20",
  members: ["SHOP-17", "SHOP-18", "SHOP-19", "SHOP-20", "SHOP-21"],
  edges: [
    { parent: "SHOP-17", child: "SHOP-18" },
    { parent: "SHOP-18", child: "SHOP-20" },
    { parent: "SHOP-19", child: "SHOP-20" },
    { parent: "SHOP-20", child: "SHOP-21" },
  ],
  legend: "Chain of SHOP-20 · SHOP-17 → SHOP-18 → SHOP-20 ← SHOP-19 · SHOP-20 → SHOP-21",
};
const SHOP_21: ChainView = { ...SHOP_20, key: "SHOP-21" };

describe("the chain highlight", () => {
  it("follows the hovered card, else the selected one, and never a drag", () => {
    const chains = [SHOP_20, SHOP_21];
    expect(activeChain(chains, "SHOP-21", "SHOP-20", false)).toBe(SHOP_21);
    expect(activeChain(chains, null, "SHOP-20", false)).toBe(SHOP_20);
    expect(activeChain(chains, "SHOP-22", "SHOP-20", false), "an unlinked card lights nothing").toBeNull();
    expect(activeChain(chains, "SHOP-21", null, true)).toBeNull();
    expect(activeChain(chains, null, null, false)).toBeNull();
  });

  it("gives each card its look", () => {
    expect(chainStateOf(SHOP_20, "SHOP-20")).toBe("origin");
    expect(chainStateOf(SHOP_20, "SHOP-17")).toBe("highlighted");
    expect(chainStateOf(SHOP_20, "SHOP-22")).toBe("dimmed");
    expect(chainStateOf(null, "SHOP-22")).toBe("none");
    expect(membersIn(SHOP_20, ["SHOP-16", "SHOP-17"])).toEqual(["SHOP-17"]);
    expect(membersIn(null, ["SHOP-17"])).toEqual([]);
  });

  it("draws arrows only between cards in view", () => {
    const column = { left: 0, top: 40, right: 256, bottom: 800 };
    expect(inView({ left: 6, top: 50, right: 250, bottom: 150 }, [column])).toBe(true);
    expect(inView({ left: 6, top: -90, right: 250, bottom: 30 }, [column]), "scrolled out above").toBe(false);
  });

  it("joins cards side to side, and around the left edge in one column", () => {
    const review = { left: 809, top: 44, right: 1053, bottom: 179 };
    const ready = { left: 280, top: 329, right: 524, bottom: 458 };
    // SHOP-18 (Review) → SHOP-20 (Ready for Dev): from SHOP-18's left side to SHOP-20's right side.
    const back = arrow(review, ready);
    expect(back.line.startsWith("M809 111.5 C")).toBe(true);
    expect(back.head.startsWith("M524 393.5 L")).toBe(true);
    const forward = arrow(ready, review);
    expect(forward.line.startsWith("M524 393.5 C")).toBe(true);
    expect(forward.head.startsWith("M809 111.5 L")).toBe(true);
    const below = { left: 280, top: 464, right: 524, bottom: 577 };
    const same = arrow(ready, below);
    expect(same.line).toMatch(/^M280 393.5 C240 393.5 240 520.5 /);
    expect(same.head.startsWith("M280 520.5 L")).toBe(true);
  });
});

const card = (key: string): BoardCard => {
  const view: TicketCardView = {
    key,
    key_label: key,
    pinned: false,
    priority: null,
    title: "Checkout page",
    pill: null,
    pill_short: null,
    callout: null,
    entry: null,
    options: [],
    bars: [],
    sleeve: null,
    meta: [],
    outline: "normal",
    actions: [],
    faded: null,
    fresh: false,
    open_panel: "Open panel",
    open_panel_keys: "↵",
    compact: { flag: null, value: null },
  };
  return { kind: "ticket", card: view, needs_you: false };
};

const column = (over: Partial<BoardColumn>): BoardColumn => ({
  id: "ready_for_dev",
  title: "Ready for Dev",
  icon: "play",
  count: "2",
  hint: null,
  note: null,
  empty_hint: "",
  collapsed: false,
  toggle: null,
  hidden: null,
  cards: [card("SHOP-20"), card("SHOP-28")],
  folds: [],
  dev: null,
  add: null,
  ...over,
});

const props = (c: BoardColumn, chain: ChainView | null): ColumnProps => ({
  column: c,
  chipWords: { above: "{n} above", need_you: "{n} need you" },
  dragging: false,
  selected: null,
  chain,
  lifted: null,
  handlers: {
    press: () => undefined,
    pressChild: () => undefined,
    key: () => undefined,
    open: () => undefined,
    action: () => undefined,
    addToEpic: () => undefined,
    menu: () => undefined,
    hover: () => undefined,
  },
  scrollTop: 0,
  onScroll: () => undefined,
  onToggleDone: () => undefined,
  onUnfold: () => undefined,
  onAdd: () => undefined,
  element: () => undefined,
});

describe("a column under the chain highlight", () => {
  it("outlines members and dims the rest", () => {
    const html = renderToStaticMarkup(createElement(Column, props(column({}), SHOP_20)));
    expect(html).toContain('data-chain="origin"');
    expect(html).toContain('data-chain="dimmed"');
    const plain = renderToStaticMarkup(createElement(Column, props(column({}), null)));
    expect(plain).not.toContain("data-chain");
  });

  it("names members folded in the Done rail and behind Dev's fold", () => {
    const done = column({
      id: "done",
      title: "Done",
      collapsed: true,
      toggle: "Expand Done",
      cards: [card("SHOP-17"), card("SHOP-9")],
    });
    const rail = renderToStaticMarkup(createElement(Column, props(done, SHOP_20)));
    expect(rail).toContain('<span class="board-rail-chain">SHOP-17</span>');
    expect(rail).not.toContain(">SHOP-9<");
    const dev = column({
      id: "dev",
      title: "Dev",
      cards: [],
      dev: {
        top: [],
        section: "WORKING · VALIDATING",
        working_count: "6",
        working: [card("SHOP-33")],
        fold: "2 more · working",
        folded: ["SHOP-18", "SHOP-34"],
      },
    });
    const folded = renderToStaticMarkup(createElement(Column, props(dev, SHOP_20)));
    expect(folded).toMatch(/class="board-chip" data-chain="true"[^>]*>.*2 more · working/);
  });
});
