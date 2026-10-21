// The Add project dialog's go-ahead and the Flow setup page (T-0431, T-0434, T-0436; ASK-04, ASK-08): after a project
// is added later, the same go-ahead as the Flow step, shorter; Ask <Tool> closes onto the project's Flow setup page,
// Skip settles the default flow (the board), a preset opens the usual preview over the page. Leaving to another
// project and coming back finds the page again.
import { createElement, Fragment } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { createSelection, selectProject } from "../../app/selection";
import type { FlowSetupPageView } from "../../generated/FlowSetupPageView";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import { FlowDialogButtons, FlowDialogGoAhead, FlowDialogPresets } from "./AddProjectDialog";
import { askFromDialog, dialogOpens, presetFromDialog, projectAdded } from "./flow_dialog";
import { NO_TOOL, recorder, STEP as SHOP_STEP } from "./flow_fixtures";
import { backToTools, mainAreaShows } from "./flow_setup_page";
import { chooseDefault } from "./logic";

afterEach(() => {
  overlays.store.set([]);
});

// `onboarding.flow_dialog` for billing-api, added after shop-web (`s14-18`), with Claude Code and OpenCode connected.
const STEP: OnboardingFlowStep = {
  ...SHOP_STEP,
  project: "BILL",
  title: "billing-api’s flow",
  line: "Each agent looks at how you use it on this Mac and at this repo, and proposes a flow for billing-api.",
};

const NOTHING: OnboardingFlowStep = { ...NO_TOOL, project: "BILL", title: "billing-api’s flow" };

/** `onboarding.flow_setup_page` for `project`. */
const page = (project: string, shows: boolean): FlowSetupPageView => ({
  project,
  shows,
  back_to_tools: "agent_tools",
});

const noop = () => undefined;

describe("the Add project dialog's go-ahead (`s14-18`)", () => {
  it("draws the line, an Ask per tool and the provider line, with no intro and no heading", () => {
    const html = renderToStaticMarkup(
      createElement(
        Fragment,
        null,
        createElement(FlowDialogGoAhead, { step: STEP, busy: false, onAsk: noop }),
        createElement(FlowDialogButtons, { step: STEP, busy: false, onSkip: noop, onPresets: noop }),
      ),
    );
    const order = [
      STEP.line,
      "Ask Claude Code",
      "Ask OpenCode",
      STEP.provider,
      STEP.skip,
      STEP.use_preset,
    ].map((text) => html.indexOf(text));
    expect(order.every((at) => at >= 0)).toBe(true);
    expect(order).toEqual(order.toSorted((a, b) => a - b));
    expect(html).not.toContain(STEP.intro);
    expect(html).not.toContain(STEP.heading);
    expect(html).not.toMatch(/disabled=""/);
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
        createElement(FlowDialogButtons, { step: NOTHING, busy: false, onSkip: noop, onPresets: noop }),
      ),
    );
    expect(html).toContain(NOTHING.presets.text);
    expect(html).toContain(NOTHING.presets.note ?? "");
    expect(html).toContain("Grill first");
    expect(html).not.toContain("Ask ");
  });

  it("a preset proposes it and opens the usual preview over the page", async () => {
    const { asked, ask } = recorder({ "flow_preview.propose": { project: "API", proposal: 9 } });
    await presetFromDialog("API", "team-pr.yaml", ask);
    expect(asked).toEqual([
      ["flow_preview.propose", { project: "API", source: { from: "preset", file: "team-pr.yaml" } }],
    ]);
    expect(overlays.store.get()).toEqual([{ id: "flow_preview", params: { project: "API", focus: true } }]);
  });
});

describe("the Flow setup page (`s14-18`, After Ask)", () => {
  it("Ask from the dialog → the project's Flow setup page, the agent at work there", async () => {
    const selection = createSelection();
    selectProject("SHOP", selection);
    // Added: billing-api is selected behind the dialog, which moves to the go-ahead.
    expect(dialogOpens(projectAdded("BILL", selection), STEP)).toEqual({ at: "go_ahead", project: "BILL" });
    expect(selection.get().project).toBe("BILL");
    const { asked, ask } = recorder({ "onboarding.ask": { id: "pa-7", session: "s-1" } });
    expect(await askFromDialog("BILL", "opencode", ask, selection)).toBe("pa-7");
    expect(asked).toEqual([["onboarding.ask", { project: "BILL", tool: "opencode" }]]);
    // The dialog closed; the main area is the page, not the board.
    expect(selection.get().project).toBe("BILL");
    expect(mainAreaShows(page("BILL", true), "BILL", false)).toBe("page");
  });

  it("leaving to another project and coming back → the page", () => {
    const selection = createSelection();
    selectProject("BILL", selection);
    expect(mainAreaShows(page("BILL", true), selection.get().project, false)).toBe("page");
    selectProject("SHOP", selection);
    // The answer for billing-api isn't shop-web's: nothing until shop-web's comes, then its board.
    expect(mainAreaShows(page("BILL", true), selection.get().project, false)).toBe("nothing");
    expect(mainAreaShows(page("SHOP", false), selection.get().project, false)).toBe("board");
    selectProject("BILL", selection);
    expect(mainAreaShows(page("SHOP", false), selection.get().project, false)).toBe("nothing");
    expect(mainAreaShows(page("BILL", true), selection.get().project, false)).toBe("page");
  });

  it("Use the default → the board", async () => {
    const { asked, ask } = recorder();
    await chooseDefault("BILL", ask);
    expect(asked).toEqual([["onboarding.use_default", { project: "BILL" }]]);
    // `projects_changed` and `flow_setup_changed` ask again: the project is settled.
    expect(mainAreaShows(page("BILL", false), "BILL", false)).toBe("board");
  });

  it("draws nothing behind onboarding's Flow step, and the board with no project", () => {
    expect(mainAreaShows(page("BILL", true), "BILL", true)).toBe("nothing");
    expect(mainAreaShows(page("SHOP", false), "SHOP", true)).toBe("board");
    expect(mainAreaShows(undefined, null, false)).toBe("board");
    expect(mainAreaShows(undefined, "BILL", false)).toBe("nothing");
  });

  it("Back to Tools opens Settings → Agent tools", () => {
    const opened: unknown[] = [];
    backToTools(page("BILL", true), (params) => opened.push(params));
    expect(opened).toEqual([{ section: "agent_tools" }]);
  });
});
