// Area 25 · the ticket sheet's Files tab (FTB-01…FTB-05, SHT-06; T-0468): the tree, the search, a file, All changes
// with line comments, and ⌥⌘P (Search the ticket's files). The review bar under every tab is the sheet's
// (`ReviewBar`). What it says is Rust's (`src/api/files.rs`, `otaq-view`'s `files_tab.rs`); lines arrive highlighted
// by `highlight.rs`.
import type { Screen } from "../../app/registry";
import { selection, showTab } from "../../app/selection";
import { FilesTab } from "./FilesTab";
import "./files.css";
import { searchFocus } from "./logic";

export const files: Screen = (app) => {
  app.sheetTabs.register("files", { Component: FilesTab });
  // ⌥⌘P: the open ticket's Files tab with its search box focused (FTB-02).
  app.shortcuts.on("go_to_file", () => {
    if (selection.get().ticket === null) return false;
    showTab("files");
    searchFocus.update((n) => n + 1);
    return true;
  });
};
