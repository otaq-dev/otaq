// Area 13 · Run & Shell (T-0232): the panel's Run and Shell tabs, the run-command dialog and the header's running
// processes. What they say is Rust's (`src/api/run.rs`, `crewboard-view`'s `run_page.rs`).
import type { Screen } from "../../app/registry";
import { ProcessesOverlay } from "./Processes";
import { RunCommandDialog } from "./RunCommand";
import { RunTab } from "./RunTab";
import { ShellTab } from "./ShellTab";
import "./run.css";

export const run: Screen = (app) => {
  app.panelTabs.register("run", { Component: RunTab });
  app.panelTabs.register("shell", { Component: ShellTab });
  app.overlays.register("run_command", { Component: RunCommandDialog });
  app.overlays.register("processes", { Component: ProcessesOverlay });
};
