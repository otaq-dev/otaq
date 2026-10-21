// The Flow step (T-0434; ASK-04…ASK-06): where it opens and how its Flow setup moves it on, Skip settling the default
// flow, a proposal in the step and Use this flow, a preset, its screens as Rust words them, and the usual preview held
// off while it shows.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { INITIAL_LAYOUT, type ShellLayout } from "../../app/layout";
import { createSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { PageEvent } from "../../generated/PageEvent";
import type { PageEvents } from "../../service";
import { applyPreview, followProposals, NO_CHOICES } from "../flow_preview/logic";
import type { PaneFocusRequest, PaneRoutes, ShownAgents } from "../pane/logic";
import { FlowGoAhead, FlowPresets, FlowProposal } from "./FlowStep";
import { atWork, NO_TOOL, recorder, STEP, stepAtWork, working } from "./flow_fixtures";
import {
  afterFlowStep,
  flowStepChanged,
  flowStepOpens,
  proposalGone,
  proposalShown,
  proposePreset,
  proposingAgent,
  STEP_ORDER,
  chooseDefault,
  firstProposals,
} from "./logic";

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

  it("opens on the go-ahead, on the agents at work once a tool was asked, or on the presets with no tool", () => {
    expect(flowStepOpens(STEP)).toEqual({ at: "go_ahead" });
    expect(flowStepOpens(stepAtWork([working("claude", "a-cc", 0)]))).toEqual({ at: "at_work" });
    expect(flowStepOpens(NO_TOOL)).toEqual({ at: "presets" });
    // A change of the Flow setup moves the go-ahead on; the user's own place stays.
    expect(flowStepChanged({ at: "go_ahead" }, NO_TOOL)).toEqual({ at: "presets" });
    expect(flowStepChanged({ at: "go_ahead" }, STEP)).toEqual({ at: "go_ahead" });
    expect(flowStepChanged({ at: "presets" }, stepAtWork([]))).toEqual({ at: "presets" });
  });

  it("Skip settles the default flow: every proposal closes, the board shows with no agent", async () => {
    const r = routes();
    const { asked, ask } = recorder();
    await chooseDefault("SHOP", ask);
    expect(asked).toEqual([["onboarding.use_default", { project: "SHOP" }]]);
    afterFlowStep("SHOP", null, r);
    expect(r.selection.get().project).toBe("SHOP");
    expect(r.shown.get()).toEqual({});
    expect(r.layout.get().paneHidden).toBe(true);
  });

  it("an agent's proposal shows in the step; Use this flow keeps that agent open in the pane", async () => {
    const proposing = atWork([
      { ...working("claude", "a-cc", 3), state: "proposed", status: "Proposed", proposal: 3, buttons: [] },
      working("opencode", "a-oc", 2),
    ]);
    const proposal = proposalShown({ at: "at_work" }, 3);
    expect(proposal).toEqual({ at: "proposal", first: 3 });
    // It proposes again after Ask for changes: the first stays, so the preview reads "Updated…".
    expect(proposalShown(proposal, 5)).toBe(proposal);
    // Gone from elsewhere: back where the step opens now.
    expect(flowStepChanged(proposalGone(proposal), stepAtWork(proposing.rows))).toEqual({ at: "at_work" });
    expect(proposingAgent(proposing, 3)).toBe("a-cc");
    expect(proposingAgent(proposing, 4)).toBeNull();
    const { asked, ask } = recorder();
    await applyPreview({ ...PREVIEW, proposal: 3 }, ask);
    expect(asked).toEqual([["flow_preview.apply", { project: "SHOP", proposal: 3, choices: NO_CHOICES }]]);
    const r = routes();
    afterFlowStep("SHOP", proposingAgent(proposing, 3), r);
    expect(r.shown.get()).toEqual({ SHOP: "a-cc" });
    expect(r.layout.get().paneHidden).toBe(false);
  });

  it("compared proposals keep the one each agent showed first", () => {
    const compare = (agent: string | null, proposal: number) => ({
      project: "SHOP",
      compare: true,
      proposals: [{ tool: "claude", agent, view: { ...PREVIEW, proposal } }],
      others: [],
      closes_others: null,
      use_default: null,
    });
    const firsts = firstProposals({}, compare("a-cc", 3));
    expect(firsts).toEqual({ "a-cc": 3 });
    // Its proposal after Ask for changes: the first stays, and nothing new means the same map.
    expect(firstProposals(firsts, compare("a-cc", 5))).toBe(firsts);
    // A preset (no agent) is never recorded.
    expect(firstProposals(firsts, compare(null, 6))).toBe(firsts);
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
    await applyPreview(PREVIEW, ask);
    expect(asked).toEqual([
      ["flow_preview.propose", { project: "SHOP", source: { from: "preset", file: "grill-first.yaml" } }],
      ["flow_preview.apply", { project: "SHOP", proposal: 4, choices: NO_CHOICES }],
    ]);
    expect(proposalGone({ at: "proposal", first: 4 })).toEqual({ at: "go_ahead" });
  });

  it("draws the go-ahead (a row per tool, the only one primary) and no agent tool connected", () => {
    const ignore = () => undefined;
    const goAhead = renderToStaticMarkup(
      createElement(FlowGoAhead, {
        step: STEP,
        busy: false,
        onAsk: ignore,
        onSkip: ignore,
        onPresets: ignore,
      }),
    );
    for (const text of [STEP.title, STEP.line, STEP.heading, STEP.provider, STEP.skip, STEP.use_preset]) {
      expect(goAhead).toContain(text);
    }
    expect(goAhead).toContain("Ask Claude Code");
    expect(goAhead).toContain("Ask OpenCode");
    expect(goAhead.match(/connected/g)).toHaveLength(2);
    expect(goAhead).not.toContain('data-variant="primary"');
    expect(goAhead).not.toContain("Look and propose");
    const one = { ...STEP, rows: STEP.rows.slice(0, 1).map((r) => ({ ...r, primary: true })) };
    const single = renderToStaticMarkup(
      createElement(FlowGoAhead, {
        step: one,
        busy: false,
        onAsk: ignore,
        onSkip: ignore,
        onPresets: ignore,
      }),
    );
    expect(single).toMatch(/data-variant="primary"[^>]*>.*?Ask Claude Code/);

    const none = renderToStaticMarkup(
      createElement(FlowPresets, {
        step: NO_TOOL,
        busy: false,
        onPick: ignore,
        onSkip: ignore,
        onAgents: ignore,
        onBackToTools: ignore,
      }),
    );
    expect(none).toContain(NO_TOOL.presets.text);
    expect(none).toContain(NO_TOOL.presets.note ?? "");
    expect(none).toContain("Back to Tools");
    expect(none).toContain("START FROM A PRESET");
    expect(none).toContain("Otaq default");
    expect(none).toContain("Skip, use the default");
    expect(none).not.toContain("Ask an agent");
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
