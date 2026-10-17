// The app's own way of carrying out a tool problem's button (`toolProblems.ts`): the service, Settings, the Terminals
// drawer, the agent row's picker and the pane.
import { openOverlay } from "../../app/overlays";
import { requestAgentRowFocus } from "../../app/selection";
import type { ToolProblemAction } from "../../generated/ToolProblemAction";
import { request } from "../../tauri";
import { startNewAgentOn } from "../pane/logic";
import { showDrawerTab } from "../terminals";
import { runToolProblemAction, type ToolProblemPlace, type ToolProblemRunner } from "./toolProblems";

export const APP_RUNNER: ToolProblemRunner = {
  request,
  openSettings: (params) => {
    openOverlay("settings", params);
  },
  showDrawerTab,
  focusAgentRow: (ticket, role) => {
    requestAgentRowFocus(ticket, role);
  },
  newAgentOn: (project, agent) => {
    startNewAgentOn(project, agent);
  },
};

/** A tool problem's button, done in the app. */
export function toolProblemAction(action: ToolProblemAction, place: ToolProblemPlace): void {
  runToolProblemAction(action, place, APP_RUNNER);
}
