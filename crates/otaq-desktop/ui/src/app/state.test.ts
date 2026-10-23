import { describe, expect, it } from "vitest";

import { createOverlayStack, modalOpen } from "./overlays";
import {
  closeTicket,
  createSelection,
  focusTerminal,
  openTicket,
  selectProject,
  showTab,
  stepTicket,
} from "./selection";
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
  it("opens a ticket on its first tab, j / k keep the tab, and a new project starts closed (SHT-02)", () => {
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

    stepTicket("SHOP-23", s);
    expect(s.get().tab).toBe("diff");
    openTicket("SHOP-23", undefined, s);
    expect(s.get().tab, "the open ticket keeps it").toBe("diff");
    openTicket("SHOP-24", undefined, s);
    expect(s.get().tab).toBeNull();
    showTab("history", s);
    closeTicket(s);
    expect(s.get()).toEqual({ project: "SHOP", ticket: null, tab: "history", focusedTerminal: null });

    openTicket("SHOP-22", undefined, s);
    selectProject("CREW", s);
    expect(s.get()).toMatchObject({ project: "CREW", ticket: null });
  });

  it("switching projects keeps what each one showed", () => {
    const s = createSelection();
    selectProject("SHOP", s);
    openTicket("SHOP-22", "diff", s);
    focusTerminal("session-1", s);
    selectProject("CREW", s);
    openTicket("CREW-3", "terminal", s);
    selectProject("SHOP", s);
    expect(s.get()).toEqual({ project: "SHOP", ticket: "SHOP-22", tab: "diff", focusedTerminal: null });
    selectProject("CREW", s);
    expect(s.get()).toMatchObject({ project: "CREW", ticket: "CREW-3", tab: "terminal" });
  });
});

describe("the overlay stack", () => {
  it("opens by id with params, newest on top, and knows when a modal is open", () => {
    const stack = createOverlayStack();
    stack.open("activity");
    stack.open("edit_ticket", { ticket: "SHOP-22" });
    expect(stack.store.get().map((o) => o.id)).toEqual(["activity", "edit_ticket"]);
    expect(modalOpen(stack.store.get())).toBe(true);

    stack.open("activity");
    expect(stack.store.get().map((o) => o.id)).toEqual(["edit_ticket", "activity"]);
    stack.closeTop();
    stack.toggle("edit_ticket", { ticket: "SHOP-22" });
    expect(stack.store.get()).toEqual([]);
    expect(modalOpen(stack.store.get())).toBe(false);

    stack.toggle("activity");
    expect(modalOpen(stack.store.get())).toBe(false);
    stack.toggle("activity");
    stack.closeTop();
    expect(stack.store.get()).toEqual([]);
  });
});
