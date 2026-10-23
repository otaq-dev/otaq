// The project menu's choices (FED-09, d0115; `s26-08`): a click on the project's name in the title bar. What it says is
// Rust's (`shell.header`'s `menu`, `otaq-view`'s `project_menu.rs`); this file only says where each choice goes:
// Change flow… opens the flow editor over the board, Export flow… the save dialog then `settings.flow_export`, Project
// settings… the project's Settings, Open in Finder `shell.open_folder`, Open a terminal here a new terminal in the
// Terminals drawer.
import { openOverlay } from "../../app/overlays";
import type { HeaderProject } from "../../generated/HeaderProject";
import { request, saveFlowFile } from "../../tauri";
import { changeFlowOpen, type FlowEditorOpen, openFlowEditor } from "../flow_editor/logic";
import { showDrawerTab } from "../terminals";

export interface ProjectMenuRoutes {
  ask: typeof request;
  openEditor: (open: FlowEditorOpen) => void;
  saveFile: typeof saveFlowFile;
  openSettings: (project: string) => void;
  showTerminal: (pending: Promise<unknown>) => void;
}

export const PROJECT_MENU_ROUTES: ProjectMenuRoutes = {
  ask: request,
  openEditor: openFlowEditor,
  saveFile: (title, fileName) => saveFlowFile(title, fileName),
  openSettings: (project) => {
    openOverlay("settings", { project });
  },
  showTerminal: showDrawerTab,
};

/** The chosen item, by its id from Rust. Export flow… with its dialog cancelled does nothing. */
export async function projectMenuChoice(
  project: HeaderProject,
  id: string,
  routes: ProjectMenuRoutes = PROJECT_MENU_ROUTES,
): Promise<void> {
  switch (id) {
    case "change_flow":
      routes.openEditor(changeFlowOpen(project.key));
      return;
    case "export_flow": {
      const path = await routes.saveFile(project.menu.export_title, project.menu.export_file);
      if (path === null) return;
      await routes.ask("settings.flow_export", { project: project.key, path });
      return;
    }
    case "project_settings":
      routes.openSettings(project.key);
      return;
    case "open_folder":
      await routes.ask("shell.open_folder", { project: project.key });
      return;
    case "open_terminal":
      routes.showTerminal(routes.ask("drawer.new", { project: project.key }));
      return;
    default:
      return;
  }
}
