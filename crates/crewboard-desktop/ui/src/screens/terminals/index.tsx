// Terminals & agents (T-0242, the UI part of T-0203; d0068): the drawer overlay, the header's button (drawn by the
// shell's header, T-0219) and the keys ⇧⌘T (the last-used terminal, or a new one) and ⇧⌘A (a new Project agent).
// What they say is Rust's (`src/api/drawer.rs`, `crewboard-view`'s `project_terminal.rs`).
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { selection } from "../../app/selection";
import { request } from "../../tauri";
import { TerminalsDrawer } from "./Drawer";
import { requestTab } from "./logic";
import "./terminals.css";

export { TerminalsButton } from "./Button";

/** Opens the drawer on the tab a request answers with (the request picks or opens it on the Rust side). */
export function showDrawerTab(pending: Promise<unknown>): void {
  openOverlay("terminals_agents");
  pending.then(
    (tab) => {
      if (typeof tab === "string") requestTab(tab);
    },
    () => undefined,
  );
}

export const terminals: Screen = (app) => {
  app.overlays.register("terminals_agents", { Component: TerminalsDrawer });
  app.shortcuts.on("project_terminal", () => {
    const { project } = selection.get();
    if (project === null) return false;
    showDrawerTab(request("drawer.last_terminal", { project }));
    return true;
  });
  app.shortcuts.on("new_project_agent", () => {
    const { project } = selection.get();
    if (project === null) return false;
    showDrawerTab(request("drawer.new", { project, item: "new-agent" }));
    return true;
  });
};
