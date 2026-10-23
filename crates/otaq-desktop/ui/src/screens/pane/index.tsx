// The Project agent panel (T-0330, T-0458; SHL-04, SHB-03, SHB-04, d0087, d0113): the `pane` slot, Open / close the
// Project agent (⌘L: opens and focuses it, or closes it from inside) and New Project agent conversation (⇧⌘L: the empty
// panel; nothing starts). What it says is Rust's (`src/api/pane.rs`, `otaq-view`'s `agent_pane.rs`).
import type { Screen } from "../../app/registry";
import { focusPane, inPanel, newProjectAgent, PANE_ROUTES } from "./logic";
import { Pane } from "./Pane";
import "./pane.css";

export const pane: Screen = (app) => {
  app.slots.register("agent_panel", { Component: Pane });
  app.shortcuts.on("focus_pane", () =>
    focusPane(PANE_ROUTES, typeof document !== "undefined" && inPanel(document.activeElement)),
  );
  app.shortcuts.on("new_project_agent", () => newProjectAgent());
};
