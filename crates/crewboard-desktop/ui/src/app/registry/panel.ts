// The side panel's tabs and sections (UI-04). The panel screen (T-0222) draws the tab strip, with each tab's label
// and indicator from the Rust panel view model, and shows the registered tab; each tab's screen registers its own.
import type { ComponentType } from "react";

import type { PanelTab } from "../../generated/PanelTab";

/** Each tab's number: its place in the strip and its ⌘⌥ key (d0058). Terminal · Diff · Run · Shell · Ticket · History. */
export const PANEL_TAB_NUMBER: Record<PanelTab, number> = {
  terminal: 1,
  diff: 2,
  run: 3,
  shell: 4,
  ticket: 5,
  history: 6,
};

export const PANEL_TABS: readonly PanelTab[] = (Object.keys(PANEL_TAB_NUMBER) as PanelTab[]).sort(
  (a, b) => PANEL_TAB_NUMBER[a] - PANEL_TAB_NUMBER[b],
);

/** The tab with this number (`switch_tab`'s), if any. */
export function panelTabNumbered(n: number): PanelTab | undefined {
  return PANEL_TABS.find((tab) => PANEL_TAB_NUMBER[tab] === n);
}

/** The panel's sections, in order (the epic panel shows only for an epic). */
export const PANEL_SECTIONS = ["epic", "stack", "messages"] as const;

/** The bar on top of the panel during a ⌘J run through the inbox (T-0230), registered like a section. */
export const PANEL_TOP = "inbox_run" as const;

/** The box under the tab strip while the ticket's PR is open (T-0227, *05 → SHOP-26 · Approved, PR open*). */
export const PANEL_UNDER_TABS = "pr_open" as const;

export type PanelSection = (typeof PANEL_SECTIONS)[number] | typeof PANEL_TOP | typeof PANEL_UNDER_TABS;

/** A tab or section is drawn for the open ticket, by key. */
export interface TicketProps {
  ticket: string;
}

export type PanelTabEntries = Record<PanelTab, { Component: ComponentType<TicketProps> }>;
export type PanelSectionEntries = Record<PanelSection, { Component: ComponentType<TicketProps> }>;
