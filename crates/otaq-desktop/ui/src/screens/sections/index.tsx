// Overview's sections (T-0223): the epic dialogs (Move to epic…, Close epic…) that the sheet's and the card's menus
// open. DEPENDENCIES is `screens/dependencies` (T-0463); History and the Messages between linked agents are the
// sheet's Activity (`screens/activity`, T-0466, T-0469); an epic's children its own Overview and Tickets tab
// (`screens/epic`, SHT-07). What they say is Rust's (`src/api/sections.rs`).
import type { Screen } from "../../app/registry";
import { CloseEpicDialog, MoveToEpicDialog } from "./EpicDialogs";
import "./sections.css";

export const sections: Screen = (app) => {
  app.overlays.register("move_to_epic", { Component: MoveToEpicDialog });
  app.overlays.register("close_epic", { Component: CloseEpicDialog });
};
