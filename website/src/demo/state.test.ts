import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DemoStore, TOAST_MS } from "./state";
import { boardView } from "./view";

let store: DemoStore;

beforeEach(() => {
  vi.useFakeTimers();
  store = new DemoStore();
});

afterEach(() => {
  store.dispose();
  vi.useRealTimers();
});

const ticket = (k: string) => {
  const t = store.find(k);
  if (!t) throw new Error("no ticket " + k);
  return t;
};
const texts = (k: string) => ticket(k).log.map((l) => l.t);
const needs = () => boardView(store.getState()).needs;
const agent = (id: string) => {
  const a = store.getState().agents.find((x) => x.id === id);
  if (!a) throw new Error("no agent " + id);
  return a;
};

describe("the guided steps (The example)", () => {
  it("answering the Project agent “Only open issues” puts NOTE-14 and NOTE-15 in Backlog", () => {
    expect(needs()).toBe(5);
    store.answer("imp-open");
    expect(ticket("NOTE-14")).toMatchObject({
      col: "backlog",
      st: "queued",
      title: "Show a hint when the notes file is missing",
      agent: "Pip",
      fresh: true,
    });
    expect(ticket("NOTE-15")).toMatchObject({
      col: "backlog",
      title: "Read notes from stdin",
      agent: "Theo",
      waits: ["NOTE-8"],
    });
    expect(agent("a1")).toMatchObject({ st: "idle", ask: null, created: ["NOTE-14", "NOTE-15"] });
    expect(agent("a1").lines.map((l) => l.t)).toContain("❯ Only open issues");
    expect(store.getState().toast).toBe("The Project agent created NOTE-14 and NOTE-15 in Backlog.");
    expect(needs()).toBe(4);
    // the new cards say who made them; opening one clears its glow
    const card = boardView(store.getState())
      .columns.find((c) => c.id === "backlog")
      ?.sections[0]?.cards.find((c) => c.k === "NOTE-14");
    expect(card?.metas.map((m) => m.t)).toContain("created by Project agent “Import GitHub issues”");
    store.openTicket("NOTE-14");
    expect(ticket("NOTE-14").fresh).toBe(false);
  });

  it("answering “Include closed ones” says the closed issues are fixed and imports the same two", () => {
    store.answer("imp-closed");
    const lines = agent("a1").lines.map((l) => l.t);
    expect(lines).toContain("❯ Include closed ones");
    expect(lines).toContain("All 3 closed issues are fixed in main, so there’s nothing to import from them.");
    expect(ticket("NOTE-14").col).toBe("backlog");
    expect(ticket("NOTE-15").col).toBe("backlog");
  });

  it("NOTE-6 to Dev starts its agent, then reaches Ready for Review on its timer", () => {
    expect(boardView(store.getState()).columns[0]?.coach).toBe(true);
    store.move("NOTE-6", "dev");
    expect(ticket("NOTE-6")).toMatchObject({ col: "dev", st: "starting", stage: "worktree" });
    expect(store.getState().toast).toBe("Starting NOTE-6 in its own worktree.");
    expect(store.getState().coach).toBe(false);
    expect(texts("NOTE-6")).toEqual(["Creating worktree ~/.otaq/wt/pocket-notes/NOTE-6 from main"]);

    vi.advanceTimersByTime(1300);
    expect(ticket("NOTE-6").stage).toBe("setup");
    expect(texts("NOTE-6")).toContain("Setup: npm ci · done in 6s");

    vi.advanceTimersByTime(1400);
    expect(ticket("NOTE-6")).toMatchObject({ st: "working", tokens: "3k tokens", age: "active just now" });
    expect(texts("NOTE-6")).toContain("Claude Code · Mira · opus");

    vi.advanceTimersByTime(6500);
    expect(ticket("NOTE-6").st).toBe("validating");
    expect(texts("NOTE-6")).toContain("Submitted NOTE-6 for review.");

    vi.advanceTimersByTime(4000);
    expect(ticket("NOTE-6")).toMatchObject({ st: "review", col: "review", adds: 40, dels: 4 });
    expect(ticket("NOTE-6").hist.at(-1)?.t).toBe("Validation round 1 passed · moved to Ready for Review");
    expect(store.getState().toast).toBe("NOTE-6 is ready for review.");
  });

  it("answering NOTE-8 lets it carry on, and the Needs you count follows", () => {
    expect(needs()).toBe(5);
    store.answer("note8");
    expect(ticket("NOTE-8")).toMatchObject({ st: "working", ask: null });
    expect(texts("NOTE-8")).toContain("❯ Open $EDITOR with the current text");
    expect(needs()).toBe(4);
    vi.advanceTimersByTime(3500);
    expect(ticket("NOTE-8").st).toBe("validating");
    vi.advanceTimersByTime(4000);
    expect(ticket("NOTE-8")).toMatchObject({ st: "review", col: "review" });
  });

  it("answering NOTE-8 “Fail with a usage message”", () => {
    store.answer("note8b");
    expect(ticket("NOTE-8").st).toBe("working");
    expect(texts("NOTE-8")).toContain("Okay. With no text, notes edit prints its usage and exits with 1.");
  });

  it("the inbox lists what needs you, what was interrupted and what is ready for review", () => {
    const groups = boardView(store.getState()).inbox;
    expect(groups.map((g) => [g.title, g.items.map((i) => i.k)])).toEqual([
      ["Needs you", ["NOTE-8", "SHOP-22", "Agent", "Agent"]],
      ["Interrupted", ["SHOP-25"]],
      ["Ready for review", ["NOTE-4", "NOTE-5", "SHOP-18"]],
    ]);
    store.toggleInbox();
    expect(store.getState().inboxOpen).toBe(true);
    store.openTicket("NOTE-4", "diff");
    expect(store.getState()).toMatchObject({ inboxOpen: false, openKey: "NOTE-4", ptab: "diff" });
  });

  it("approving NOTE-4 squash-merges it into Done", () => {
    store.approve("NOTE-4");
    expect(ticket("NOTE-4")).toMatchObject({ col: "review", st: "merging" });
    const review = boardView(store.getState()).columns.find((c) => c.id === "review");
    expect(review?.sections[1]?.cards.map((c) => c.k)).toEqual(["NOTE-4"]);
    vi.advanceTimersByTime(1500);
    expect(ticket("NOTE-4")).toMatchObject({ col: "done", st: "done" });
    expect(texts("NOTE-4").at(-1)).toBe("Squash-merged into main");
    expect(store.getState().toast).toBe("NOTE-4 is merged into main.");
    store.openTicket("NOTE-4");
    expect(boardView(store.getState()).panel?.isDone).toBe(true);
  });
});

