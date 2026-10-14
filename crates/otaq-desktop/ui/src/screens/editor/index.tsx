// Area 08 · Ticket editor (T-0228): quick-create and the full form as overlays (`new_ticket`, opened by ⌘N, the
// board's + and Add ticket to epic; `edit_ticket`), and the Ticket tab, the editor in read mode inside the panel.
// What they say and every rule is Rust's (`src/api/editor.rs`, the `otaq-view` editor view models).
import type { Screen } from "../../app/registry";
import { EditTicket, NewTicket } from "./Modal";
import { TicketTab } from "./TicketTab";
import "./editor.css";

export const editor: Screen = (app) => {
  app.overlays.register("new_ticket", { Component: NewTicket });
  app.overlays.register("edit_ticket", { Component: EditTicket });
  app.panelTabs.register("ticket", { Component: TicketTab });
};
