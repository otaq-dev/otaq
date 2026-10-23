// The epic sheet's Lead tab (T-0290; LEAD-11, LEAD-12; d0083): the lead's status, its tokens, "Changes by the lead"
// and its terminal (or Open lead, "Before the restart", or its ended terminal). What it says is Rust's
// (`src/api/lead.rs`, `lead_tab.rs`).
import type { Screen } from "../../app/registry";
import { LeadTab } from "./LeadTab";
import "./lead_tab.css";

export const leadTab: Screen = (app) => {
  app.sheetTabs.register("lead", { Component: LeadTab });
};
