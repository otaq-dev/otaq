// The ticket sheet's tabs and the content they show (SHT-02, T-0464). The sheet (`screens/sheet`) draws the tab bar
// from Rust's `sheet` view and shows the body registered for the shown tab in `sheetTabs` (Overview, Session, Files,
// Activity; an epic's Lead). Until each tab's own ticket lands (T-0465 Overview, T-0466 Session and Activity, T-0468
// Files, T-0469 the epic's), the sheet's screen registers bodies that show today's panel tabs, which their screens
// still register in `panelTabs`, and the panel's sections.
import type { ComponentType } from "react";

import type { PanelTab } from "../../generated/PanelTab";
import type { SheetTab } from "../../generated/SheetTab";

/** The sheet's tabs, in order (an epic's Lead takes Session's place). */
export const SHEET_TABS: readonly SheetTab[] = ["overview", "session", "files", "activity", "lead"];

/**
 * Today's panel tabs, the content the sheet's tabs show until their tickets land (`SheetTabView.contents`):
 * Terminal, Run and Shell in Session, Ticket in Overview, History in Activity, Lead in an epic's Lead; Files is the Files
 * tab's own (T-0468), which `sheetTabs` draws.
 */
export const PANEL_TABS: readonly PanelTab[] = [
  "terminal",
  "files",
  "run",
  "shell",
  "ticket",
  "history",
  "lead",
];

/** The panel's sections, in order (the epic section shows only for an epic), at the top of Overview. */
export const PANEL_SECTIONS = ["epic", "stack", "messages"] as const;

/** The box under Overview's top while the ticket's PR is open (T-0227, *05 → SHOP-26 · Approved, PR open*). */
export const PANEL_UNDER_TABS = "pr_open" as const;

export type PanelSection = (typeof PANEL_SECTIONS)[number] | typeof PANEL_UNDER_TABS;

/** A tab or section is drawn for the open ticket, by key. */
export interface TicketProps {
  ticket: string;
}

export type SheetTabEntries = Record<SheetTab, { Component: ComponentType<TicketProps> }>;
export type PanelTabEntries = Record<PanelTab, { Component: ComponentType<TicketProps> }>;
export type PanelSectionEntries = Record<PanelSection, { Component: ComponentType<TicketProps> }>;
