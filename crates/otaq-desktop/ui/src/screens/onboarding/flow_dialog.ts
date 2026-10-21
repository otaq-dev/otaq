// The Add project dialog's go-ahead (ASK-08, d0108; *14 → Row · Another project, and the presets* `s14-18`/`s14-19`),
// without the DOM: once a project is added later, it is selected (its Flow setup page shows behind the dialog) and the
// dialog asks the same go-ahead as onboarding's Flow step, shorter. Ask <Tool> starts that tool's flow agent and closes
// onto the Flow setup page, where the agent works (ASK-06, ASK-07 there); a preset picked proposes it and opens the
// usual preview over the page; Skip, use the default settles the default flow, so the board shows. The words are
// Rust's (`onboarding.flow_dialog`).
import { type Selection, selection as appSelection, selectProject } from "../../app/selection";
import type { Store } from "../../app/store";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import { proposeFlow } from "../flow_preview/logic";
import { type Ask, askTool } from "./logic";

/** Where the dialog is: the form, then, once the project is added, its go-ahead or the presets. */
export type AddDialogState =
  { at: "form" } | { at: "go_ahead"; project: string } | { at: "presets"; project: string };

/** The project was added: it is selected, so its Flow setup page shows behind the dialog, which moves to the go-ahead. */
export function projectAdded(key: string, selection: Store<Selection> = appSelection): AddDialogState {
  selectProject(key, selection);
  return { at: "go_ahead", project: key };
}

/** The go-ahead's words arrived: the presets when no agent tool is connected, else the go-ahead. */
export function dialogOpens(state: AddDialogState, step: OnboardingFlowStep): AddDialogState {
  if (state.at === "form") return state;
  return { at: step.opens === "no_tool" ? "presets" : "go_ahead", project: state.project };
}

/**
 * Ask <Tool>: its flow agent starts, and the dialog closes onto the project's Flow setup page, which shows it at work
 * (the project stays selected). Resolves to the agent's id.
 */
export async function askFromDialog(
  project: string,
  tool: string,
  ask: Ask,
  selection: Store<Selection> = appSelection,
): Promise<string> {
  const asked = await askTool(project, tool, ask);
  selectProject(project, selection);
  return asked.id;
}

/** A preset picked in the dialog: its proposal, and the usual preview over the project's Flow setup page. */
export function presetFromDialog(project: string, file: string, ask: Ask): Promise<void> {
  return proposeFlow(project, { from: "preset", file }, ask);
}
