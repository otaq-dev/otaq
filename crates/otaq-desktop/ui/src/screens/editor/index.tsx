// Area 08 · Ticket editor (T-0228): quick-create and the full form as overlays (`new_ticket`, opened by ⌘N, the
// board's + and Add ticket to epic; `edit_ticket`), the Ticket tab, the editor in read mode inside the panel, and the
// description over the whole window (`full_view`, T-0260), opened from either, and the report view over the panel
// (`report`, CMT-12) that every report chip's Open shows.
// What they say and every rule is Rust's (`src/api/editor.rs`, the `otaq-view` editor view models).
import type { Screen } from "../../app/registry";
import { FullViewHost } from "../../markdown";
import { EditTicket, NewTicket } from "./Modal";
import { registerReportOpener, ReportViewOverlay } from "./ReportView";
import { TicketTab } from "./TicketTab";
import "./editor.css";
import "./report.css";

export const editor: Screen = (app) => {
  app.overlays.register("new_ticket", { Component: NewTicket });
  app.overlays.register("edit_ticket", { Component: EditTicket });
  app.overlays.register("full_view", { Component: FullViewHost });
  // Open on a report chip (the thread's, a box's, History's): the report view (CMT-12).
  app.overlays.register("report", { Component: ReportViewOverlay });
  registerReportOpener();
  app.panelTabs.register("ticket", { Component: TicketTab });
};
