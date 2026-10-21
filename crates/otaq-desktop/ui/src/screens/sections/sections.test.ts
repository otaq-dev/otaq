import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { registered, registerScreens } from "../../app/registry";
import type { CloseEpicDialogView } from "../../generated/CloseEpicDialogView";
import type { EpicView } from "../../generated/EpicView";
import type { HistoryRowView } from "../../generated/HistoryRowView";
import type { HistoryView } from "../../generated/HistoryView";
import type { MessagesView } from "../../generated/MessagesView";
import type { MoveDialogView } from "../../generated/MoveDialogView";
import type { PillView } from "../../generated/PillView";
import type { StackView } from "../../generated/StackView";
import type { ValidationReportView } from "../../generated/ValidationReportView";
import { sections } from ".";
import { EpicBody } from "./Epic";
import { CloseEpicBody, MoveDialogBody } from "./EpicDialogs";
import { HistoryBody } from "./History";
import { canMove, followMove, pickedChoice, progressPercent, stackOpen } from "./logic";
import { MessagesBody } from "./Messages";
import { Report } from "./Report";
import { StackBody } from "./Stack";

const pill = (label: string): PillView => ({ palette: "queued", icon: "hourglass", label });

const STACK: StackView = {
  title: "Dependencies",
  summary: "Depends on SHOP-18",
  count: "2 tickets",
  rows: [
    { key: "SHOP-18", title: "Checkout API", depth: 0, current: false, waits: null, pill: pill("Review") },
    {
      key: "SHOP-20",
      title: "Checkout page",
      depth: 1,
      current: true,
      waits: "waits for SHOP-18 to merge",
      pill: pill("Ready for Dev"),
    },
  ],
  add_dependency: "+ Add dependency",
  show_hint: "Show dependencies",
  hide_hint: "Hide dependencies",
};

const MOVE: MoveDialogView = {
  old_key: "SHOP-51",
  title: "Move SHOP-51 Wishlist share link to an epic",
  choices: [
    {
      key: "SHOP-40",
      title: "Wishlist",
      progress: "1 of 4 merged",
      new_key: "SHOP-40.4",
      heading: "Move SHOP-51 Wishlist share link to an epic",
      alias_note: "· the old key SHOP-51 still works",
      base_note: null,
      confirm: "Move to SHOP-40",
    },
    {
      key: "SHOP-60",
      title: "Gift cards",
      progress: "0 of 1 merged",
      new_key: "SHOP-60.2",
      heading: "Move SHOP-51 Wishlist share link to an epic",
      alias_note: "· the old key SHOP-51 still works",
      base_note: "SHOP-51 will start from the epic’s branch instead of release/2.3, and merge back into it.",
      confirm: "Move to SHOP-60",
    },
  ],
  disk_note: "No branch or worktree exists yet, so nothing on disk is renamed.",
  refusal: null,
};

const html = (el: Parameters<typeof renderToStaticMarkup>[0]) => renderToStaticMarkup(el);

/** A failed round's report: one finding with its place, a check, a linked note and an uncovered gap. */
export const REPORT: ValidationReportView = {
  title: "Validation report",
  meta: "round 1 · 4 min · 12k",
  failed: true,
  findings: [
    {
      text: [{ text: "The description asks for keyboard navigation; arrow keys do nothing", key: false }],
      place: "components/AddressAutocomplete.tsx:88",
    },
  ],
  checked_heading: null,
  checked: [[{ text: "pnpm test passed", key: false }]],
  notes_heading: null,
  notes: [
    {
      text: [
        { text: "Order confirmation email: covered by ", key: false },
        { text: "SHOP-21", key: true },
      ],
      create: null,
    },
    {
      text: [{ text: "Not covered by any ticket: refunds endpoint", key: false }],
      create: "Not covered by any ticket: refunds endpoint",
    },
  ],
  create: "Create ticket",
  create_icon: "plus",
};

