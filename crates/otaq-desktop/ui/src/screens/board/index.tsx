// Area 03 · Board (T-0220, T-0221): the board slot, ⌘N (a new Backlog ticket, as the GPUI board's) and, inside the
// board, ⇧⌘D and Esc during a drag; the header's search and filter field and ⌘K; the card's right-click menu and the
// delete confirmation every place that deletes opens (`delete_ticket`). What it says is Rust's (`src/api/board.rs`,
// the `otaq-view` board, filter, card-menu and delete view models).
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { selection } from "../../app/selection";
import { Board } from "./Board";
import "./board.css";
import { DeleteDialog } from "./DeleteDialog";
import { requestSearchFocus } from "./filter";
import { SearchField } from "./Search";

export const board: Screen = (app) => {
  app.slots.register("board", { Component: Board });
  app.slots.register("header_search", { Component: SearchField });
  app.overlays.register("delete_ticket", { Component: DeleteDialog });
  app.shortcuts.on("new_ticket", () => {
    openOverlay("new_ticket");
  });
  // ⌘K: the field takes the keyboard (only while a project is shown).
  app.shortcuts.on("search", () => {
    if (selection.get().project === null) return false;
    requestSearchFocus();
    return true;
  });
};
