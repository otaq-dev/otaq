import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { cardActions, requestCardAction } from "../../app/cardActions";
import { overlays } from "../../app/overlays";
import { registered, registerScreens } from "../../app/registry";
import type { BoardColumn } from "../../generated/BoardColumn";
import type { DragAnswer } from "../../generated/DragAnswer";
import type { DropOutcome } from "../../generated/DropOutcome";
import { afterDrop, dropCard, openApproveChoice } from "./Board";
import { Column, type ColumnProps } from "./Column";
import { aboveChip, type Ask, columnAt, createDrag, type DragState, fill, moveFocus } from "./drag";
import { board } from ".";
import { boardPrefs, changePrefs, prefsOf, rememberColumnScroll, scrollPlace } from "./prefs";

afterEach(() => {
  overlays.store.set([]);
  boardPrefs.set({});
  cardActions.set(null);
});

const ANSWER: DragAnswer = {
  key: "SHOP-24",
  columns: [
    { column: "backlog", verdict: { kind: "home" } },
    { column: "ready_for_dev", verdict: { kind: "allowed", icon: "play", hint: "Drop to queue" } },
    { column: "dev", verdict: { kind: "allowed", icon: "play", hint: "Drop to queue" } },
    {
      column: "ready_for_review",
      verdict: { kind: "forbidden", title: "Only agents move tickets to Ready for Review", detail: null },
    },
    { column: "approved", verdict: { kind: "forbidden", title: "Approve from Review", detail: null } },
    { column: "done", verdict: { kind: "forbidden", title: "No", detail: null } },
  ],
};

const SPANS = [
  { id: "backlog", left: 10, right: 226 },
  { id: "ready_for_dev", left: 234, right: 450 },
  { id: "dev", left: 458, right: 674 },
  { id: "ready_for_review", left: 682, right: 898 },
  { id: "approved", left: 906, right: 1122 },
  { id: "done", left: 1130, right: 1164 },
] as const;

