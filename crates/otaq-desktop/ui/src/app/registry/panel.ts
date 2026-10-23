// The ticket sheet's tabs and Overview's sections (SHT-02, T-0464). The sheet (`screens/sheet`) draws the tab bar from
// Rust's `sheet` view and shows the body registered for the shown tab in `sheetTabs`: Overview (`screens/overview`),
// Session (`screens/session`), Files (`screens/files`), Activity (`screens/activity`) and an epic's Lead
// (`screens/lead_tab`).
import type { ComponentType } from "react";

import type { SheetTab } from "../../generated/SheetTab";

/** The sheet's tabs, in order (an epic's Lead takes Session's place). */
export const SHEET_TABS: readonly SheetTab[] = ["overview", "session", "files", "activity", "lead"];

/** Overview's sections, in order (the epic section shows only for an epic). */
export const PANEL_SECTIONS = ["epic", "stack", "messages"] as const;

/** The box under Overview's top while the ticket's PR is open (T-0227, *05 → SHOP-26 · Approved, PR open*). */
export const PANEL_UNDER_TABS = "pr_open" as const;

export type PanelSection = (typeof PANEL_SECTIONS)[number] | typeof PANEL_UNDER_TABS;

/** A tab or section is drawn for the open ticket, by key. */
export interface TicketProps {
  ticket: string;
}

export type SheetTabEntries = Record<SheetTab, { Component: ComponentType<TicketProps> }>;
export type PanelSectionEntries = Record<PanelSection, { Component: ComponentType<TicketProps> }>;
