// The Add project dialog's go-ahead (ONB-08, d0108; *14 → Row · Another project, and the presets* `s14-18`/`s14-19`),
// without the DOM: once a project is added later, the dialog asks the same go-ahead as onboarding's Flow step, shorter.
// Look and propose a flow starts the onboarding agent and closes onto the new project's board with the agent in its
// pane; a preset picked proposes it; either proposal opens in the usual preview (Apply · Ask for more changes · Cancel),
// so nothing here holds it. Skip, use the default only closes. The words are Rust's (`onboarding.flow_dialog`).
import { selectProject } from "../../app/selection";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { PaneFlowStarted } from "../../generated/PaneFlowStarted";
import { proposeFlow } from "../flow_preview/logic";
import { PANE_ROUTES, type PaneRoutes } from "../pane/logic";
import { afterFlowStep, type Ask } from "./logic";

/** Where the dialog is: the form, then, once the project is added, its go-ahead or the presets. */
export type AddDialogState =
  { at: "form" } | { at: "go_ahead"; project: string } | { at: "presets"; project: string };

/** The project was added: its board shows behind the dialog, which moves to the go-ahead while it loads. */
export function projectAdded(key: string, routes: PaneRoutes = PANE_ROUTES): AddDialogState {
  selectProject(key, routes.selection);
  return { at: "go_ahead", project: key };
}

/** The go-ahead's words arrived: the presets when there is nothing to look at, else the go-ahead. */
export function dialogOpens(state: AddDialogState, step: OnboardingFlowStep): AddDialogState {
  if (state.at === "form") return state;
  return { at: step.opens_on_presets ? "presets" : "go_ahead", project: state.project };
}

/** Look and propose a flow: the onboarding agent starts and shows in the new project's pane. Resolves to its id. */
export async function lookFromDialog(
  project: string,
  ask: Ask,
  routes: PaneRoutes = PANE_ROUTES,
): Promise<string> {
  const started = (await ask("pane.start_flow", { project })) as PaneFlowStarted;
  afterFlowStep(project, started.id, routes);
  return started.id;
}

/** A preset picked in the dialog: its proposal, and the usual preview over the new project's board. */
export function presetFromDialog(project: string, file: string, ask: Ask): Promise<void> {
  return proposeFlow(project, { from: "preset", file }, ask);
}
