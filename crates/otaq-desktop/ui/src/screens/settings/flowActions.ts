// Settings → Flow's actions (T-0422; FLW-07…FLW-11): what each button asks of Rust, and where the user goes next.
// Every action but Export asks for a proposal (`settings.flow_propose`), closes Settings and opens the preview (T-0420,
// `openFlowPreview`: it is drawn under the window's overlays, and the keyboard moves into it). Export and Import go through
// the system's save and open dialogs. Change with the Project agent starts a Project agent with Rust's first message
// (`/flow`), as the empty pane's Send would: the click is the go-ahead (d0087).
import { closeOverlay } from "../../app/overlays";
import type { FlowSettings } from "../../generated/FlowSettings";
import type { FlowSource } from "../../generated/FlowSource";
import { pickFlowFile, request, saveFlowFile } from "../../tauri";
import { openFlowPreview } from "../flow_preview/logic";
import { PANE_ROUTES, type PaneRoutes, sendFirstMessage, showPane } from "../pane/logic";
import { selectProject } from "../../app/selection";

export interface FlowRoutes {
  ask: typeof request;
  /** Closes the Settings overlay, so the preview over the main area shows. */
  closeSettings: () => void;
  /** Opens the preview of the project's open proposal, the keyboard in it. */
  openPreview: (project: string) => void;
  pickFile: typeof pickFlowFile;
  saveFile: typeof saveFlowFile;
  pane: PaneRoutes;
}

export const FLOW_ROUTES: FlowRoutes = {
  ask: request,
  closeSettings: () => {
    closeOverlay("settings");
  },
  openPreview: (project) => {
    openFlowPreview(project);
  },
  pickFile: (title) => pickFlowFile(title),
  saveFile: (title, fileName) => saveFlowFile(title, fileName),
  pane: PANE_ROUTES,
};

/** A proposal from `source` (a preset, another project, the previous flow, the default, the shipped agents file), then
 * its preview. A refusal rejects, and Settings stays open to show it. */
export async function proposeFrom(
  project: string,
  source: FlowSource,
  routes: FlowRoutes = FLOW_ROUTES,
): Promise<void> {
  await routes.ask("settings.flow_propose", { project, source });
  routes.closeSettings();
  routes.openPreview(project);
}

/** Import flow…: the open dialog, then the file's proposal. Cancelling the dialog does nothing. */
export async function importFlow(section: FlowSettings, routes: FlowRoutes = FLOW_ROUTES): Promise<void> {
  const path = await routes.pickFile(section.import_title);
  if (path === null) return;
  await routes.ask("settings.flow_import", { project: section.project, path });
  routes.closeSettings();
  routes.openPreview(section.project);
}

/** Export flow…: the save dialog offering "shop-web-flow.yaml", then Rust writes the flow document there. Settings
 * stays open. Whether a file was saved. */
export async function exportFlow(section: FlowSettings, routes: FlowRoutes = FLOW_ROUTES): Promise<boolean> {
  const path = await routes.saveFile(section.export_title, section.export_file);
  if (path === null) return false;
  await routes.ask("settings.flow_export", { project: section.project, path });
  return true;
}

/** Change with the Project agent: Settings closes, the pane shows and a new Project agent starts with `/flow`. */
export function changeWithProjectAgent(
  section: FlowSettings,
  routes: FlowRoutes = FLOW_ROUTES,
): Promise<string | null> {
  routes.closeSettings();
  selectProject(section.project, routes.pane.selection);
  showPane(routes.pane);
  return sendFirstMessage(section.project, section.change_message, routes.ask, routes.pane);
}

/** The ⋯ menu's choice, by its id from Rust. */
export function menuChoice(
  section: FlowSettings,
  id: string,
  routes: FlowRoutes = FLOW_ROUTES,
): Promise<unknown> {
  switch (id) {
    case "import":
      return importFlow(section, routes);
    case "export":
      return exportFlow(section, routes);
    case "back":
      return proposeFrom(section.project, { from: "previous" }, routes);
    case "reset":
      return proposeFrom(section.project, { from: "default" }, routes);
    default:
      return Promise.resolve();
  }
}