/** A fake door that counts its requests and answers `board.drag` with `ANSWER`. */
function fakeDoor() {
  const calls: { method: string; params: unknown }[] = [];
  let answer: (value: unknown) => void = () => undefined;
  const ask: Ask = (method, params) => {
    calls.push({ method, params });
    return new Promise((resolve) => {
      answer = resolve;
    });
  };
  return {
    calls,
    ask,
    answer: (value: unknown = ANSWER) => {
      answer(value);
    },
  };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("a drag asks Rust once (UIT-04)", () => {
  it("asks at drag start and never per pointer move", async () => {
    const door = fakeDoor();
    const states: (DragState | null)[] = [];
    const drag = createDrag(door.ask, { onState: (s) => states.push(s) });
    let measured = 0;
    const spans = () => {
      measured += 1;
      return SPANS;
    };
    drag.press("SHOP-24", 100, 100);
    expect(drag.move(102, 101, spans)).toBe(false);
    expect(door.calls).toEqual([]);
    for (let x = 105; x < 1100; x += 3) drag.move(x, 120 + (x % 7), spans);
    expect(door.calls).toEqual([{ method: "board.drag", params: { key: "SHOP-24" } }]);
    expect(measured).toBe(1);
    // A state change per column crossed, not per move.
    expect(states.length).toBeLessThan(12);
    door.answer();
    await flush();
    expect(drag.state()?.answer).toEqual(ANSWER);
    expect(door.calls).toHaveLength(1);
  });

  it("drops only on an allowed column, after the answer came", async () => {
    const door = fakeDoor();
    const drag = createDrag(door.ask, { onState: () => undefined });
    drag.press("SHOP-24", 100, 100);
    drag.move(300, 100, () => SPANS);
    expect(drag.release()).toEqual({ kind: "none" });

    drag.press("SHOP-24", 100, 100);
    drag.move(300, 100, () => SPANS);
    door.answer();
    await flush();
    expect(drag.release()).toEqual({ kind: "drop", key: "SHOP-24", column: "ready_for_dev" });

    for (const [x, why] of [
      [700, "forbidden"],
      [150, "its own column"],
      [230, "a gap"],
    ] as const) {
      drag.press("SHOP-24", 120, 100);
      drag.move(x, 100, () => SPANS);
      door.answer();
      await flush();
      expect(drag.release(), why).toEqual({ kind: "none" });
    }
  });

  it("a short press is a click, and Esc cancels", async () => {
    const door = fakeDoor();
    const drag = createDrag(door.ask, { onState: () => undefined });
    drag.press("SHOP-24", 100, 100);
    drag.move(101, 101, () => SPANS);
    expect(drag.release()).toEqual({ kind: "click", key: "SHOP-24" });
    drag.press("SHOP-24", 100, 100);
    drag.move(300, 100, () => SPANS);
    door.answer();
    await flush();
    drag.cancel();
    expect(drag.state()).toBeNull();
    expect(drag.release()).toEqual({ kind: "none" });
    expect(door.calls).toHaveLength(1);
  });

  it("an answer for an earlier drag is ignored", async () => {
    const door = fakeDoor();
    const drag = createDrag(door.ask, { onState: () => undefined });
    drag.press("SHOP-24", 100, 100);
    drag.move(300, 100, () => SPANS);
    drag.cancel();
    door.answer();
    await flush();
    expect(drag.state()).toBeNull();
  });

  it("Space lifts, ←/→ move between columns, Space drops", async () => {
    const door = fakeDoor();
    const drag = createDrag(door.ask, { onState: () => undefined });
    const order = SPANS.map((s) => s.id);
    drag.lift("SHOP-24", "backlog");
    door.answer();
    await flush();
    drag.step(-1, order);
    expect(drag.state()?.over).toBe("backlog");
    drag.step(1, order);
    drag.step(1, order);
    expect(drag.state()?.over).toBe("dev");
    // Pointer moves don't move a keyboard drag.
    drag.move(700, 100, () => SPANS);
    expect(drag.state()?.over).toBe("dev");
    expect(drag.release()).toEqual({ kind: "drop", key: "SHOP-24", column: "dev" });
    expect(door.calls).toHaveLength(1);
  });
});

describe("board helpers", () => {
  it("finds the column under the pointer", () => {
    expect(columnAt(SPANS, 300)).toBe("ready_for_dev");
    expect(columnAt(SPANS, 230)).toBeNull();
    expect(columnAt(SPANS, 1140)).toBe("done");
  });

  it("words the scrolled column's chip from Rust's templates", () => {
    const words = { above: "{n} above", need_you: "{n} need you" };
    expect(fill("{n} above", 3)).toBe("3 above");
    expect(aboveChip(words, [])).toBeNull();
    expect(aboveChip(words, [false, false, false])).toEqual({ text: "3 above", needYou: false });
    expect(aboveChip(words, [true, true, false, true, true, true, true])).toEqual({
      text: "7 above · 6 need you",
      needYou: true,
    });
  });

  it("walks cards with the arrow keys, skipping empty columns", () => {
    const columns = [["A", "B"], [], ["C"], ["D", "E", "F"]];
    expect(moveFocus(columns, 0, 0, "ArrowDown")).toEqual({ col: 0, row: 1 });
    expect(moveFocus(columns, 0, 1, "ArrowDown")).toBeNull();
    expect(moveFocus(columns, 0, 1, "ArrowRight")).toEqual({ col: 2, row: 0 });
    expect(moveFocus(columns, 2, 0, "ArrowLeft")).toEqual({ col: 0, row: 0 });
    expect(moveFocus(columns, 3, 2, "ArrowLeft")).toEqual({ col: 2, row: 0 });
    expect(moveFocus(columns, 3, 0, "ArrowRight")).toBeNull();
  });

  it("keeps each project's choices and scroll places", () => {
    changePrefs("SHOP", (p) => ({ ...p, doneCollapsed: false }));
    expect(prefsOf(boardPrefs.get(), "SHOP").doneCollapsed).toBe(false);
    expect(prefsOf(boardPrefs.get(), "DOCS").doneCollapsed).toBeNull();
    rememberColumnScroll("SHOP", "dev", 420);
    expect(scrollPlace("SHOP").columns.dev).toBe(420);
    expect(scrollPlace("DOCS").columns.dev).toBeUndefined();
  });

  it("leaves a card's button press for the panel", () => {
    requestCardAction("SHOP-25", "resume");
    requestCardAction("SHOP-25", "resume");
    expect(cardActions.get()).toEqual({ ticket: "SHOP-25", kind: "resume", seq: 2 });
  });
});

describe("drops", () => {
  const park: DropOutcome = {
    kind: "park",
    dialog: {
      key: "SHOP-25",
      title: "Park SHOP-25?",
      message: "SHOP-25’s agent will be stopped. Its branch, files and conversation are kept.",
      kept: [{ icon: "git-branch", text: "crewboard/SHOP-25" }],
      confirm: "Park",
      confirm_icon: "square-parking",
    },
  };

  it("Park asks first, and its button sends board.park", async () => {
    const asked: [string, unknown][] = [];
    const ask = (method: string, params?: unknown) => {
      asked.push([method, params]);
      return Promise.resolve(null);
    };
    afterDrop(park, () => undefined, ask);
    const open = overlays.store.get();
    expect(open.map((o) => o.id)).toEqual(["confirm"]);
    const confirm = open[0];
    if (confirm?.id !== "confirm") throw new Error("no confirm");
    expect(confirm.params.title).toBe("Park SHOP-25?");
    expect(confirm.params.rows).toEqual([{ icon: "git-branch", text: "crewboard/SHOP-25" }]);
    expect(asked).toEqual([]);
    confirm.params.onConfirm();
    await flush();
    expect(asked).toEqual([["board.park", { key: "SHOP-25" }]]);
  });

  it("a drop on Approved shows the approve choice; its choices open their dialogs", async () => {
    const menus: string[] = [];
    const ask = (method: string) =>
      Promise.resolve(
        method === "board.drop"
          ? ({
              kind: "approve",
              menu: { key: "SHOP-18", title: "Approve SHOP-18 Checkout API", items: [], note: "" },
            } satisfies DropOutcome)
          : null,
      );
    await dropCard("SHOP-18", "approved", (menu) => menus.push(menu.title), ask);
    expect(menus).toEqual(["Approve SHOP-18 Checkout API"]);
    openApproveChoice("local_merge", "SHOP-18");
    openApproveChoice("push_to_pr", "SHOP-26");
    expect(overlays.store.get()).toEqual([
      { id: "local_merge", params: { ticket: "SHOP-18" } },
      { id: "open_pr", params: { ticket: "SHOP-26" } },
    ]);
  });

  it("a move needs nothing more; a not-yet move and a refusal are toasts", async () => {
    const asked: [string, unknown][] = [];
    const ask = (method: string, params?: unknown) => {
      asked.push([method, params]);
      if (method === "board.drop") return Promise.reject(new Error("PR rework isn't available yet"));
      return Promise.resolve(null);
    };
    afterDrop({ kind: "moved" }, () => undefined, ask);
    expect(asked).toEqual([]);
    afterDrop({ kind: "not_yet", message: "This move isn’t available yet" }, () => undefined, ask);
    expect(asked[0]?.[0]).toBe("notices.toast");
    await dropCard("SHOP-26", "ready_for_review", () => undefined, ask);
    expect(asked[1]).toEqual(["board.drop", { key: "SHOP-26", column: "ready_for_review" }]);
    expect(asked[2]).toEqual([
      "notices.toast",
      { level: "error", title: "PR rework isn't available yet", text: null, link: null, ticket: null },
    ]);
    expect(overlays.store.get()).toEqual([]);
  });
});

describe("the board screen", () => {
  it("fills the board slot and ⌘N opens a new ticket", () => {
    const app = registerScreens({ board });
    expect(registered(app).slots).toEqual(["board"]);
    expect(app.shortcuts.run({ action: "new_ticket" })).toBe(true);
    expect(overlays.store.get()).toEqual([{ id: "new_ticket", params: undefined }]);
  });

  const column = (over: Partial<BoardColumn>): BoardColumn => ({
    id: "approved",
    title: "Approved",
    icon: "check",
    count: "0",
    hint: null,
    note: null,
    empty_hint: "Drag a ticket here from Ready for Review to approve it.",
    collapsed: false,
    toggle: null,
    hidden: null,
    cards: [],
    dev: null,
    add: null,
    ...over,
  });

  const props = (c: BoardColumn): ColumnProps => ({
    column: c,
    chipWords: { above: "{n} above", need_you: "{n} need you" },
    dragging: false,
    selected: null,
    lifted: null,
    handlers: {
      press: () => undefined,
      key: () => undefined,
      open: () => undefined,
      action: () => undefined,
      addToEpic: () => undefined,
    },
    scrollTop: 0,
    onScroll: () => undefined,
    onToggleDone: () => undefined,
    onUnfold: () => undefined,
    onAdd: () => undefined,
    element: () => undefined,
  });

  it("draws an empty column with its hint, and Done folded as the rail", () => {
    const empty = renderToStaticMarkup(createElement(Column, props(column({}))));
    expect(empty).toContain("Drag a ticket here from Ready for Review");
    expect(empty).toContain('data-column="approved"');
    const rail = renderToStaticMarkup(
      createElement(
        Column,
        props(column({ id: "done", title: "Done", count: "2", collapsed: true, toggle: "Expand Done" })),
      ),
    );
    expect(rail).toContain("board-rail");
    expect(rail).not.toContain("board-column-cards");
  });

  it("outlines a column with its verdict only while dragging", () => {
    const c = column({ id: "ready_for_review", title: "Ready for Review" });
    const forbidden = { kind: "forbidden", title: "Only agents", detail: null } as const;
    const drawn = renderToStaticMarkup(
      createElement(Column, { ...props(c), verdict: forbidden, dragging: true }),
    );
    expect(drawn).toContain('data-verdict="forbidden"');
    const idle = renderToStaticMarkup(createElement(Column, { ...props(c), verdict: forbidden }));
    expect(idle).not.toContain("data-verdict");
    const home = renderToStaticMarkup(
      createElement(Column, { ...props(c), verdict: { kind: "home" }, dragging: true }),
    );
    expect(home).not.toContain("data-verdict");
  });
});
