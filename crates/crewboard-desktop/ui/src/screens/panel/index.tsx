// Area 05 · Ticket side panel (T-0222): the panel slot, its Close ticket and Submit for review dialogs, and while it
// shows its keys (Esc, ⇧⌘E, ⌘[ / ⌘], ⌘⌥1–6). Tabs and sections are filled by their own screens through the
// registries. What it says is Rust's (`src/api/panel.rs`, the `crewboard-view` panel view models).
import type { Screen } from "../../app/registry";
import { CloseTicketDialog, SubmitForReviewDialog } from "./Dialogs";
import { SidePanel } from "./Panel";
import "./panel.css";

export const panel: Screen = (app) => {
  app.slots.register("panel", { Component: SidePanel });
  app.overlays.register("close_ticket", { Component: CloseTicketDialog });
  app.overlays.register("submit_for_review", { Component: SubmitForReviewDialog });
};
