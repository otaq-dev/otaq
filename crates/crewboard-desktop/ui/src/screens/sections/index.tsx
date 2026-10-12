// The panel's sections and its History tab (T-0223; *05 · Ticket side panel*): the epic panel, the Stack list and the
// Messages between linked agents above the tabs, the History tab, and the epic dialogs (Move to epic…, Close
// epic…) that the panel's and the card's menus open. What they say is Rust's (`src/api/sections.rs`).
import type { Screen } from "../../app/registry";
import { CloseEpicDialog, MoveToEpicDialog } from "./EpicDialogs";
import { EpicSection } from "./Epic";
import { HistoryTab } from "./History";
import { MessagesSection } from "./Messages";
import { StackSection } from "./Stack";
import "./sections.css";

export const sections: Screen = (app) => {
  app.panelSections.register("epic", { Component: EpicSection });
  app.panelSections.register("stack", { Component: StackSection });
  app.panelSections.register("messages", { Component: MessagesSection });
  app.panelTabs.register("history", { Component: HistoryTab });
  app.overlays.register("move_to_epic", { Component: MoveToEpicDialog });
  app.overlays.register("close_epic", { Component: CloseEpicDialog });
};
