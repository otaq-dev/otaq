// Area 06 · Terminal tab (T-0224): the side panel's Terminal tab, showing the open ticket's agent session in T-0216's
// terminal with its header and hint strips. What it says is Rust's (`src/api/terminal_tab.rs`, `terminal_strip.rs`).
import type { Screen } from "../../app/registry";
import { TerminalTab } from "./TerminalTab";
import "./terminal_tab.css";

export const terminalTab: Screen = (app) => {
  app.panelTabs.register("terminal", { Component: TerminalTab });
};
