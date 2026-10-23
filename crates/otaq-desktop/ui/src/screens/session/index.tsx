// Area 25 · the sheet's Session (SHT-04, T-0466; canvas `s25-06`): the switch Agent · <check's agent> · Run · Shell over
// the ticket's terminals (its agent's and its check's, `terminal_tab.view`), its Run and its Shell; and Run's
// run-command dialog and the header's running processes (area 13, T-0232). What they say is Rust's
// (`src/api/terminal_tab.rs`, `src/api/run.rs`, `otaq-view`'s `session_tab.rs`, `run_page.rs`).
import type { Screen } from "../../app/registry";
import { ProcessesOverlay } from "./Processes";
import { RunCommandDialog } from "./RunCommand";
import "./run.css";
import { SessionTab } from "./Session";
import "./session.css";
import "./terminal_tab.css";

export const session: Screen = (app) => {
  app.sheetTabs.register("session", { Component: SessionTab });
  app.overlays.register("run_command", { Component: RunCommandDialog });
  app.overlays.register("processes", { Component: ProcessesOverlay });
};
