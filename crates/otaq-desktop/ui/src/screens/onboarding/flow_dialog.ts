// The Add project dialog's go-ahead (ONB-08, d0108; *14 → Row · Another project, and the presets* `s14-18`/`s14-19`),
// without the DOM: once a project is added later, the dialog asks the same go-ahead as onboarding's Flow step, shorter.
// Ask <Tool> starts that tool's flow agent and closes onto the new project's board with the agent in its pane; a preset
// picked proposes it; either proposal opens in the usual preview (Apply · Ask for more changes · Cancel), so nothing
// here holds it. Skip, use the default settles the default flow. The words are Rust's (`onboarding.flow_dialog`).
import { selectProject } from "../../app/selection";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import { proposeFlow } from "../flow_preview/logic";
import { PANE_ROUTES, type PaneRoutes } from "../pane/logic";
import { afterFlowStep, type Ask, askTool } from "./logic";

/** Where the dialog is: the form, then, once the project is added, its go-ahead or the presets. */
export type AddDialogState =
  { at: "form" } | { at: "go_ahead"; project: string } | { at: "presets"; project: string };

/** The project was added: its board shows behind the dialog, which moves to the go-ahead while it loads. */
export function projectAdded(key: string, routes: PaneRoutes = PANE_ROUTES): AddDialogState {
  selectProject(key, routes.selection);
  return { at: "go_ahead", project: key };
}

/** The go-ahead's words arrived: the presets when no agent tool is connected, else the go-ahead. */
export function dialogOpens(state: AddDialogState, step: OnboardingFlowStep): AddDialogState {
  if (state.at === "form") return state;
  return { at: step.opens === "no_tool" ? "presets" : "go_ahead", project: state.project };
}

/** Ask <Tool>: its flow agent starts and shows in the new project's pane. Resolves to its id. */
export async function askFromDialog(
  project: string,
  tool: string,
  ask: Ask,
  routes: PaneRoutes = PANE_ROUTES,
): Promise<string> {
  const asked = await askTool(project, tool, ask);
  afterFlowStep(project, asked.id, routes);
  return asked.id;
}

/** A preset picked in the dialog: its proposal, and the usual preview over the new project's board. */
export function presetFromDialog(project: string, file: string, ask: Ask): Promise<void> {
  return proposeFlow(project, { from: "preset", file }, ask);
}