describe("the sections' interaction state", () => {
  it("the Stack opens by itself when there are links; a toggle wins", () => {
    expect(stackOpen(null, 2)).toBe(true);
    expect(stackOpen(false, 2)).toBe(false);
    expect(stackOpen(true, 0)).toBe(false);
  });

  it("Move to epic picks the first epic until one is clicked; a refusal blocks it", () => {
    expect(pickedChoice(MOVE, null)?.key).toBe("SHOP-40");
    expect(pickedChoice(MOVE, "SHOP-60")?.new_key).toBe("SHOP-60.2");
    expect(canMove(MOVE, null)).toBe(true);
    expect(canMove({ ...MOVE, refusal: "SHOP-51 can’t move" }, null)).toBe(false);
    expect(canMove({ ...MOVE, choices: [] }, null)).toBe(false);
  });

  it("the panel follows a moved ticket to its new key", () => {
    expect(followMove("SHOP-51", "SHOP-51", "SHOP-40.4")).toBe("SHOP-40.4");
    expect(followMove("SHOP-12", "SHOP-51", "SHOP-40.4")).toBeNull();
  });

  it("the progress bar's fill", () => {
    expect(progressPercent(1, 4)).toBe(25);
    expect(progressPercent(0, 0)).toBe(0);
  });
});

