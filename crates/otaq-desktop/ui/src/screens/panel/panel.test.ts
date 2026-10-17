import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { cardActions, requestCardAction, takeCardAction } from "../../app/cardActions";
import { overlays } from "../../app/overlays";
import { createRegistries, registered, registerScreens, RegistriesContext } from "../../app/registry";
import { NOTHING_SELECTED, requestTicketEdit, selection, ticketEditRequests } from "../../app/selection";
import type { CardActionKind } from "../../generated/CardActionKind";
import type { PanelAction } from "../../generated/PanelAction";
import type { PanelAnswerRow } from "../../generated/PanelAnswerRow";
import type { PanelTab } from "../../generated/PanelTab";
import type { PanelView } from "../../generated/PanelView";
import { panel } from ".";
import {
  actionRoute,
  canSend,
  cardRoute,
  draggedWidth,
  iconTabs,
  initialPick,
  menuRoute,
  numberedTab,
  pickByKey,
  picksOther,
  shownTab,
  shownWidth,
} from "./logic";
import { PanelBody } from "./Panel";

afterEach(() => {
  overlays.store.set([]);
  selection.set(NOTHING_SELECTED);
  ticketEditRequests.set(null);
  cardActions.set(null);
});

const SIZES = {
  default_widths: { w1280: 600, w1440: 660, w1920: 760 },
  min_width: 480,
  main_keeps: 240,
  icon_tabs_below: 560,
};
const hint = (text: string, keys: string | null = null) => ({ text, keys });
const tab = (t: PanelTab, label: string, n: number) => ({
  tab: t,
  label,
  icon: "file-text" as const,
  keys: `⌘⌥${String(n)}`,
  mark: null,
});

const VIEW: PanelView = {
  key: "SHOP-19",
  title: "Price table",
  epic: false,
  priority: {
    number: 3,
    short: "P3",
    word: "Normal",
    label: "P3 · Normal",
    icon: "chevron-up",
    ink: "muted",
  },
  priority_hint: "Priority",
  priority_menu: [],
  pill: { palette: "working", icon: "loader", label: "Agent Working" },
  branch: { shown: "otaq/SHOP-19", full: "otaq/SHOP-19" },
  copy_branch: "Copy branch name",
  base: "main",
  signal: { text: "40s ago", stale: false },
  tokens: { text: "182k", tooltip: null },
  token_split: null,
  previous: null,
  next: "SHOP-20",
  previous_hint: hint("Previous ticket", "⌘["),
  next_hint: hint("Next ticket", "⌘]"),
  expand_hint: hint("Expand", "⇧⌘E"),
  collapse_hint: hint("Back to the board", "⇧⌘E"),
  close_hint: hint("Close", "Esc"),
  more_hint: "More",
  resize_hint: "Panel width",
  buttons: [{ action: "open_terminal", label: "Open terminal", icon: "square-terminal", look: "secondary" }],
  approve_more: null,
  approve_opens_menu: false,
  stop_message: null,
  answer_agent: null,
  parent: null,
  menu: [],
  attach_command: "otaq attach SHOP-19",
  answer: null,
  notice: null,
  conflict: null,
  start_problem: null,
  mark_closed: null,
  run: null,
  tabs: [
    { ...tab("terminal", "Terminal", 1), mark: { kind: "live" } },
    tab("diff", "Diff", 2),
    tab("run", "Run", 3),
    tab("shell", "Shell", 4),
    tab("ticket", "Ticket", 5),
    { ...tab("history", "History", 6), mark: { kind: "note", text: "1" } },
  ],
  live_agent: true,
  sizes: SIZES,
  agents: null,
  focus_agent_row: null,
};

function render(view: PanelView, opts: { tab?: PanelTab; expanded?: boolean; width?: number } = {}) {
  const app = createRegistries();
  app.panelTabs.register("history", {
    Component: ({ ticket }) => createElement("p", null, `history of ${ticket}`),
  });
  app.panelSections.register("stack", { Component: () => createElement("p", null, "the stack") });
  return renderToStaticMarkup(
    createElement(
      RegistriesContext,
      { value: app },
      createElement(PanelBody, {
        view,
        tab: opts.tab ?? "terminal",
        expanded: opts.expanded ?? false,
        width: opts.width ?? 660,
      }),
    ),
  );
}

