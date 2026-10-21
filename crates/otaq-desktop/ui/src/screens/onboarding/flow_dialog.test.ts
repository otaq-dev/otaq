// The Add project dialog's go-ahead (T-0431; ONB-08): after a project is added later, the same go-ahead as the Flow
// step, shorter; Skip only closes, Look starts the onboarding agent in the new project's pane, a preset opens the
// usual preview.
import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { INITIAL_LAYOUT, type ShellLayout } from "../../app/layout";
import { overlays } from "../../app/overlays";
import { createSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { PaneFocusRequest, PaneRoutes, ShownAgents } from "../pane/logic";
import { FlowDialogButtons, FlowDialogGoAhead, FlowDialogPresets } from "./AddProjectDialog";
import { dialogOpens, lookFromDialog, presetFromDialog, projectAdded } from "./flow_dialog";

afterEach(() => {
  overlays.store.set([]);
});

// `onboarding.flow_dialog` for billing-api on GitLab, added after shop-web (`s14-18`).
const STEP: OnboardingFlowStep = {
  project: "BILL",
  title: "billing-api’s flow",
  intro: "Look at how you work here, like for shop-web.",
  line: "Claude looks at how you use Claude Code on this Mac, this repo and GitLab, and proposes a flow for billing-api.",
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
      {
        name: "Team PR",
        delivers: "Done · PR, automatic",
        line: "Teams that review on GitHub or GitLab: the PR is the review",
        columns: ["Ready for Dev", "Dev", "Validate", "Done"].map((label) => ({
          icon: "bot" as const,
          label,
        })),
        file: "team-pr.yaml",
      },
    ],
  },
  start_message: "/flow",
  working: { title: "Reading how you work", text: "This can take a few minutes." },
};

const NOTHING: OnboardingFlowStep = {
  ...STEP,
  can_propose: false,
  opens_on_presets: true,
  presets: {
    ...STEP.presets,
    text: "Nothing to look at yet · pick a preset to start. You can change it later by asking the Project agent.",
    reason:
      "No Claude Code history on this Mac, no commits in this repo yet, and neither gh nor glab is signed in.",
  },
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

const noop = () => undefined;

describe("the Add project dialog's go-ahead", () => {
  it("add a project → the go-ahead → Skip closes on the default flow", () => {
    const r = routes();
    const { asked } = recorder();
    // Added: the new project's board shows behind the dialog.
    const added = projectAdded("BILL", r);
    expect(r.selection.get().project).toBe("BILL");
    const shown = dialogOpens(added, STEP);
    expect(shown).toEqual({ at: "go_ahead", project: "BILL" });
    const html = renderToStaticMarkup(
      createElement(
        Fragment,
        null,
        createElement(FlowDialogGoAhead, { step: STEP }),
        createElement(FlowDialogButtons, {
          step: STEP,
          busy: false,
          onSkip: noop,
          onPresets: noop,
          onLook: noop,
        }),
      ),
    );
    for (const text of [STEP.intro, STEP.line, STEP.provider, STEP.skip, STEP.use_preset, STEP.propose]) {
      expect(html).toContain(text);
    }
    expect(html).not.toMatch(/disabled=""/);
    // Skip only closes: nothing is asked, no agent shows, the project keeps the default flow.
    expect(asked).toEqual([]);
    expect(r.shown.get()).toEqual({});
    expect(r.layout.get().paneHidden).toBe(true);
    expect(overlays.store.get()).toEqual([]);
  });

  it("nothing to look at opens on the presets, with the reason and Look off", () => {
    expect(dialogOpens({ at: "go_ahead", project: "API" }, NOTHING)).toEqual({
      at: "presets",
      project: "API",
    });
    expect(dialogOpens({ at: "form" }, NOTHING)).toEqual({ at: "form" });
    const html = renderToStaticMarkup(
      createElement(
        Fragment,
        null,
        createElement(FlowDialogPresets, { step: NOTHING, busy: false, onPick: noop }),
        createElement(FlowDialogButtons, {
          step: NOTHING,
          busy: false,
          onSkip: noop,
          onPresets: noop,
          onLook: noop,
        }),
      ),
    );
    expect(html).toContain(NOTHING.presets.text);
    expect(html).toContain(NOTHING.presets.reason ?? "");
    expect(html).toContain("Team PR");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Look and propose a flow/);
  });

  it("Look and propose a flow starts the onboarding agent and shows it in the new project's pane", async () => {
    const r = routes();
    const { asked, ask } = recorder({ "pane.start_flow": { id: "pa-7", session: "s-1" } });
    const agent = await lookFromDialog("BILL", ask, r);
    expect(agent).toBe("pa-7");
    expect(asked).toEqual([["pane.start_flow", { project: "BILL" }]]);
    expect(r.selection.get().project).toBe("BILL");
    expect(r.shown.get()).toEqual({ BILL: "pa-7" });
    expect(r.layout.get().paneHidden).toBe(false);
  });

  it("a preset proposes it and opens the usual preview", async () => {
    const { asked, ask } = recorder({ "flow_preview.propose": { project: "API", proposal: 9 } });
    await presetFromDialog("API", "team-pr.yaml", ask);
    expect(asked).toEqual([
      ["flow_preview.propose", { project: "API", source: { from: "preset", file: "team-pr.yaml" } }],
    ]);
    expect(overlays.store.get()).toEqual([{ id: "flow_preview", params: { project: "API", focus: true } }]);
  });
});
