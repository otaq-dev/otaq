import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { DecisionItemView } from "../generated/DecisionItemView";
import type { DecisionsSection } from "../generated/DecisionsSection";
import { DecisionItem, DecisionsList } from ".";

// Hand-written views, as `decision_item.rs` words them (*07 → Row · Decision Item (0084)*).
const LABELS = { built: "Built", other: "Other option", why: "Why", change: "Change…" };

const FULL_NAME: DecisionItemView = {
  id: "SHOP-23#1",
  question: "Split fullName everywhere, or only in the profile form?",
  built: "only in the profile form",
  other: ["everywhere"],
  why: "the description only mentions the profile form.",
  mark: null,
  status: null,
  change: true,
  labels: LABELS,
};

const ORDER: DecisionItemView = {
  ...FULL_NAME,
  id: "SHOP-40.1#1",
  question: "Order of saved items?",
  built: "newest first",
  other: ["in the order they were added"],
  why: "matches the order history page.",
  mark: {
    text: "Wishlist lead: contradicts PAY-123: “items are listed in the order they were added”.",
    source: "PAY-123",
  },
};

const html = (item: DecisionItemView) => renderToStaticMarkup(createElement(DecisionItem, { item }));

describe("the Decision Item draws each variant from its view", () => {
  it("plain, in review: signpost, the question, Built / Other option / Why, and Change…", () => {
    const out = html(FULL_NAME);
    expect(out).toContain("signpost");
    for (const text of [
      FULL_NAME.question,
      "Built",
      "only in the profile form",
      "Other option",
      "everywhere",
      "Why",
    ]) {
      expect(out).toContain(text);
    }
    expect(out).toContain("Change…");
    expect(out).not.toContain("decision-mark");
    expect(out).not.toContain("decision-status");
  });

  it("with the lead's mark: a violet row with its source chip, never an error colour", () => {
    const out = html(ORDER);
    expect(out).toContain('class="decision-mark"');
    expect(out).toContain("Wishlist lead: contradicts PAY-123");
    expect(out).toContain('class="decision-source"');
    expect(out).toContain("PAY-123");
    expect(out).not.toContain("error");
    const noSource = html({
      ...ORDER,
      mark: { text: "Wishlist lead: the page sorts the other way", source: null },
    });
    expect(noSource).not.toContain("decision-source");
  });

  it("read-only while the ticket works: no Change…", () => {
    expect(html({ ...FULL_NAME, change: false })).not.toContain("Change…");
  });

  it("after Approve and after Change…: the status line with its icon", () => {
    const kept = html({
      ...FULL_NAME,
      change: false,
      status: { text: "Kept by you · Approve, 16:40", icon: "check", kept: true },
    });
    expect(kept).toContain("Kept by you · Approve, 16:40");
    expect(kept).toContain("data-kept");
    const sent = html({
      ...FULL_NAME,
      change: false,
      status: {
        text: "Changed by you → sent back · “Split fullName everywhere”",
        icon: "corner-up-left",
        kept: false,
      },
    });
    expect(sent).toContain("Changed by you → sent back");
    expect(sent).toContain("corner-up-left");
    expect(sent).not.toContain("data-kept");
    const fixed = html({
      ...ORDER,
      change: false,
      status: { text: "Changed by you → SHOP-40.7", icon: "plus", kept: false },
    });
    expect(fixed).toContain("Changed by you → SHOP-40.7");
    expect(fixed).toContain("decision-mark");
  });
});

describe("the Decisions list", () => {
  it("an epic's list: the count, the hint and one head per child", () => {
    const section: DecisionsSection = {
      title: "Decisions",
      count: 2,
      hint: "Approve keeps them all · Change… adds a fix child",
      groups: [
        { child: { key: "SHOP-40.1", title: "Wishlist API", state: "merged" }, items: [ORDER] },
        {
          child: { key: "SHOP-40.4", title: "Wishlist share link", state: null },
          items: [{ ...FULL_NAME, id: "SHOP-40.4#1" }],
        },
      ],
    };
    const out = renderToStaticMarkup(createElement(DecisionsList, { section }));
    expect(out).toContain('class="decisions-count">2<');
    expect(out).toContain("Approve keeps them all · Change… adds a fix child");
    expect(out).toContain("SHOP-40.1");
    expect(out).toContain("Wishlist API");
    expect(out).toContain('class="decisions-child-state">merged<');
    expect(out.indexOf("SHOP-40.1")).toBeLessThan(out.indexOf("SHOP-40.4"));
    expect(out.match(/decisions-child-state/g)).toHaveLength(1);
  });

  it("no hint after approval", () => {
    const out = renderToStaticMarkup(
      createElement(DecisionsList, {
        section: { title: "Decisions", count: 1, hint: null, groups: [{ child: null, items: [FULL_NAME] }] },
      }),
    );
    expect(out).not.toContain("decisions-hint");
    expect(out).not.toContain("decisions-child");
  });
});
