// Area 05 · the ticket's actions (T-0222): its Close ticket and Submit for review dialogs. The sheet
// (`screens/sheet`, T-0464) fills the panel slot and shows this folder's action bar, answer box and callouts in its
// Overview until T-0465. What they say is Rust's (`src/api/panel.rs`, the `otaq-view` panel view models).
import type { Screen } from "../../app/registry";
import { CloseTicketDialog, SubmitForReviewDialog } from "./Dialogs";
import "./panel.css";

export const panel: Screen = (app) => {
  app.overlays.register("close_ticket", { Component: CloseTicketDialog });
  app.overlays.register("submit_for_review", { Component: SubmitForReviewDialog });
};
