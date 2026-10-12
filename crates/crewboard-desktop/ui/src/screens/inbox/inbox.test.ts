import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createOverlayStack } from "../../app/overlays";
import { createRegistries, registered, registerScreens, RegistriesContext } from "../../app/registry";
import { createSelection, type TerminalFocusRequest } from "../../app/selection";
import { createStore } from "../../app/store";
import type { InboxOption } from "../../generated/InboxOption";
import type { InboxRowView } from "../../generated/InboxRowView";
import type { InboxView } from "../../generated/InboxView";
import { inbox } from ".";
import { AllClearBody } from "./AllClear";
import { InboxBody } from "./Inbox";
import {
  canSend,
  describeAction,
  initialPick,
  landNext,
  openItem,
  opensNote,
  type OpenRoutes,
  pickByKey,
  picksOther,
  step,
  waitingRows,
} from "./logic";
import { ResumeAllBody } from "./ResumeAll";
import { RunBarBody } from "./RunBar";

const OPTIONS: InboxOption[] = [
  { number: 1, label: "Postmark", recommended: true, picked: true, other: false },
  { number: 2, label: "SES", recommended: false, picked: false, other: false },
  { number: 3, label: "Other… type your own answer", recommended: false, picked: false, other: true },
];

function row(key: string, extra: Partial<InboxRowView> = {}): InboxRowView {
  return {
    key,
    project: "SHOP",
    project_name: "shop-web",
    title: `${key} title`,
    reason: "asks: which provider?",
    age: "6 min",
    blocks: null,
    pill: { palette: "attn", icon: "message-circle-question-mark", label: "Asks you" },
    restart_note: null,
    sent_note: null,
    resolved: false,
    tab: "terminal",
    answer: null,
    buttons: [],
    hint: null,
    note: null,
    ...extra,
  };
}

const ANSWER = {
  open: "Answer",
  open_icon: "corner-down-left" as const,
  send: "Send answer",
  send_icon: "corner-down-left" as const,
  question: "q1",
  message: null,
  heading: "OPTIONS FROM SHOP-21’S AGENT",
  options: OPTIONS,
  recommended: "Recommended",
  placeholder: "Type your answer",
  logged: "Logged in SHOP-21 Messages",
  cancel: "Cancel",
};

const VIEW: InboxView = {
  title: "Needs you",
  total: 3,
  scope: "· all projects",
  next: { keys: "⌘J", label: "Next" },
  limit: null,
  groups: [
    {
      id: "orange",
      title: "ASKS YOU · NEEDS YOU · IDLE",
      count: 1,
      resume_all: null,
      rows: [
        row("SHOP-22", { resolved: true, reason: "resolved elsewhere · just now" }),
        row("SHOP-21", { answer: ANSWER }),
      ],
    },
    {
      id: "stopped",
      title: "INTERRUPTED & PARENT CLOSED",
      count: 2,
      resume_all: "Resume all (1)",
      rows: [
        row("SHOP-30", {
          buttons: [
            {
              action: { kind: "reopen_parent", parent: "SHOP-26" },
              label: "Reopen parent",
              icon: "rotate-ccw",
              primary: true,
            },
            {
              action: { kind: "remove_dependency", parent: "SHOP-26" },
              label: "Remove dependency",
              icon: "unlink",
              primary: false,
            },
          ],
        }),
        row("SHOP-18", { tab: "diff" }),
      ],
    },
  ],
  empty: null,
  foot: [
    { keys: "↑↓", label: "move" },
    { keys: "↵", label: "open" },
    { keys: "⌘J", label: "next item" },
  ],
};

function render(node: ReturnType<typeof createElement>) {
  return renderToStaticMarkup(createElement(RegistriesContext, { value: createRegistries() }, node));
}

function routes() {
  return {
    selection: createSelection(),
    focus: createStore<TerminalFocusRequest | null>(null),
    overlays: createOverlayStack(),
  } satisfies OpenRoutes;
}

