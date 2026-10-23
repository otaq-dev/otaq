// The epic's sheet (SHT-07, T-0469; `s25-11`): its Tickets tab. Its header pills, what its lead asks and Overview's
// stage grid are drawn by the sheet and Overview (`EpicSheet.tsx`); its Lead tab is `screens/lead_tab`'s. What it says
// is Rust's (`src/api/epic.rs`, `otaq-view`'s `epic_sheet.rs`).
import type { Screen } from "../../app/registry";
import { TicketsTab } from "./EpicSheet";

export const epic: Screen = (app) => {
  app.sheetTabs.register("tickets", { Component: TicketsTab });
};
