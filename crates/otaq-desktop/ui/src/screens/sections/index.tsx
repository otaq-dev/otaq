// Overview's sections (T-0223): the epic panel, the Stack list and the Messages between linked agents, and the epic
// dialogs (Move to epic…, Close epic…) that the sheet's and the card's menus open. History is the sheet's Activity
// (`screens/activity`, T-0466). What they say is Rust's (`src/api/sections.rs`).
import type { Screen } from "../../app/registry";
import { CloseEpicDialog, MoveToEpicDialog } from "./EpicDialogs";
import { EpicSection } from "./Epic";
import { MessagesSection } from "./Messages";
import { StackSection } from "./Stack";
import "./sections.css";

export const sections: Screen = (app) => {
  app.panelSections.register("epic", { Component: EpicSection });
  app.panelSections.register("stack", { Component: StackSection });
  app.panelSections.register("messages", { Component: MessagesSection });
  app.overlays.register("move_to_epic", { Component: MoveToEpicDialog });
  app.overlays.register("close_epic", { Component: CloseEpicDialog });
};
