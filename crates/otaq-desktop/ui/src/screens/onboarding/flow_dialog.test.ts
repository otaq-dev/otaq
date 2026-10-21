// The Add project dialog's go-ahead (T-0431, T-0434; ONB-08): after a project is added later, the same go-ahead as the
// Flow step, shorter; Skip settles the default flow, Ask <Tool> starts that tool's flow agent in the new project's
// pane, a preset opens the usual preview.
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
import { askFromDialog, dialogOpens, presetFromDialog, projectAdded } from "./flow_dialog";
import { NO_TOOL, recorder, STEP as SHOP_STEP } from "./flow_fixtures";
import { chooseDefault } from "./logic";

afterEach(() => {
  overlays.store.set([]);
});

// `onboarding.flow_dialog` for billing-api, added after shop-web (`s14-18`), with Claude Code and OpenCode connected.
const STEP: OnboardingFlowStep = {
  ...SHOP_STEP,
  project: "BILL",
  title: "billing-api’s flow",
  intro: "Look at how you work here, like for shop-web.",
  line: "Each agent looks at how you use it on this Mac and at this repo, and proposes a flow for billing-api.",
};

const NOTHING: OnboardingFlowStep = { ...NO_TOOL, project: "BILL", title: "billing-api’s flow" };

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
  it("add a project → the go-ahead → Skip settles the default flow", async () => {
    const r = routes();
    const { asked, ask } = recorder();
    // Added: the new project's board shows behind the dialog.
    const added = projectAdded("BILL", r);
    expect(r.selection.get().project).toBe("BILL");
    const shown = dialogOpens(added, STEP);
    expect(shown).toEqual({ at: "go_ahead", project: "BILL" });
    const html = renderToStaticMarkup(
      createElement(
        Fragment,
        null,
        createElement(FlowDialogGoAhead, { step: STEP, busy: false, onAsk: noop }),
        createElement(FlowDialogButtons, {
          step: STEP,
          busy: false,
          onSkip: noop,
          onPresets: noop,
        }),
      ),
    );
    for (const text of [STEP.intro, STEP.line, STEP.provider, STEP.skip, STEP.use_preset, "Ask OpenCode"]) {
      expect(html).toContain(text);
    }
    expect(html).not.toMatch(/disabled=""/);
    // Skip: the default flow, no agent shows.
    await chooseDefault("BILL", ask);
    expect(asked).toEqual([["onboarding.use_default", { project: "BILL" }]]);
    expect(r.shown.get()).toEqual({});
    expect(r.layout.get().paneHidden).toBe(true);
    expect(overlays.store.get()).toEqual([]);
  });

  it("no agent tool connected opens on the presets, with the line", () => {
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
        }),
      ),
    );
    expect(html).toContain(NOTHING.presets.text);
    expect(html).toContain(NOTHING.presets.note ?? "");
    expect(html).toContain("Grill first");
    expect(html).not.toContain("Ask ");
  });

  it("Ask OpenCode starts its flow agent and shows it in the new project's pane", async () => {
    const r = routes();
    const { asked, ask } = recorder({ "onboarding.ask": { id: "pa-7", session: "s-1" } });
    const agent = await askFromDialog("BILL", "opencode", ask, r);
    expect(agent).toBe("pa-7");
    expect(asked).toEqual([["onboarding.ask", { project: "BILL", tool: "opencode" }]]);
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
