// The sheet's Overview (SHT-03, RPT-02; T-0465): what the ticket needs from you, the description in place, the
// comments and the Attachments column. What it says is Rust's (`src/api/overview.rs`, `view_models/overview.rs`).
import type { Screen } from "../../app/registry";
import { OverviewTab } from "./Overview";
import "./overview.css";

export const overview: Screen = (app) => {
  app.sheetTabs.register("overview", { Component: OverviewTab });
};
