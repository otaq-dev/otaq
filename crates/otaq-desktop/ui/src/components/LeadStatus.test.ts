import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import type { LeadStatusView } from "../generated/LeadStatusView";
import { LeadStatus, leadAction } from "./LeadStatus";

const html = (lead: LeadStatusView, form: "full" | "short", plain = false) =>
  renderToStaticMarkup(createElement(LeadStatus, { lead, form, plain }));

/** Hand-written as Rust words them (`lead_status.rs`). */
const lead = (over: Partial<LeadStatusView>): LeadStatusView => ({
  epic: "SHOP-40",
  name: "Wishlist lead",
  state: "idle",
  short: "idle",
  full: "Wishlist lead · idle",
  tone: "idle",
  icon: "bot",
  action: null,
  tooltip: { title: "Wishlist lead · idle", lines: ["212k today"] },
  opens_tab: true,
  ...over,
});

const REPLANNING = lead({
  state: "re-planning",
  short: "re-planning",
  full: "Wishlist lead · re-planning",
  tone: "working",
});
const ANSWERING = lead({ state: "answering SHOP-40.2", short: "answering 40.2", tone: "working" });
const ASKS = lead({ state: "asks you", short: "asks you", tone: "asks_you" });
const ENDED = lead({
  state: "ended by restart",
  short: "ended by restart",
  tone: "ended",
  action: { kind: "resume", label: "Resume", icon: "rotate-ccw" },
});
const NONE = lead({
  name: null,
  state: "",
  short: "",
  full: "Open lead",
  tone: "none",
  action: { kind: "open", label: "Open lead", icon: "bot" },
  opens_tab: false,
});

describe("the Lead Status draws each form (T-0289)", () => {
  it("full: the icon, the name and · the state, in its tone", () => {
    const out = html(REPLANNING, "full");
    expect(out).toContain('data-tone="working"');
    expect(out).toContain('data-form="full"');
    expect(out).toContain('<span class="lead-status-name">Wishlist lead</span>');
    expect(out).toContain('<span class="lead-status-state">· re-planning</span>');
    expect(out).toContain('aria-label="Wishlist lead · re-planning"');
    expect(out).not.toContain("lead-status-action");
  });

  it("short: no name, the short words", () => {
    const out = html(ANSWERING, "short");
    expect(out).not.toContain("lead-status-name");
    expect(out).toContain('<span class="lead-status-state">answering 40.2</span>');
  });

  it("asks you is turquoise, and plain inside a turquoise label", () => {
    expect(html(ASKS, "full")).toContain('data-tone="asks_you"');
    expect(html(ASKS, "short", true)).toContain('data-plain="true"');
    expect(html(ASKS, "short")).not.toContain("data-plain");
  });

  it("ended by restart offers Resume", () => {
    const out = html(ENDED, "short");
    expect(out).toContain('data-tone="ended"');
    expect(out).toContain("<span>Resume</span>");
  });

  it("no lead is the Open lead button alone", () => {
    const out = html(NONE, "full");
    expect(out).toContain('data-tone="none"');
    expect(out).toContain("<span>Open lead</span>");
    expect(out).not.toContain("lead-status-main");
  });
});

describe("Open lead and Resume", () => {
  it("ask the board, and a refusal is a toast in the service's words", async () => {
    const ask = vi.fn<(method: string, params?: unknown) => Promise<unknown>>((method) =>
      method === "board.lead_resume"
        ? Promise.reject(new Error("Usage limit reached · the lead opens once it resets"))
        : Promise.resolve(null),
    );
    await leadAction("resume", "SHOP-40", ask);
    expect(ask).toHaveBeenNthCalledWith(1, "board.lead_resume", { key: "SHOP-40" });
    expect(ask.mock.calls[1]?.[0]).toBe("notices.toast");
    expect(ask.mock.calls[1]?.[1]).toMatchObject({
      level: "error",
      title: "Usage limit reached · the lead opens once it resets",
    });
  });
});
