// @vitest-environment happy-dom
// The proposals compared (T-0435, ASK-07; `s14-16` *two proposals to compare*, *one proposal ready, the other
// stopped*): every why line on both, Use this flow applies that proposal by its number, Ask for changes names that
// proposal's agent (the page focuses its terminal tab), a tool not ready shows the page's row beside them.
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import type { FlowCompareView } from "../../generated/FlowCompareView";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import { FlowCompare } from "./FlowCompare";
import { NO_CHOICES, rememberFirsts } from "./logic";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function proposal(id: number, tool: string, rows: [string, string, string][]): FlowPreviewView {
  return {
    project: "SHOP",
    proposal: id,
    title: "A flow for shop-web",
    subtitle: `Recommended by ${tool} · nothing changes until you apply it`,
    close_label: "Close",
    same: null,
    table: {
      headings: ["STAGE", "AGENT · MODEL", "OPTIONS A TICKET CAN PICK"],
      always: "Backlog → Ready for Dev → … → Done: always first and last",
      rows: rows.map(([name, model, why]) => ({
        id: name.toLowerCase(),
        icon: "bot",
        name,
        kind: "Agent",
        agent: `${name} agent`,
        agent_muted: false,
        model,
        options: "—",
        options_muted: true,
        why,
      })),
    },
    changes: [],
    agents: { heading: "AGENTS IT ADDS", rows: [], already: "None: it uses 6 agents already in Otaq" },
    problems: [],
    clashes: [],
    warnings: [{ level: "amber", text: "Done opens a PR on GitHub without asking you." }],
    applies: null,
    agent_waits: "The agent waits for your answer in its terminal",
    cancel: null,
    ask: { label: "Ask for changes", placeholder: "", send: "", back: "" },
    apply: { label: "Use this flow", enabled: true, off: null },
    close: null,
    choices: NO_CHOICES,
  };
}

const CLAUDE = proposal(3, "Claude Code", [
  [
    "Grill",
    "Claude Code · sonnet",
    "You asked for clarification before coding in 34 of 50 sessions (Claude Code)",
  ],
  ["Security", "Claude Code · opus", "From 41 /security-review runs in shop-web and billing-api"],
]);
const OPENCODE = proposal(4, "OpenCode", [
  [
    "Dev",
    "OpenCode · tool default",
    "Most of your OpenCode sessions in shop-web end with the tests passing (OpenCode)",
  ],
  [
    "Validate",
    "OpenCode · tool default",
    "AGENTS.md asks for the tests and a lint pass before every PR (this repo)",
  ],
]);

const TWO: FlowCompareView = {
  project: "SHOP",
  compare: true,
  proposals: [
    { tool: "claude", agent: "a-cc", view: CLAUDE },
    { tool: "opencode", agent: "a-oc", view: OPENCODE },
  ],
  others: [],
  closes_others: "Picking one closes the other: its agent is told you chose another flow, and ends.",
  use_default: "Use the default",
};

let host: HTMLDivElement;
let root: Root;
let asked: [string, unknown][];
let used: number[];
let askedFor: string[];
let defaults: number;

function show(view: FlowCompareView) {
  act(() => {
    root.render(
      createElement(FlowCompare, {
        view,
        onChoices: () => undefined,
        onAskForChanges: (agent: string) => {
          askedFor.push(agent);
        },
        onUsed: (p: number) => {
          used.push(p);
        },
        onUseDefault: () => {
          defaults += 1;
        },
        renderOther: (tool: string) => createElement("div", { className: "row-stub" }, `${tool} · Stopped`),
        ask: (method: string, params?: unknown) => {
          asked.push([method, params]);
          return Promise.resolve({ project: "SHOP", proposal: 4 });
        },
      }),
    );
  });
}

function card(tool: string): HTMLElement {
  const found = host.querySelector<HTMLElement>(`.flow-compare-proposal[data-tool='${tool}']`);
  if (!found) throw new Error(`no ${tool} proposal`);
  return found;
}

function button(within: HTMLElement, label: string): HTMLButtonElement {
  const found = [...within.querySelectorAll("button")].find((b) => b.textContent.trim() === label);
  if (!found) throw new Error(`no ${label}`);
  return found;
}

beforeEach(() => {
  asked = [];
  used = [];
  askedFor = [];
  defaults = 0;
  host = document.body.appendChild(document.createElement("div"));
  root = createRoot(host);
});

afterEach(() => {
  act(() => {
    root.unmount();
  });
  host.remove();
});

describe("the proposals compared", () => {
  it("shows both side by side with every why line, each tool's label, and one Use the default", () => {
    show(TWO);
    for (const view of [CLAUDE, OPENCODE]) {
      for (const row of view.table?.rows ?? []) expect(host.textContent).toContain(row.why);
    }
    expect(card("claude").textContent).toContain("Recommended by Claude Code");
    expect(card("opencode").textContent).toContain("OpenCode · tool default");
    expect(card("opencode").textContent).toContain("None: it uses 6 agents already in Otaq");
    expect(host.textContent).toContain(TWO.closes_others);
    const useDefault = [...host.querySelectorAll("button")].filter(
      (b) => b.textContent.trim() === "Use the default",
    );
    expect(useDefault).toHaveLength(1);
    act(() => {
      useDefault[0]?.click();
    });
    expect(defaults).toBe(1);
  });

  it("Use this flow applies that proposal by its number", async () => {
    show(TWO);
    await act(async () => {
      button(card("opencode"), "Use this flow").click();
      await Promise.resolve();
    });
    expect(asked).toEqual([["flow_preview.apply", { project: "SHOP", proposal: 4, choices: NO_CHOICES }]]);
    expect(used).toEqual([4]);
  });

  it("Ask for changes names that proposal's agent", () => {
    show(TWO);
    act(() => {
      button(card("claude"), "Ask for changes").click();
    });
    expect(askedFor).toEqual(["a-cc"]);
    expect(asked).toEqual([]);
  });

  it("shows a tool not ready beside the ready one, as the page draws its row", () => {
    show({ ...TWO, proposals: TWO.proposals.slice(0, 1), others: ["opencode"] });
    const other = host.querySelector(".flow-compare-other[data-tool='opencode']");
    expect(other?.textContent).toBe("opencode · Stopped");
    expect(host.querySelectorAll(".flow-compare-proposal")).toHaveLength(1);
  });
});

describe("rememberFirsts", () => {
  it("keeps each agent's first proposal, so a later one reads as its answer", () => {
    const first = rememberFirsts({}, TWO);
    expect(first).toEqual({ "a-cc": 3, "a-oc": 4 });
    const answered: FlowCompareView = {
      ...TWO,
      proposals: [{ tool: "claude", agent: "a-cc", view: { ...CLAUDE, proposal: 7 } }],
    };
    expect(rememberFirsts(first, answered)).toBe(first);
    expect(rememberFirsts({}, { ...TWO, proposals: [{ tool: null, agent: null, view: CLAUDE }] })).toEqual(
      {},
    );
  });
});
