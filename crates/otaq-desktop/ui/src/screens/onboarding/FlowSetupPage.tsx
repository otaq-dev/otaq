// A later project's Flow setup page (T-0436; ASK-04, ASK-08, d0108; *14 → Row · Another project, and the presets*
// `s14-18`/`s14-19`): a project added after another one's flow is settled shows this page in its main area instead of
// its board until its flow is settled. The body is onboarding's Flow step (`FlowStep.tsx`): the go-ahead, the tabs over
// the flow agents' terminals with the Agents at work card, and the proposals side by side. Settling it (Use this flow,
// a preset applied, Use the default, Skip) shows the board, with the chosen proposal's flow agent in the pane; leaving
// to another project and coming back finds the page again. No tool connected: the presets, Skip and Back to Tools,
// which opens Settings → Agent tools here. The title bar's "Flow setup · 2 agents at work" and the sidebar's mark are
// the header's and the rail's (`shell.header`, `shell.rail`).
import { type ComponentType, useCallback } from "react";

import { useOpenOverlays } from "../../app/overlays";
import { useSelection } from "../../app/selection";
import type { FlowSetupPageView } from "../../generated/FlowSetupPageView";
import type { OnboardingProjectParams } from "../../generated/OnboardingProjectParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { FlowStep } from "./FlowStep";
import { backToTools, FLOW_SETUP_PAGE_EVENTS, mainAreaShows } from "./flow_setup_page";
import { afterFlowStep } from "./logic";

/** The page for the project of `view` (in Flow setup). */
export function FlowSetupPage({ view }: { view: FlowSetupPageView }) {
  const { project } = view;
  return (
    <div className="flow-setup-page" aria-label={project}>
      <FlowStep
        key={project}
        project={project}
        onDone={(agent) => {
          afterFlowStep(project, agent);
        }}
        onBackToTools={() => {
          backToTools(view);
        }}
      />
    </div>
  );
}

/** `Board` for the board slot, or the shown project's Flow setup page while that project is in Flow setup. */
export function withFlowSetup(Board: ComponentType): ComponentType {
  return function BoardOrFlowSetup() {
    const { project } = useSelection();
    const onboardingOpen = useOpenOverlays().some((o) => o.id === "onboarding");
    const load = useCallback(() => {
      if (project === null) return new Promise<never>(() => undefined);
      const params: OnboardingProjectParams = { project };
      return request("onboarding.flow_setup_page", params) as Promise<FlowSetupPageView>;
    }, [project]);
    const { view } = useServiceView(load, FLOW_SETUP_PAGE_EVENTS);
    switch (mainAreaShows(view, project, onboardingOpen)) {
      case "board":
        return <Board />;
      case "page":
        return view && <FlowSetupPage view={view} />;
      case "nothing":
        return null;
    }
  };
}