describe("the inbox's own rules", () => {
  it("↑↓ walk the waiting rows only, inside the list", () => {
    expect(waitingRows(VIEW).map((r) => r.key)).toEqual(["SHOP-21", "SHOP-30", "SHOP-18"]);
    expect(step(0, -1, 3)).toBe(0);
    expect(step(1, 1, 3)).toBe(2);
    expect(step(2, 1, 3)).toBe(2);
    expect(step(5, 1, 0)).toBe(0);
  });

  it("the answer box picks the recommended option, 1–4 pick, Other… needs text", () => {
    expect(initialPick(OPTIONS)).toBe(1);
    expect(initialPick(OPTIONS.map((o) => ({ ...o, picked: false })))).toBeNull();
    expect(pickByKey("2", OPTIONS)).toBe(2);
    expect(pickByKey("4", OPTIONS)).toBeNull();
    expect(picksOther(3, OPTIONS)).toBe(true);
    expect(canSend(1, "", OPTIONS)).toBe(true);
    expect(canSend(3, " ", OPTIONS)).toBe(false);
    expect(canSend(3, "Mailgun", OPTIONS)).toBe(true);
    expect(canSend(null, "x", OPTIONS)).toBe(false);
    expect(canSend(null, "Only the form", [])).toBe(true);
    expect(canSend(null, "  ", [])).toBe(false);
  });

  it("a row opens its ticket on its tab, the terminal focused, and the inbox closes", () => {
    const r = routes();
    r.overlays.open("inbox");
    openItem("BILL", "BILL-5", "diff", r);
    expect(r.selection.get()).toMatchObject({ project: "BILL", ticket: "BILL-5", tab: "diff" });
    expect(r.focus.get()).toBeNull();
    expect(r.overlays.store.get()).toEqual([]);
    openItem("SHOP", "SHOP-22", "terminal", r);
    expect(r.focus.get()).toEqual({ ticket: "SHOP-22", seq: 1 });
  });

  it("⌘J lands on the next item, or shows All clear and closes the panel", () => {
    const r = routes();
    const changes = createStore(0);
    landNext({ kind: "open", project: "SHOP", key: "SHOP-23", tab: "terminal" }, r, changes);
    expect(r.selection.get().ticket).toBe("SHOP-23");
    expect(r.focus.get()?.ticket).toBe("SHOP-23");
    landNext(
      { kind: "all_clear", title: "All clear", text: "7 items…", segments: 7, shown_ms: 3000 },
      r,
      changes,
    );
    expect(r.selection.get().ticket).toBeNull();
    const open = r.overlays.store.get();
    expect(open.map((o) => o.id)).toEqual(["all_clear"]);
    expect(changes.get()).toBe(2);
  });

  it("names each row action", () => {
    expect(describeAction({ kind: "retry_start" })).toBe("retry_start");
    expect(describeAction({ kind: "reopen_parent", parent: "SHOP-26" })).toBe("reopen_parent SHOP-26");
    expect(describeAction({ kind: "resume_target", message: "m1" })).toBe("resume_target m1");
    expect(describeAction({ kind: "retry_merge" })).toBe("retry_merge");
    expect(opensNote({ kind: "answer_agent", note: "" })).toBe(true);
    expect(opensNote({ kind: "move_to_review_anyway" })).toBe(false);
  });
});

/** *10 → New item types · group 1 (M12)*, as Rust words it. */
const M12: InboxView = {
  ...VIEW,
  total: 2,
  groups: [
    {
      id: "orange",
      title: "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · PROJECT AGENTS",
      count: 2,
      resume_all: null,
      rows: [
        row("SHOP-33", {
          reason: "validator permission: Allow Bash(pnpm test)?",
          hint: { keys: "↵ / ⌘J", label: "open Terminal · Validator side" },
        }),
        row("SHOP-32", {
          reason: "validation failed 3 times",
          buttons: [
            {
              action: { kind: "move_to_review_anyway" },
              label: "Move to Review anyway",
              icon: "eye",
              primary: true,
            },
            {
              action: { kind: "another_round" },
              label: "Give it another round",
              icon: "rotate-ccw",
              primary: false,
            },
            {
              action: { kind: "answer_agent", note: "" },
              label: "Answer the agent…",
              icon: "message-square",
              primary: false,
            },
          ],
          note: {
            placeholder: "Tell the agent what to do",
            send: "Send to the agent",
            send_icon: "corner-down-left",
            cancel: "Cancel",
            text: "Goes with the findings · the round count starts over",
          },
        }),
      ],
    },
  ],
};