describe("the sections", () => {
  it("Stack: rows with their waiting text, the open ticket marked", () => {
    const out = html(createElement(StackBody, { ticket: "SHOP-20", view: STACK }));
    for (const text of [
      "Dependencies",
      "2 tickets",
      "Checkout API",
      "waits for SHOP-18 to merge",
      "+ Add dependency",
    ]) {
      expect(out).toContain(text);
    }
    expect(out).toContain('aria-current="true"');
    const none = html(
      createElement(StackBody, {
        ticket: "SHOP-12",
        view: { ...STACK, summary: "No dependencies · nothing waits on SHOP-12", rows: [] },
      }),
    );
    expect(none).toContain("No dependencies · nothing waits on SHOP-12");
    expect(none).not.toContain("stack-row");
  });

  it("Messages: the question, its answer and an Answer button on an inbox question", () => {
    const view: MessagesView = {
      title: "Messages",
      heading: "1 thread · linked tickets only",
      you: "you",
      answer: "Answer",
      placeholder: "Type your answer",
      send: "Send",
      threads: [
        {
          id: "m1",
          answerable: true,
          question: {
            from: "SHOP-40.2",
            to: "SHOP-40.1",
            from_name: null,
            to_name: null,
            time: "10:42",
            text: "Which endpoint returns the wishlist?",
            status: "Sent to your inbox",
            icon: "inbox",
            tone: "attention",
          },
          answer: {
            from: null,
            to: "SHOP-40.2",
            from_name: null,
            to_name: null,
            time: "10:43",
            text: "GET /api/wishlist",
            status: "Answered by you · 1 min",
            icon: "check-check",
            tone: "done",
          },
        },
      ],
    };
    const out = html(createElement(MessagesBody, { view }));
    for (const text of [
      "1 thread · linked tickets only",
      "SHOP-40.1",
      "GET /api/wishlist",
      ">you<",
      "Answer",
    ]) {
      expect(out).toContain(text);
    }
    expect(out).toContain('data-tone="attention"');
  });

  it("Messages with the epic lead: its name at the other end (T-0290)", () => {
    const view: MessagesView = {
      title: "Messages",
      heading: "1 thread · with Wishlist lead",
      you: "you",
      answer: "Answer",
      placeholder: "Type your answer",
      send: "Send",
      threads: [
        {
          id: "q7",
          answerable: false,
          question: {
            from: "SHOP-40.2",
            to: null,
            from_name: null,
            to_name: "Wishlist lead",
            time: "11:05",
            text: "Should the button show a filled heart once the item is saved?",
            status: "Routed to the lead · no inbox item",
            icon: "send",
            tone: "delivered",
          },
          answer: {
            from: null,
            to: "SHOP-40.2",
            from_name: "Wishlist lead",
            to_name: null,
            time: "11:06",
            text: "Yes: filled heart means saved, outline means not saved (plan, step 2).",
            status: "Answered by Wishlist lead · typed into SHOP-40.2’s session",
            icon: "check-check",
            tone: "done",
          },
        },
      ],
    };
    const out = html(createElement(MessagesBody, { view }));
    expect(out).toContain("1 thread · with Wishlist lead");
    expect(out).toContain('<span class="message-name">Wishlist lead</span>');
    expect(out).toContain("Answered by Wishlist lead · typed into SHOP-40.2’s session");
    expect(out).not.toContain(">you<");
    expect(out).not.toContain(">Answer<");
  });

  it("History: sessions with their copyable Claude session ID, events without", () => {
    const view: HistoryView = {
      columns: ["EVENT", "WHEN", "DURATION", "TOKENS"],
      copy_hint: "Copy session ID",
      copied: "Copied",
      empty: "Nothing has happened to this ticket yet.",
      total: null,
      key: "SHOP-28",
      rows: [
        {
          title: "Agent session",
          detail: "claude 9f1c-77 · exited",
          when: "09:42",
          duration: "38 min",
          tokens: "74k",
          tool_session_id: "9f1c-77",
          is_session: true,
          icon: "square-terminal",
          palette: null,
          mark: null,
          opens: null,
        },
        {
          title: "Created",
          detail: "by you",
          when: "09:10",
          duration: "—",
          tokens: "—",
          tool_session_id: null,
          is_session: false,
          icon: "history",
          palette: null,
          mark: null,
          opens: null,
        },
      ],
    };
    const out = html(createElement(HistoryBody, { view }));
    for (const text of ["EVENT", "DURATION", "Agent session", "claude 9f1c-77", "38 min", "Created"]) {
      expect(out).toContain(text);
    }
    expect(out.match(/Copy session ID/g)).toHaveLength(1);
    expect(html(createElement(HistoryBody, { view: { ...view, rows: [] } }))).toContain(view.empty);
  });

  it("History: validation rounds open nothing any more; the total sits under the rows", () => {
    const view: HistoryView = {
      columns: ["EVENT", "WHEN", "DURATION", "TOKENS"],
      copy_hint: "Copy session ID",
      copied: "Copied",
      empty: "Nothing has happened to this ticket yet.",
      total: { label: "Validation total · 2 entries", tokens: "15k" },
      key: "SHOP-32",
      rows: [
        {
          title: "Round 1 · failed",
          detail: "agent submitted",
          when: "10:02",
          duration: "4 min",
          tokens: "12k",
          tool_session_id: null,
          is_session: false,
          icon: "shield-x",
          palette: "attn",
          mark: null,
          opens: null,
        },
      ],
    };
    const out = html(createElement(HistoryBody, { view }));
    expect(out).toContain("Round 1 · failed");
    expect(out).not.toContain("aria-expanded");
    expect(out).toContain("Validation total · 2 entries");
    expect(out).toContain("15k");
  });

  it("the Diff tab's report draws its findings, checks and notes, with Create ticket", () => {
    const out = html(createElement(Report, { report: REPORT, ticket: "SHOP-32" }));
    expect(out).toContain("arrow keys do nothing");
    expect(out).toContain("components/AddressAutocomplete.tsx:88");
    expect(out).toContain("Create ticket");
    expect(out).toContain('class="report-key"');
  });

  it("History: description and comment lines open their before / after and a report (s05-45, CMT-14)", () => {
    const row = (over: Partial<HistoryRowView>): HistoryRowView => ({
      title: "",
      detail: null,
      when: "2:14 pm",
      duration: "—",
      tokens: "—",
      tool_session_id: null,
      is_session: false,
      icon: "history",
      palette: null,
      mark: { kind: "agent" },
      opens: null,
      ...over,
    });
    const view: HistoryView = {
      columns: ["EVENT", "WHEN", "DURATION", "TOKENS"],
      copy_hint: "Copy session ID",
      copied: "Copied",
      empty: "Nothing has happened to this ticket yet.",
      total: null,
      key: "SHOP-34",
      rows: [
        row({
          title: "Griller · Grill edited the description · added Agreed (4 lines)",
          opens: {
            label: "DESCRIPTION",
            caption: "Before / after",
            lines: [
              { kind: "same", sign: "", text: "Admins can export the orders table as CSV." },
              { kind: "added", sign: "+", text: "## Agreed" },
            ],
            report: null,
          },
        }),
        row({
          title: "Validator · Validate · round 2 commented · with a report (48 KB)",
          detail: "“Fail: the 10,000 row limit isn't…”",
          opens: {
            label: "COMMENT",
            caption: null,
            lines: [{ kind: "same", sign: "", text: "Fail: the 10,000 row limit isn't enforced." }],
            report: { n: 4, chip: { label: "Report", size: "· 48 KB", open: "Open" } },
          },
        }),
        row({ title: "You deleted a comment", mark: { kind: "you" } }),
      ],
    };
    const closed = html(createElement(HistoryBody, { view }));
    expect(closed.match(/aria-expanded="false"/g)).toHaveLength(2);
    expect(closed).toContain("comment-mark");
    expect(closed).not.toContain("DESCRIPTION");
    const opened = html(createElement(HistoryBody, { view, opened: [0, 1] }));
    expect(opened).toContain("DESCRIPTION");
    expect(opened).toContain("Before / after");
    expect(opened).toContain('data-kind="added"');
    expect(opened).toContain("## Agreed");
    expect(opened).toContain("· 48 KB");
    expect(opened).toContain(">Open</button>");
  });

  it("the epic panel: progress, branch, children and Add ticket to epic", () => {
    const view: EpicView = {
      external_id: "PAY-123",
      progress: "1 of 3 merged",
      merged: 1,
      total: 3,
      branch: "otaq/SHOP-40 · local",
      heading: "TICKETS · 3",
      children: [
        { key: "SHOP-40.1", title: "Wishlist API", deps: null, pill: pill("Merged"), finished: true },
        {
          key: "SHOP-40.2",
          title: "Wishlist button",
          deps: "⛓ SHOP-40.1",
          pill: pill("Working"),
          finished: false,
        },
      ],
      add: "Add ticket to epic",
      next_key: "· new tickets get the next key, SHOP-40.4",
      note: "When every child is merged…",
    };
    const out = html(createElement(EpicBody, { ticket: "SHOP-40", view }));
    for (const text of [
      "PAY-123",
      "1 of 3 merged",
      "TICKETS · 3",
      "⛓ SHOP-40.1",
      "Add ticket to epic",
      "SHOP-40.4",
    ]) {
      expect(out).toContain(text);
    }
    expect(out).toContain('aria-valuenow="1"');
  });

  it("the epic dialogs: the new key, the base note, the agents and what stays merged", () => {
    const move = html(
      createElement(MoveDialogBody, { view: MOVE, picked: "SHOP-60", onPick: () => undefined, error: null }),
    );
    for (const text of [
      "SHOP-60.2",
      "the old key SHOP-51 still works",
      "instead of release/2.3",
      MOVE.disk_note,
    ]) {
      expect(move).toContain(text);
    }
    const refused = html(
      createElement(MoveDialogBody, {
        view: { ...MOVE, refusal: "SHOP-51 can’t move" },
        picked: null,
        onPick: () => undefined,
        error: null,
      }),
    );
    expect(refused).toContain("SHOP-51 can’t move");

    const close: CloseEpicDialogView = {
      title: "Close epic SHOP-40 Wishlist?",
      agents: ["SHOP-40.2 Wishlist button — its agent is running."],
      also: "Also closes SHOP-40.3 Wishlist page (Backlog).",
      note: "Branches are kept.",
      confirm: "Stop 1 agent & close 2 tickets",
    };
    const out = html(createElement(CloseEpicBody, { view: close, error: null }));
    for (const text of [close.agents[0] ?? "", close.also ?? "", close.note]) expect(out).toContain(text);
  });

  it("registers the sections, the History tab and the epic dialogs from its folder", () => {
    expect(registered(registerScreens({ sections }))).toMatchObject({
      panelSections: ["epic", "stack", "messages"],
      panelTabs: ["history"],
      overlays: ["move_to_epic", "close_epic"],
    });
  });
});