describe("the other changes", () => {
  it("Send back returns a ticket to its agent, and it comes back for review", () => {
    store.sendBack("NOTE-5");
    expect(ticket("NOTE-5")).toMatchObject({ col: "dev", st: "working", round: 1 });
    expect(texts("NOTE-5")).toContain("❯ (sent back) Please also cover the empty-list case.");
    expect(store.getState().toast).toBe("NOTE-5 went back to its agent with your comment.");
    vi.advanceTimersByTime(3500 + 4000);
    expect(ticket("NOTE-5")).toMatchObject({ col: "review", st: "review", adds: 64, dels: 9 });
  });

  it("approving a ticket another waits for starts that one", () => {
    store.approve("SHOP-18");
    vi.advanceTimersByTime(1500);
    expect(store.getState().toast).toBe("SHOP-18 is merged. SHOP-21 was waiting for it and starts now.");
    vi.advanceTimersByTime(600);
    expect(ticket("SHOP-21")).toMatchObject({ col: "dev", st: "starting" });
  });

  it("SHOP-22's permission: Allow once and Deny", () => {
    store.answer("shop22");
    expect(texts("SHOP-22")).toContain("Applied migration 20261019_order_status");
    expect(ticket("SHOP-22").st).toBe("working");

    const other = new DemoStore();
    other.answer("shop22b");
    expect(other.find("SHOP-22")).toMatchObject({ st: "working", ask: null });
    expect(other.find("SHOP-22")?.log.map((l) => l.t)).toContain("❯ Deny");
    other.dispose();
  });

  it("SHOP-25 resumes its session", () => {
    expect(boardView(store.getState()).inbox.find((g) => g.title === "Interrupted")).toBeDefined();
    store.answer("shop25");
    expect(ticket("SHOP-25")).toMatchObject({ st: "working", ask: null });
    expect(texts("SHOP-25")).toContain("Resumed the same conversation");
    expect(boardView(store.getState()).inbox.find((g) => g.title === "Interrupted")).toBeUndefined();
  });

  it("the Checkout slowness agent: Create them and Not yet", () => {
    store.answer("shop-create");
    expect(["SHOP-35", "SHOP-36", "SHOP-37"].map((k) => ticket(k).title)).toEqual([
      "Fetch carrier quotes in parallel",
      "Cache shipping quotes per cart, 5 min",
      "Load the reviews widget after paint",
    ]);
    expect(ticket("SHOP-36").waits).toEqual(["SHOP-35"]);
    expect(agent("a3").created).toEqual(["SHOP-35", "SHOP-36", "SHOP-37"]);

    const other = new DemoStore();
    other.answer("shop-no");
    expect(other.getState().agents.find((a) => a.id === "a3")).toMatchObject({
      st: "idle",
      ask: null,
      created: [],
    });
    other.dispose();
  });

  it("dragging refuses Review and Done, and a ticket with unmerged dependencies waits in Ready for Dev", () => {
    store.move("NOTE-12", "review");
    expect(store.getState().toast).toBe(
      "Agents move tickets to Review when they submit. Approve from the ticket’s panel.",
    );
    store.move("NOTE-8", "backlog");
    expect(store.getState().toast).toBe("Stop the agent from the ticket’s panel first.");
    store.move("NOTE-4", "dev");
    expect(store.getState().toast).toBe("Use Send back in the ticket’s panel to return it to its agent.");
    store.move("NOTE-13.4", "dev");
    expect(ticket("NOTE-13.4")).toMatchObject({ col: "ready", st: "queued" });
    expect(store.getState().toast).toBe(
      "NOTE-13.4 waits for NOTE-13.2 and NOTE-13.3 to merge. It starts on its own after that.",
    );
    store.move("NOTE-12", "ready");
    expect(store.getState().toast).toBe("NOTE-12 starts when an agent is free, in priority order.");
    store.move("NOTE-12", "backlog");
    expect(ticket("NOTE-12").col).toBe("backlog");
  });

  it("typing to a new Project agent names it after the message", () => {
    store.selectAgent("a2");
    store.sendToAgent("Why is notes ls slow with 10,000 notes? Make tickets for what you find.");
    expect(agent("a2")).toMatchObject({ st: "working", name: "Why is notes ls slow wit…" });
    vi.advanceTimersByTime(1100);
    expect(agent("a2").st).toBe("idle");
    expect(agent("a2").lines.at(-1)?.c).toBe("d");
  });

  it("the toast goes after 3.6 s, and a new one restarts the clock", () => {
    store.say("one");
    vi.advanceTimersByTime(TOAST_MS - 100);
    store.say("two");
    vi.advanceTimersByTime(200);
    expect(store.getState().toast).toBe("two");
    vi.advanceTimersByTime(TOAST_MS);
    expect(store.getState().toast).toBeNull();
  });

  it("the timers set at start move NOTE-10, NOTE-13.2 and SHOP-19 on", () => {
    store.start();
    vi.advanceTimersByTime(9000);
    expect(ticket("NOTE-10")).toMatchObject({ col: "review", st: "review", adds: 37, dels: 5 });
    vi.advanceTimersByTime(16000 - 9000 + 6500 + 4000);
    expect(ticket("NOTE-13.2").col).toBe("review");
    vi.advanceTimersByTime(24000);
    expect(ticket("SHOP-19").col).toBe("review");
  });
});

describe("timers", () => {
  it("dispose clears every timer, and nothing changes afterwards", () => {
    store.start();
    store.move("NOTE-6", "dev");
    store.approve("NOTE-4");
    store.recheck();
    expect(store.pendingTimers()).toBeGreaterThan(0);
    expect(vi.getTimerCount()).toBe(store.pendingTimers());
    const before = store.getState();
    store.dispose();
    expect(store.pendingTimers()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(60_000);
    expect(store.getState()).toBe(before);
  });

  it("a fresh store starts afresh", () => {
    store.answer("imp-open");
    expect(new DemoStore().find("NOTE-14")).toBeUndefined();
  });
});
