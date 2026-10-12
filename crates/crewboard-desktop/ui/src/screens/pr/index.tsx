// Area 07 · PR flow (T-0227): the `open_pr` overlay (Open PR, Push to PR and their states) and the PR box under the
// panel's tabs while a PR is open. The review flow (⌘↵, the panel's Approve) and the board's approve choice open the
// overlay; what it says is Rust's (`src/api/pr.rs`, `crewboard-view`'s `pr_page.rs`).
import type { Screen } from "../../app/registry";
import { OpenPrDialog, PrOpenSection } from "./Dialogs";
import "./pr.css";

export const pr: Screen = (app) => {
  app.overlays.register("open_pr", { Component: OpenPrDialog });
  app.panelSections.register("pr_open", { Component: PrOpenSection });
};
