import { describe, expect, it } from "vitest";

import { createOverlayStack, modalOpen } from "./overlays";
import { closeTicket, createSelection, focusTerminal, openTicket, selectProject, showTab } from "./selection";
import { createStore } from "./store";

describe("the store", () => {
  it("tells listeners about changes only", () => {
    const store = createStore(1);
    let calls = 0;
    const stop = store.subscribe(() => calls++);
    store.set(1);
    store.update((n) => n + 1);
    expect([store.get(), calls]).toEqual([2, 1]);
    stop();
    store.set(5);
    expect(calls).toBe(1);
  });
});

describe("the selection", () => {
  it("opens and closes a ticket, keeps the tab, and starts each project closed", () => {
    const s = createSelection();
    selectProject("SHOP", s);
    openTicket("SHOP-22", "diff", s);
    focusTerminal("session-1", s);
    expect(s.get()).toEqual({
      project: "SHOP",
      ticket: "SHOP-22",
      tab: "diff",
      focusedTerminal: "session-1",
    });

    openTicket("SHOP-23", undefined, s);
    expect(s.get().tab).toBe("diff");
    showTab("history", s);
    closeTicket(s);
    expect(s.get()).toEqual({ project: "SHOP", ticket: null, tab: "history", focusedTerminal: null });

    openTicket("SHOP-22", undefined, s);
    selectProject("CREW", s);
    expect(s.get()).toMatchObject({ project: "CREW", ticket: null });
  });
});

describe("the overlay stack", () => {
  it("opens by id with params, newest on top, and knows when a modal is open", () => {
    const stack = createOverlayStack();
    stack.open("inbox");
    stack.open("edit_ticket", { ticket: "SHOP-22" });
    expect(stack.store.get().map((o) => o.id)).toEqual(["inbox", "edit_ticket"]);
    expect(modalOpen(stack.store.get())).toBe(true);

    stack.open("inbox");
    expect(stack.store.get().map((o) => o.id)).toEqual(["edit_ticket", "inbox"]);
    stack.closeTop();
    stack.toggle("edit_ticket", { ticket: "SHOP-22" });
    expect(stack.store.get()).toEqual([]);
    expect(modalOpen(stack.store.get())).toBe(false);

    stack.toggle("inbox");
    expect(modalOpen(stack.store.get())).toBe(false);
    stack.toggle("inbox");
    stack.closeTop();
    expect(stack.store.get()).toEqual([]);
  });
});
