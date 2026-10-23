// Area 25 · the ticket sheet (SHT-01, SHT-02; T-0464): the panel slot's sheet and its tab bodies. Each tab's own
// ticket registers its body in place of the interim one here (T-0465 Overview, T-0466 Session and Activity, T-0468
// Files, T-0469 the epic's Lead). What it says is Rust's (`src/api/panel.rs`'s `sheet`, `view_models/sheet.rs`).
import type { Screen } from "../../app/registry";
import { TicketSheet } from "./Sheet";
import "./sheet.css";
import { ActivityTab, FilesTab, LeadSheetTab, OverviewTab, SessionTab } from "./tabs";

export const sheet: Screen = (app) => {
  app.slots.register("panel", { Component: TicketSheet });
  app.sheetTabs.register("overview", { Component: OverviewTab });
  app.sheetTabs.register("session", { Component: SessionTab });
  app.sheetTabs.register("files", { Component: FilesTab });
  app.sheetTabs.register("activity", { Component: ActivityTab });
  app.sheetTabs.register("lead", { Component: LeadSheetTab });
};