describe("the panel's layout", () => {
  it("a drag keeps at least 480 px and leaves the main area 240 px", () => {
    // The main area at 1440 with the pane at 400: 976 px, from x 464 to the window's right edge.
    expect(draggedWidth(1440 - 700, 1440, 976, SIZES)).toBe(700);
    expect(draggedWidth(1400, 1440, 976, SIZES)).toBe(480);
    expect(draggedWidth(464, 1440, 976, SIZES)).toBe(736);
    expect(draggedWidth(0, 1920, 1396, SIZES)).toBe(1156);
    expect(draggedWidth(0, 800, 500, SIZES)).toBe(480);
  });

  it("the class's default until dragged, each class keeping its own drag; icons only under 560 px unless expanded", () => {
    expect(shownWidth({}, "w1280", 896, SIZES)).toBe(600);
    expect(shownWidth({}, "w1440", 976, SIZES)).toBe(660);
    expect(shownWidth({}, "w1920", 1396, SIZES)).toBe(760);
    const dragged = { w1920: 900 };
    expect(shownWidth(dragged, "w1920", 1396, SIZES)).toBe(900);
    expect(shownWidth(dragged, "w1440", 976, SIZES)).toBe(660);
    expect(shownWidth({ w1440: 900 }, "w1440", 976, SIZES)).toBe(736);
    expect(iconTabs(480, false, SIZES)).toBe(true);
    expect(iconTabs(480, true, SIZES)).toBe(false);
    expect(iconTabs(660, false, SIZES)).toBe(false);
  });
});

describe("an epic's tabs (T-0290, d0083)", () => {
  const EPIC: PanelTab[] = ["ticket", "lead", "history"];
  it("shows the selected tab when the ticket has it, else its first", () => {
    expect(shownTab(EPIC, "terminal")).toBe("ticket");
    expect(shownTab(EPIC, "lead")).toBe("lead");
    expect(shownTab(["diff", ...EPIC], "run")).toBe("diff");
    expect(shownTab(["terminal", "diff"], "terminal")).toBe("terminal");
  });

  it("numbers ⌘⌥N in the shown order", () => {
    expect(numberedTab(EPIC, 2)).toBe("lead");
    expect(numberedTab(EPIC, 4)).toBeUndefined();
    expect(numberedTab(EPIC, 0)).toBeUndefined();
  });
});

describe("where buttons and menu items go", () => {
  it("service buttons call panel.action; the rest open a tab or a dialog", () => {
    const service: PanelAction[] = [
      "start",
      "retry_start",
      "resume",
      "retry_resume",
      "reopen",
      "mark_merged",
      "resume_lead",
    ];
    for (const action of service) expect(actionRoute(action)).toEqual({ kind: "service" });
    expect(actionRoute("answer_in_terminal")).toEqual({ kind: "terminal" });
    expect(actionRoute("approve")).toEqual({ kind: "overlay", id: "local_merge" });
    expect(actionRoute("send_back")).toEqual({ kind: "overlay", id: "send_back" });
    expect(actionRoute("edit")).toEqual({ kind: "edit" });
    expect(actionRoute("mark_closed")).toEqual({ kind: "mark_closed" });
  });

  it("the validation buttons (d0063, d0064)", () => {
    const service: PanelAction[] = [
      "move_to_review_anyway",
      "another_round",
      "retry_validation",
      "retry_merge",
      "move_to_review",
    ];
    for (const action of service) expect(actionRoute(action)).toEqual({ kind: "service" });
    expect(actionRoute("open_validator")).toEqual({ kind: "validator" });
    expect(actionRoute("answer_validator")).toEqual({ kind: "validator" });
    expect(actionRoute("answer_agent")).toEqual({ kind: "answer_agent" });
    for (const item of ["stop-validation", "skip-to-review", "validate-and-submit"])
      expect(menuRoute(item)).toEqual({ kind: "service" });
  });

  it("the … menu's ids from the view models", () => {
    expect(menuRoute("close-ticket")).toEqual({ kind: "overlay", id: "close_ticket" });
    expect(menuRoute("submit-for-review")).toEqual({ kind: "overlay", id: "submit_for_review" });
    expect(menuRoute("delete")).toEqual({ kind: "overlay", id: "delete_ticket" });
    expect(menuRoute("pin")).toEqual({ kind: "service" });
    expect(menuRoute("copy-attach")).toEqual({ kind: "copy", what: "attach" });
    expect(menuRoute("something")).toEqual({ kind: "unknown" });
  });

  it("Edit shows the Ticket tab and asks it for its form", () => {
    selection.set({ ...NOTHING_SELECTED, ticket: "SHOP-19" });
    requestTicketEdit("SHOP-19");
    requestTicketEdit("SHOP-19");
    expect(selection.get().tab).toBe("ticket");
    expect(ticketEditRequests.get()).toEqual({ ticket: "SHOP-19", seq: 2 });
  });
});

describe("a board card's button", () => {
  it("is taken once, by the panel showing its ticket", () => {
    requestCardAction("SHOP-25", "resume");
    expect(takeCardAction("SHOP-18")).toBeNull();
    expect(takeCardAction("SHOP-25")).toMatchObject({ ticket: "SHOP-25", kind: "resume" });
    expect(takeCardAction("SHOP-25")).toBeNull();
  });

  it("runs as the GPUI app's card buttons did", () => {
    const service: CardActionKind[] = [
      "deliver_now",
      "resume",
      "retry_resume",
      "retry_start",
      "retry",
      "reopen",
      "mark_merged",
      "retry_validation",
      "retry_merge",
      "move_to_review",
    ];
    for (const kind of service) expect(cardRoute(kind)).toEqual({ kind: "service" });
    expect(cardRoute({ pick_option: 3 })).toEqual({ kind: "pick", number: 3 });
    expect(cardRoute("send_answer")).toEqual({ kind: "send_answer" });
    expect(cardRoute("start_from_scratch")).toEqual({ kind: "overlay", id: "start_from_scratch" });
    expect(cardRoute("push_to_pr")).toEqual({ kind: "overlay", id: "open_pr" });
    expect(cardRoute("mark_closed")).toEqual({ kind: "mark_closed" });
    expect(cardRoute("review")).toEqual({ kind: "none" });
    expect(cardRoute("run_chip")).toEqual({ kind: "run_tab" });
    expect(actionRoute("run")).toEqual({ kind: "run" });
  });
});

