// The Project agent pane (T-0330; SHL-04, SHL-09, d0087): the `pane` slot, the strip's waiting dot (`pane_strip`),
// ⌘L (focus the pane, showing it first) and ⇧⌘A (the empty pane; nothing starts). What it says is Rust's
// (`src/api/pane.rs`, `otaq-view`'s `agent_pane.rs`).
import type { Screen } from "../../app/registry";
import { focusPane, newProjectAgent } from "./logic";
import { Pane, PaneStripDot } from "./Pane";
import "./pane.css";

export const pane: Screen = (app) => {
  app.slots.register("pane", { Component: Pane });
  app.slots.register("pane_strip", { Component: PaneStripDot });
  app.shortcuts.on("focus_pane", () => focusPane());
  app.shortcuts.on("new_project_agent", () => newProjectAgent());
};
