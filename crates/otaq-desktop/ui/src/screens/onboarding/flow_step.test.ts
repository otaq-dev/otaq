// The Flow step (T-0429; ONB-04…ONB-07): its state machine, the start and the preset behind one function each, how it
// ends on the project's board, its screens as Rust words them, and the usual preview held off while it shows.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { INITIAL_LAYOUT, type ShellLayout } from "../../app/layout";
import { createSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { PageEvent } from "../../generated/PageEvent";
import type { PageEvents } from "../../service";
import { applyPreview, cancelPreview, followProposals, NO_CHOICES } from "../flow_preview/logic";
import type { PaneFocusRequest, PaneRoutes, ShownAgents } from "../pane/logic";
import { FlowGoAhead, FlowPresets, FlowProposal } from "./FlowStep";
import {
  afterFlowStep,
  flowAgent,
  flowStepOpens,
  proposalGone,
  proposalShown,
  proposePreset,
  STEP_ORDER,
  startFlowAgent,
} from "./logic";

const CARD = (name: string, file: string, columns: string[]) => ({
  name,
  delivers: "Done · click",
  line: "Today’s board",
  columns: columns.map((label) => ({ icon: "bot" as const, label })),
  file,
});

// `onboarding.flow` for the example's shop-web (`s14-12`).
const STEP: OnboardingFlowStep = {
  project: "SHOP",
  title: "Build shop-web’s flow",
  intro:
    "An agent proposes the board’s columns: who works on a ticket, what checks it, how it’s delivered. Nothing changes until you say so.",
  line: "Claude looks at how you use Claude Code on this Mac (what you typed in it, your skills and CLAUDE.md files), this repo and GitHub, and proposes a flow for shop-web.",
  provider: "What the agent reads goes to your model provider, and it uses tokens.",
  skip: "Skip, use the default",
  use_preset: "Use a preset instead",
  propose: "Look and propose a flow",
  can_propose: true,
  opens_on_presets: false,
  presets: {
    text: "An agent proposes the board’s columns: who works on a ticket, what checks it, how it’s delivered. Nothing changes until you say so.",
    reason: null,
    heading: "START FROM A PRESET",
    cards: [
      CARD("Otaq default", "otaq-default.yaml", ["Ready for Dev", "Dev", "Validate", "Review", "Done"]),
      CARD("Grill first", "grill-first.yaml", [
        "Ready for Dev",
        "Grill",
        "Dev",
        "Validate",
        "Review",
        "Done",
      ]),
    ],
  },
  start_message: "/flow",
  working: { title: "Reading how you work", text: "This can take a few minutes." },
};

const NOTHING: OnboardingFlowStep = {
  ...STEP,
  line: "Nothing to look at yet · pick a preset to start. You can change it later by asking the Project agent.",
  can_propose: false,
  opens_on_presets: true,
  presets: {
    ...STEP.presets,
    text: "Nothing to look at yet · pick a preset to start. You can change it later by asking the Project agent.",
    reason:
      "No Claude Code history on this Mac, no commits in this repo yet, and neither gh nor glab is signed in.",
  },
};

// `flow_preview.view` with `onboarding` for a preset picked in the step.
const PREVIEW: FlowPreviewView = {
  project: "SHOP",
  proposal: 4,
  title: "Grill first for shop-web",
  subtitle: "Preset · nothing changes until you apply it",
  close_label: "Close",
  same: null,
  table: null,
  changes: [],
  agents: null,
  problems: [],
  clashes: [],
  warnings: [],
  applies: "Applies now: shop-web has no tickets yet.",
  agent_waits: null,
  cancel: "Use the default",
  ask: null,
  apply: { label: "Use this flow", enabled: true, off: null },
  close: null,
  choices: NO_CHOICES,
};

function recorder(answers: Record<string, unknown> = {}) {
  const asked: [string, unknown][] = [];
  const ask = (method: string, params?: unknown) => {
    asked.push([method, params]);
    return Promise.resolve(answers[method] ?? null);
  };
  return { asked, ask };
}

function routes(): PaneRoutes {
  return {
    selection: createSelection(),
    layout: createStore<ShellLayout>({ ...INITIAL_LAYOUT, paneHidden: true }),
    shown: createStore<ShownAgents>({}),
    focus: createStore<PaneFocusRequest | null>(null),
  };
}

describe("the Flow step", () => {
  it("is the fifth step; Done is the board itself", () => {
    expect(STEP_ORDER).toEqual(["welcome", "theme", "tools", "project", "flow"]);
  });

  it("opens on the go-ahead, or on the presets when there is nothing to look at", () => {
    expect(flowStepOpens(STEP)).toEqual({ at: "go_ahead" });
    expect(flowStepOpens(NOTHING)).toEqual({ at: "presets", agent: null });
  });

  it("Skip finishes on the default flow: the board shows, nothing is asked", () => {
    const r = routes();
    const { asked } = recorder();
    afterFlowStep("SHOP", flowAgent(flowStepOpens(STEP)), r);
    expect(r.selection.get().project).toBe("SHOP");
    expect(r.shown.get()).toEqual({});
    expect(r.layout.get().paneHidden).toBe(true);
    expect(asked).toEqual([]);
  });

  it("Look and propose a flow starts the Project agent with the flow skill; its proposal shows in the step", async () => {
    const { asked, ask } = recorder({ "pane.send": "pa-1" });
    const agent = await startFlowAgent(STEP, ask);
    expect(agent).toBe("pa-1");
    expect(asked).toEqual([["pane.send", { project: "SHOP", message: "/flow" }]]);
    const proposal = proposalShown({ at: "working", agent }, 3);
    expect(proposal).toEqual({ at: "proposal", agent: "pa-1", first: 3 });
    // The agent proposes again after Ask for changes: the first stays, so the preview reads "Updated…".
    expect(proposalShown(proposal, 5)).toBe(proposal);
    // Cancelled from elsewhere: back to the agent at work.
    expect(proposalGone(proposal)).toEqual({ at: "working", agent: "pa-1" });
    // Done: the agent stays open in its pane.
    const r = routes();
    afterFlowStep("SHOP", flowAgent(proposal), r);
    expect(r.shown.get()).toEqual({ SHOP: "pa-1" });
    expect(r.layout.get().paneHidden).toBe(false);
  });

  it("a preset → the preview → Use this flow applies it", async () => {
    const { asked, ask } = recorder({ "flow_preview.propose": { project: "SHOP", proposal: 4 } });
    const proposal = await proposePreset(STEP, "grill-first.yaml", ask);
    expect(proposal).toBe(4);
    const html = renderToStaticMarkup(
      createElement(FlowProposal, {
        view: PREVIEW,
        choices: NO_CHOICES,
        busy: false,
        error: null,
        onChoices: () => undefined,
        onError: () => undefined,
        onUseDefault: () => undefined,
        onAsk: () => undefined,
        onUse: () => undefined,
      }),
    );
    expect(html).toContain("Grill first for shop-web");
    expect(html).toContain("Use the default");
    expect(html).toContain("Use this flow");
    expect(html).not.toContain("Ask for changes");
    await applyPreview(PREVIEW, ask);
    expect(asked).toEqual([
      ["flow_preview.propose", { project: "SHOP", source: { from: "preset", file: "grill-first.yaml" } }],
      ["flow_preview.apply", { project: "SHOP", proposal: 4, choices: NO_CHOICES }],
    ]);
    expect(proposalGone({ at: "proposal", agent: null, first: 4 })).toEqual({ at: "presets", agent: null });
  });

  it("Use the default cancels the proposal (the agent gets Cancel)", async () => {
    const { asked, ask } = recorder();
    await cancelPreview(PREVIEW, ask);
    expect(asked).toEqual([["flow_preview.cancel", { project: "SHOP" }]]);
  });

  it("draws the go-ahead and Nothing to look at yet as Rust words them", () => {
    const props = {
      busy: false,
      onSkip: () => undefined,
      onPresets: () => undefined,
      onLook: () => undefined,
    };
    const goAhead = renderToStaticMarkup(createElement(FlowGoAhead, { ...props, step: STEP }));
    for (const text of [STEP.title, STEP.line, STEP.provider, STEP.skip, STEP.use_preset, STEP.propose]) {
      expect(goAhead).toContain(text);
    }
    expect(goAhead).not.toMatch(/disabled=""/);
    const nothing = renderToStaticMarkup(
      createElement(FlowPresets, { ...props, step: NOTHING, onPick: () => undefined }),
    );
    expect(nothing).toContain(NOTHING.presets.text);
    expect(nothing).toContain(NOTHING.presets.reason ?? "");
    expect(nothing).toContain("START FROM A PRESET");
    expect(nothing).toContain("Otaq default");
    expect(nothing).toContain("Grill first");
    // Look and propose a flow is off with nothing to look at.
    expect(nothing).toMatch(/<button[^>]*disabled=""[^>]*>.*Look and propose a flow/);
  });
});

describe("the usual preview while the Flow step shows", () => {
  it("doesn't open over the step's project; others' still do", async () => {
    let listener: ((e: PageEvent) => void) | undefined;
    const events: PageEvents = {
      listen(l) {
        listener = l;
        return { ready: Promise.resolve(), stop: () => undefined };
      },
    };
    const held = createStore<string | null>("SHOP");
    const opened: string[] = [];
    const stop = followProposals(
      events,
      (p) => opened.push(p),
      () => Promise.resolve(["SHOP"]),
      held,
    );
    await Promise.resolve();
    await Promise.resolve();
    const proposed = (project: string): PageEvent =>
      ({
        kind: "service",
        name: "flow_changed",
        event: { project, change: { what: "proposed", proposal: 3 } },
      }) as unknown as PageEvent;
    listener?.(proposed("SHOP"));
    listener?.(proposed("API"));
    expect(opened).toEqual(["API"]);
    // The step is over: the next start's resync opens the stored proposal (ONB-07).
    held.set(null);
    listener?.({ kind: "resync" });
    await Promise.resolve();
    await Promise.resolve();
    expect(opened).toEqual(["API", "SHOP"]);
    stop();
  });
});