describe("answer options", () => {
  const rows: PanelAnswerRow[] = [
    { number: 1, label: "Postmark", recommended: true, picked: true, other: false },
    { number: 2, label: "SES", recommended: false, picked: false, other: false },
    { number: 3, label: "Other… type your own answer", recommended: false, picked: false, other: true },
  ];

  it("starts on the recommendation; number keys pick; Other… needs text", () => {
    expect(initialPick(rows)).toBe(1);
    expect(initialPick(rows.map((r) => ({ ...r, picked: false })))).toBeNull();
    expect(pickByKey("2", rows)).toBe(2);
    expect(pickByKey("4", rows)).toBeNull();
    expect(pickByKey("a", rows)).toBeNull();
    expect(picksOther(3, rows)).toBe(true);
    expect(canSend(null, "", rows)).toBe(false);
    expect(canSend(2, "", rows)).toBe(true);
    expect(canSend(3, "  ", rows)).toBe(false);
    expect(canSend(3, "Mailgun", rows)).toBe(true);
  });

  it("replaces the action bar with the question and its options", () => {
    const html = render({
      ...VIEW,
      answer: {
        question: "q1",
        text: "Which email provider?",
        rows,
        recommended_label: "Recommended",
        other_placeholder: "Type your answer",
        hint: "1–4 pick · ↵ sends",
        in_terminal: "Answer in terminal",
        send: "Send answer",
      },
    });
    expect(html).toContain("Which email provider?");
    expect(html.match(/role="radio"/g)).toHaveLength(3);
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain("Send answer");
    expect(html).not.toContain("Open terminal");
  });
});

describe("the panel", () => {
  it("draws the header, the bar, the registered section and the tab strip", () => {
    const html = render(VIEW);
    for (const text of [
      "SHOP-19",
      "Price table",
      "Agent Working",
      "otaq/SHOP-19",
      "main",
      "40s ago",
      "182k",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain("Open terminal");
    expect(html).toContain("the stack");
    expect(html.match(/role="tab"/g)).toHaveLength(6);
    expect(html).toContain('aria-label="Previous ticket" disabled=""');
    expect(html).toContain('role="separator"');
  });

  it("shows the registered tab's body; an unregistered tab stays empty", () => {
    expect(render(VIEW, { tab: "history" })).toContain("history of SHOP-19");
    expect(render(VIEW, { tab: "diff" })).toContain('<div class="panel-tab-body" data-tab="diff"></div>');
  });

  it("expanded: no resize edge and no fixed width", () => {
    const html = render(VIEW, { expanded: true });
    expect(html).toContain("data-expanded");
    expect(html).not.toContain('role="separator"');
  });

  it("the queued notice, the conflict strip and the start callout", () => {
    const html = render({
      ...VIEW,
      notice: { text: "Waits for SHOP-18 to merge.", parent_closed: false, choices: [] },
      conflict: { line: "Conflicts with main · 1 file (a.ts)", fix: "Send to agent to fix" },
      start_problem: {
        heading: "The start didn’t finish",
        message: "SHOP-19 stopped before its agent session started.",
        has_log: false,
        show_log: "View the setup log",
        hide_log: "Hide the setup log",
        retry_heading: "RETRY START DOES",
        retry_does: ["Runs the whole start again"],
      },
    });
    for (const text of [
      "Waits for SHOP-18",
      "Conflicts with main",
      "Send to agent to fix",
      "RETRY START DOES",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).not.toContain("View the setup log");
  });

  it("the M12 extras: token split, Auto-merge stop message and the Approve caret", () => {
    const html = render({
      ...VIEW,
      token_split: "agent 58k · validation 9k (1 round)",
      stop_message: "Keeps conflicting with otaq/SHOP-40 (3 tries in a row).",
      buttons: [{ action: "approve", label: "Approve · Local merge", icon: null, look: "primary" }],
      approve_more: "Approve choices",
    });
    expect(html).toContain("agent 58k · validation 9k (1 round)");
    expect(html).toContain("Keeps conflicting with otaq/SHOP-40");
    expect(html).toContain("▾");
    expect(render(VIEW)).not.toContain("▾");
  });

  it("registers the panel slot and its two dialogs from its folder", () => {
    const app = registerScreens({ panel });
    expect(registered(app)).toMatchObject({
      slots: ["panel"],
      overlays: ["close_ticket", "submit_for_review"],
    });
  });
});
