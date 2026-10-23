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
import { INITIAL_LAYOUT, type ShellLayout } from "../../app/layout";
import { type PaneFocusRequest, type ShownAgents } from "../pane/logic";
import { inbox } from ".";
import { AllClearBody } from "./AllClear";
import { InboxBody } from "./Inbox";
import {
  canSend,
  openAgent,
  describeAction,
  initialPick,
  landNext,
  openItem,
  openRow,
  opensNote,
  type OpenRoutes,
  pickByKey,
  picksOther,
  step,
  confirmSuggestion,
  suggestionOverlay,
  waitingRows,
} from "./logic";
import { ResumeAllBody } from "./ResumeAll";
import { RunBarBody } from "./RunBar";

const OPTIONS: InboxOption[] = [
  {
    number: 1,
    label: "Postmark",
    recommended: true,
    picked: true,
    other: false,
    note: null,
    lead_reason: null,
  },
  { number: 2, label: "SES", recommended: false, picked: false, other: false, note: null, lead_reason: null },
  {
    number: 3,
    label: "Other… type your own answer",
    recommended: false,
    picked: false,
    other: true,
    note: null,
    lead_reason: null,
  },
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
    agent: null,
    keyed: true,
    via: null,
    lead_note: null,
    lead: null,
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
  title: "Inbox",
  total: 3,
  scope: "· all projects",
  next: { keys: "⌘J", label: "Next" },
  limits: [],
  groups: [
    {
      id: "family",
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
  const selection = createSelection();
  return {
    selection,
    focus: createStore<TerminalFocusRequest | null>(null),
    overlays: createOverlayStack(),
    pane: {
      selection,
      layout: createStore<ShellLayout>({ ...INITIAL_LAYOUT, agentOpen: false }),
      shown: createStore<ShownAgents>({}),
      focus: createStore<PaneFocusRequest | null>(null),
    },
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

  it("Open agent… opens Settings → Agents on the agent whose model was refused (AGT-05, AGM-03)", () => {
    const r = routes();
    r.overlays.open("inbox");
    openAgent("fable", r);
    expect(r.overlays.store.get()).toEqual([
      { id: "settings", params: { section: "agents", agent: "fable" } },
    ]);
    openAgent(null, r);
    expect(r.overlays.store.get()).toEqual([{ id: "settings", params: { section: "agents" } }]);
    expect(describeAction({ kind: "open_agent", agent: "fable" })).toBe("open_agent fable");
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
    expect(opensNote({ kind: "move_on_anyway" })).toBe(false);
  });
});

/** *10 → New item types · group 1 (M12)*, as Rust words it. */
const M12: InboxView = {
  ...VIEW,
  total: 2,
  groups: [
    {
      id: "family",
      title: "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
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
              action: { kind: "move_on_anyway" },
              label: "Move on anyway",
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
    expect(html).toContain("ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS");
    expect(html).toContain("↵ / ⌘J open Terminal · Validator side");
    expect(html).toContain("Move on anyway");
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

/** *10 → New item types · group 1 (M12)*: the Project agent "Import GitHub issues" waits with a question. */
const AGENT = row("a1", {
  title: "Project agent “Import GitHub issues”",
  reason: "asks: Import closed issues too?",
  age: "3 min",
  pill: { palette: "attn", icon: "bot", label: "Asks you" },
  hint: { keys: "", label: "opens that Project agent" },
  answer: {
    ...ANSWER,
    question: "q7",
    heading: "OPTIONS FROM THE PROJECT AGENT",
    logged: "Typed into the agent’s terminal",
  },
  agent: { id: "a1" },
  keyed: false,
});

const AGENT_VIEW: InboxView = {
  ...VIEW,
  groups: [
    {
      id: "family",
      title: "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
      count: 1,
      resume_all: null,
      rows: [AGENT],
    },
  ],
};

describe("Project agent items", () => {
  it("name the project and the agent, no ticket key, and show that agent in the pane, its terminal focused", () => {
    const view = AGENT_VIEW;
    const html = render(
      createElement(InboxBody, {
        view,
        selected: 0,
        draft: null,
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("Project agent “Import GitHub issues”");
    expect(html).not.toContain('class="inbox-key"');
    expect(html).toContain(">opens that Project agent<");
    expect(html).toContain("Answer");

    const r = routes();
    r.overlays.open("inbox");
    openRow(AGENT, r);
    expect(r.selection.get().project).toBe("SHOP");
    expect(r.selection.get().ticket).toBeNull();
    expect(r.overlays.store.get()).toEqual([]);
    expect(r.pane.shown.get()).toEqual({ SHOP: "a1" });
    expect(r.pane.layout.get().agentOpen).toBe(true);
    expect(r.pane.focus.get()).toEqual({ seq: 1, agent: "a1" });
    expect(r.focus.get()).toBeNull();
  });

  it("answer inline with the agent's options", () => {
    const view = AGENT_VIEW;
    const html = render(
      createElement(InboxBody, {
        view,
        selected: 0,
        draft: { key: "a1", pick: 1, text: "", sending: false },
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain("OPTIONS FROM THE PROJECT AGENT");
    expect(html).toContain("Typed into the agent’s terminal");
  });

  it("a ticket row still opens its ticket", () => {
    const r = routes();
    openRow(row("SHOP-22"), r);
    expect(r.selection.get()).toMatchObject({ project: "SHOP", ticket: "SHOP-22", tab: "terminal" });
    expect(r.pane.focus.get()).toBeNull();
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
      "Inbox",
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

  it("draws each option's note whole under its label, as the option's description (ATT-17)", () => {
    const long = "Pros and cons, ".repeat(40).trim();
    expect(long.length).toBeGreaterThan(590);
    const notes = ["Opens in any editor; one file per note, no attachments.", long];
    const options: InboxOption[] = OPTIONS.map((o) => ({ ...o, note: notes[o.number - 1] ?? null }));
    const html = render(
      createElement(InboxBody, {
        view: {
          ...VIEW,
          groups: VIEW.groups
            .slice(0, 1)
            .map((g) => ({ ...g, rows: [row("SHOP-21", { answer: { ...ANSWER, options } })] })),
        },
        selected: 0,
        draft: { key: "SHOP-21", pick: 1, text: "", sending: false },
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toContain(">Opens in any editor; one file per note, no attachments.</span>");
    expect(html).toContain(`>${long}</span>`);
    expect(html.match(/class="inbox-option-note"/g)).toHaveLength(2);
    // Each note is its option's description: the button points at the note's id.
    const described = [...html.matchAll(/aria-describedby="([^"]+)"/g)].map((m) => m[1]);
    expect(described).toHaveLength(2);
    for (const id of described) expect(html).toContain(`id="${id ?? ""}" class="inbox-option-note"`);
    expect(html).not.toContain("…</span>");
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
          limits: [
            {
              tool: "claude_code",
              resume_tool: "claude",
              avatar: null,
              text: "Usage limit reached · 2 agents waiting",
              resume: "Resume (2)",
              button: null,
            },
          ],
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
    expect(html).toContain("Resume (2)");
    expect(html).not.toContain("next item");
  });

  it("a tool's limit row with several tools: its avatar, and Sign in… on a tool that isn't available", () => {
    const html = render(
      createElement(InboxBody, {
        view: {
          ...VIEW,
          limits: [
            {
              tool: "codex",
              resume_tool: "codex",
              avatar: {
                tool: "codex",
                letters: "CX",
                background: "tool-cx",
                foreground: "tool-cx-fg",
                hover: "Codex",
              },
              text: "Codex isn’t available · signed out · 5 tickets waiting",
              resume: null,
              button: {
                label: "Sign in…",
                icon: "plug",
                action: { kind: "sign_in", tool: "codex" },
                enabled: true,
                primary: false,
                tooltip: null,
              },
            },
          ],
        },
        selected: 0,
        draft: null,
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    expect(html).toMatch(/inbox-limit.*tool-avatar.*CX.*Codex isn’t available.*Sign in…/);
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

/** *10 → Row · Epic lead items (0083)*, as Rust words it: the four items. */
const PASSED_ON = row("SHOP-40.2", {
  title: "Wishlist button",
  via: "asks · passed on by Wishlist lead",
  reason: "Can guests save items without logging in?",
  age: "2 min",
  hint: { keys: "", label: "1–3 pick · ↵ sends · ⌘J opens SHOP-40.2 with its terminal focused" },
  answer: {
    ...ANSWER,
    question: "q8",
    heading: "OPTIONS FROM SHOP-40.2’S AGENT",
    recommended: "Recommended by the lead",
    options: [
      {
        number: 1,
        label: "Logged-in users only",
        recommended: true,
        picked: true,
        other: false,
        note: null,
        lead_reason: "PAY-123 only mentions signed-in users.",
      },
      {
        number: 2,
        label: "Guests too, kept in the browser",
        recommended: false,
        picked: false,
        other: false,
        note: null,
        lead_reason: null,
      },
      {
        number: 3,
        label: "Other… type your own answer",
        recommended: false,
        picked: false,
        other: true,
        note: null,
        lead_reason: null,
      },
    ],
  },
});

const STUCK = row("SHOP-40.3", {
  title: "Wishlist page",
  reason: "validation failed 3 times · “orders.test.ts expects list”",
  pill: { palette: "attn", icon: "shield-x", label: "Validation failed" },
  hint: { keys: "", label: "the lead isn’t woken a second time" },
  lead_note: {
    heading: "LEAD'S NOTE · SENT AFTER THE FIRST 3 FAILURES",
    text: "“The API returns items[] (SHOP-40.1). Fix the test, don't change the API.”",
    view_heading: "LEAD'S VIEW",
    view: "The agent keeps changing the API instead of the test.",
  },
  buttons: [
    { action: { kind: "move_on_anyway" }, label: "Move on anyway", icon: "eye", primary: true },
    { action: { kind: "another_round" }, label: "Give it another round", icon: "rotate-ccw", primary: false },
  ],
});

const LEAD = row("pa7", {
  title: "Wishlist lead · SHOP-40 Wishlist",
  reason: "asks: Should the share link (40.4) expire? PAY-123 doesn't say.",
  pill: { palette: "attn", icon: "bot", label: "Asks you" },
  hint: { keys: "", label: "opens the epic panel on its Lead tab" },
  agent: { id: "pa7" },
  lead: { epic: "SHOP-40" },
  keyed: false,
});

const CLOSE_ACTION = {
  kind: "confirm_suggestion" as const,
  id: "sg1",
  dialog: "close" as const,
  line: "Suggested by Wishlist lead: “PAY-123 doesn't ask for an export.”",
};

const SUGGESTION = row("SHOP-40.6", {
  title: "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)",
  reason: "PAY-123 doesn't ask for an export.",
  pill: { palette: "epic", icon: "circle-x", label: "Backlog" },
  tab: "ticket",
  hint: { keys: "", label: "blocks no agent · SHOP-40.6 never started" },
  keyed: false,
  buttons: [
    { action: CLOSE_ACTION, label: "Close…", icon: "circle-x", primary: true },
    { action: { kind: "dismiss_suggestion", id: "sg1" }, label: "Dismiss", icon: "x", primary: false },
  ],
});

const LEAD_VIEW: InboxView = {
  ...VIEW,
  total: 4,
  groups: [
    {
      id: "family",
      title: "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
      count: 3,
      resume_all: null,
      rows: [PASSED_ON, STUCK, LEAD],
    },
    {
      id: "suggestions",
      title: "SUGGESTIONS · NO OS NOTIFICATION",
      count: 1,
      resume_all: null,
      rows: [SUGGESTION],
    },
  ],
};

function renderLead(draft: Parameters<typeof InboxBody>[0]["draft"] = null) {
  return render(
    createElement(InboxBody, {
      view: LEAD_VIEW,
      selected: 0,
      draft,
      onDraft: () => undefined,
      onSelect: () => undefined,
    }),
  );
}

describe("epic lead items", () => {
  it("draw the four kinds: passed on, second stop, the lead's own question, the suggestion", () => {
    const html = renderLead();
    for (const text of [
      "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS",
      "asks · passed on by Wishlist lead",
      "Can guests save items without logging in?",
      "1–3 pick · ↵ sends · ⌘J opens SHOP-40.2 with its terminal focused",
      "LEAD&#x27;S NOTE · SENT AFTER THE FIRST 3 FAILURES",
      "LEAD&#x27;S VIEW",
      "The agent keeps changing the API instead of the test.",
      "the lead isn’t woken a second time",
      "Move on anyway",
      "Wishlist lead · SHOP-40 Wishlist",
      "opens the epic panel on its Lead tab",
      "SUGGESTIONS · NO OS NOTIFICATION",
      "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)",
      "PAY-123 doesn&#x27;t ask for an export.",
      "Close…",
      "Dismiss",
      "blocks no agent · SHOP-40.6 never started",
    ]) {
      expect(html).toContain(text);
    }
    // Only the two ticket rows show their key; the lead's and the suggestion's titles name theirs.
    expect(html.match(/class="inbox-key"/g)).toHaveLength(2);
  });

  it("the passed-on answer box marks the lead's option with its reason", () => {
    const html = renderLead({ key: "SHOP-40.2", pick: 1, text: "", sending: false });
    expect(html).toContain("Recommended by the lead");
    expect(html).toContain('class="inbox-option-note">PAY-123 only mentions signed-in users.<');
    expect(html).toContain('aria-checked="true"');
  });

  it("a passed-on option shows its own note first, then the lead's reason (ATT-17)", () => {
    const answer = PASSED_ON.answer;
    if (answer === null) throw new Error("PASSED_ON has an answer");
    const options = answer.options.map((o) =>
      o.number === 1 ? { ...o, note: "Fewer accounts to support; guests can't save." } : o,
    );
    const html = render(
      createElement(InboxBody, {
        view: {
          ...LEAD_VIEW,
          groups: LEAD_VIEW.groups
            .slice(0, 1)
            .map((g) => ({ ...g, rows: [{ ...PASSED_ON, answer: { ...answer, options } }] })),
        },
        selected: 0,
        draft: { key: "SHOP-40.2", pick: 1, text: "", sending: false },
        onDraft: () => undefined,
        onSelect: () => undefined,
      }),
    );
    const note = html.indexOf("Fewer accounts to support; guests can&#x27;t save.");
    const reason = html.indexOf("PAY-123 only mentions signed-in users.");
    expect(note).toBeGreaterThan(-1);
    expect(reason).toBeGreaterThan(note);
    expect(html).toContain("Recommended by the lead");
  });

  it("the lead's item opens the epic panel on its Lead tab, the lead's terminal focused", () => {
    const r = routes();
    r.overlays.open("inbox");
    openRow(LEAD, r);
    expect(r.selection.get()).toMatchObject({ project: "SHOP", ticket: "SHOP-40", tab: "lead" });
    expect(r.focus.get()).toEqual({ ticket: "SHOP-40", seq: 1 });
    expect(r.overlays.store.get()).toEqual([]);
    expect(r.pane.focus.get()).toBeNull();
    // ⌘J lands there the same way.
    const again = routes();
    landNext({ kind: "open", project: "SHOP", key: "SHOP-40", tab: "lead" }, again, createStore(0));
    expect(again.focus.get()?.ticket).toBe("SHOP-40");
  });

  it("Close… opens the usual confirmation with the suggestion; its confirm goes through inbox.act", async () => {
    expect(suggestionOverlay(SUGGESTION, CLOSE_ACTION)).toEqual([
      "close_ticket",
      { ticket: "SHOP-40.6", suggestion: { id: "sg1", line: CLOSE_ACTION.line } },
    ]);
    expect(suggestionOverlay(SUGGESTION, { ...CLOSE_ACTION, dialog: "delete" })?.[0]).toBe("delete_ticket");
    expect(suggestionOverlay(SUGGESTION, { kind: "dismiss_suggestion", id: "sg1" })).toBeNull();
    expect(describeAction({ kind: "dismiss_suggestion", id: "sg1" })).toBe("dismiss_suggestion sg1");
    const sent: unknown[] = [];
    await confirmSuggestion(
      "SHOP-40.6",
      { id: "sg1", line: CLOSE_ACTION.line },
      "close",
      (method, params) => {
        sent.push([method, params]);
        return Promise.resolve(null);
      },
    );
    expect(sent).toEqual([["inbox.act", { key: "SHOP-40.6", action: CLOSE_ACTION }]]);
  });
});
