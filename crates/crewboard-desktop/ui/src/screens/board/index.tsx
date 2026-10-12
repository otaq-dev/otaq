// Area 03 · Board (T-0220): the board slot, ⌘N (a new Backlog ticket, as the GPUI board's) and, inside the board,
// ⇧⌘D and Esc during a drag. What it says is Rust's (`src/api/board.rs`, the `crewboard-view` board view models).
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { Board } from "./Board";
import "./board.css";

export const board: Screen = (app) => {
  app.slots.register("board", { Component: Board });
  app.shortcuts.on("new_ticket", () => {
    openOverlay("new_ticket");
  });
};
