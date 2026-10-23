// A later project's Flow setup page (T-0436; ASK-04, FED-09, d0115): a project added after another one's flow is
// settled shows this page in its main area instead of its board until its flow is settled. The page is the flow
// editor from Otaq default (`FlowEditorPage`, as onboarding's Flow step): Made for <project> asks each tool, Use this
// flow or Skip, use the default settles it and shows the board, with the agent the editor's panel showed in the
// Project agent panel; leaving to another project and coming back finds the page again. The title bar's "Flow setup ·
// 2 agents at work" and the sidebar's mark are the header's and the rail's (`shell.header`, `shell.rail`).
import { type ComponentType, useCallback } from "react";

import { useOpenOverlays } from "../../app/overlays";
import { useSelection } from "../../app/selection";
import type { FlowSetupPageView } from "../../generated/FlowSetupPageView";
import type { OnboardingProjectParams } from "../../generated/OnboardingProjectParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { FlowEditorPage } from "../flow_editor/FlowEditor";
import { FLOW_SETUP_PAGE_EVENTS, mainAreaShows } from "./flow_setup_page";
import { afterFlowStep } from "./logic";

/** The page for the project of `view` (in Flow setup). */
export function FlowSetupPage({ view }: { view: FlowSetupPageView }) {
  const { project } = view;
  return (
    <div className="flow-setup-page" aria-label={project}>
      <FlowEditorPage
        key={project}
        project={project}
        onDone={(agent) => {
          afterFlowStep(project, agent);
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