describe("the M12 items", () => {
  it("draw the new heading, the validator's hint and the inline actions", () => {
    const html = render(
      createElement(InboxBody, {
        view: M12,
        selected: 0,
        draft: null,
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("ASKS YOU · NEEDS YOU · IDLE · VALIDATION · PROJECT AGENTS");
    expect(html).toContain("↵ / ⌘J open Terminal · Validator side");
    expect(html).toContain("Move to Review anyway");
    expect(html).toContain("Give it another round");
    expect(html).toContain("Answer the agent…");
    expect(html).not.toContain("Tell the agent what to do");
  });

  it("opens Answer the agent…'s note box in its row", () => {
    const html = render(
      createElement(InboxBody, {
        view: M12,
        selected: 1,
        draft: { key: "SHOP-32", pick: null, text: "", sending: false, note: true },
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("Tell the agent what to do");
    expect(html).toContain("Send to the agent");
    expect(html).not.toContain("Give it another round");
  });
});

describe("the inbox's markup", () => {
  it("registers the inbox, Resume all, All clear, the run bar and the two shortcuts", () => {
    const r = registered(registerScreens({ inbox }));
    expect(r.overlays).toEqual(["inbox", "resume_all", "all_clear"]);
    expect(r.panelSections).toEqual(["inbox_run"]);
    expect(r.shortcuts).toEqual(["next_inbox_item", "open_inbox"]);
  });

  it("draws the head, groups, rows, buttons and foot", () => {
    const html = render(
      createElement(InboxBody, {
        view: VIEW,
        selected: 0,
        draft: null,
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    for (const text of [
      "Needs you",
      "· all projects",
      "ASKS YOU · NEEDS YOU · IDLE",
      "Resume all (1)",
      "shop-web",
      "Reopen parent",
      "Remove dependency",
      "Answer",
      "next item",
    ]) {
      expect(html).toContain(text);
    }
    expect(html).toContain('data-resolved="true"');
    expect(html.match(/data-selected/g)).toHaveLength(1);
  });

  it("opens the answer box with the options and the recommended one picked", () => {
    const html = render(
      createElement(InboxBody, {
        view: VIEW,
        selected: 0,
        draft: { key: "SHOP-21", pick: 1, text: "", sending: false },
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("OPTIONS FROM SHOP-21’S AGENT");
    expect(html).toContain("Recommended");
    expect(html).toContain("Logged in SHOP-21 Messages");
    expect(html).toContain("Send answer");
    expect(html).toContain('aria-checked="true"');
  });

  it("an empty inbox and the usage-limit row", () => {
    const html = render(
      createElement(InboxBody, {
        view: {
          ...VIEW,
          total: 0,
          next: null,
          groups: [],
          foot: [],
          limit: { text: "Usage limit reached · 2 agents waiting", resume: "Resume all (2)" },
          empty: { title: "Nothing needs you", text: "3 agents running." },
        },
        selected: 0,
        draft: null,
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("Nothing needs you");
    expect(html).toContain("Usage limit reached · 2 agents waiting");
    expect(html).toContain("Resume all (2)");
    expect(html).not.toContain("next item");
  });

  it("the run bar, All clear and Resume all's ticket list", () => {
    const bar = render(
      createElement(RunBarBody, {
        bar: {
          tone: "step",
          title: "Inbox · 1 of 7",
          segments: ["current", "waiting"],
          next: { keys: "⌘J", label: "Next" },
          exit: "Exit",
        },
      }),
    );
    expect(bar).toContain("Inbox · 1 of 7");
    expect(bar).toContain('data-segment="current"');
    expect(bar).toContain("Exit");
    const clear = render(
      createElement(AllClearBody, {
        clear: { title: "All clear", text: "7 items handled", segments: 3, shown_ms: 3000 },
      }),
    );
    expect(clear).toContain("All clear");
    const dialog = render(
      createElement(ResumeAllBody, {
        dialog: {
          title: "Resume 1 interrupted session?",
          text: "Each session continues in its own worktree.",
          included: [{ key: "SHOP-25", title: "Upgrade to Next.js 15" }],
          not_included: "Not included: SHOP-28 Cart badge count.",
          cancel: "Cancel",
          cancel_hint: "cancel",
          confirm: "Resume 1",
        },
        close: () => undefined,
      }),
    );
    expect(dialog).toContain("SHOP-25");
    expect(dialog).toContain("Not included: SHOP-28");
    expect(dialog).toContain("Resume 1");
  });
});
