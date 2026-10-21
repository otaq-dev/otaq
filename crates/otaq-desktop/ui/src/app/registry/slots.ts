// The frame's slots (*02 → Row · Window 1440 × 900 (0086 · V)*), each filled by one screen.
import type { ComponentType } from "react";

export const SLOT_IDS = [
  /** The start of the header row: the inset macOS traffic lights (T-0214). */
  "title_bar_start",
  /** The header (T-0219). The header row is also the title bar. */
  "header",
  /** The header's search and filter field (T-0221), drawn by the header in place of its search entry. */
  "header_search",
  /** The end of the header row: the Windows and Linux caption buttons (T-0214). */
  "title_bar_end",
  /** The project rail (T-0219). */
  "rail",
  /** The banner area across the window under the header, pushing everything under it down (T-0231). */
  "banners",
  /**
   * The Project agent pane between the rail and the main area (SHL-04, T-0330). The frame sets its width and keeps it
   * mounted while ⌘B folds it to the strip; it reads `shellLayout.paneHidden` if it needs to know.
   */
  "pane",
  /** Under the strip's bot icon while the pane is folded: the turquoise dot while an agent waits (T-0330). */
  "pane_strip",
  /** The board, under the main area's Board tab (T-0220, T-0328). */
  "board",
  /** The Files view, under the Files tab (M24); until then a quiet line. */
  "files",
  /**
   * The side panel, shown over the main area's right side while a ticket is open (T-0222, SHL-05). It sets its own
   * width; expanded, it fills the main area.
   */
  "panel",
  /** The toasts, over everything (T-0231). */
  "toasts",
  /**
   * Always mounted in the main area's overlay layer, drawing nothing of its own: the flow preview opens itself there
   * when a proposal arrives (T-0420, FLW-04).
   */
  "main_area",
] as const;

export type SlotId = (typeof SLOT_IDS)[number];

export type SlotEntries = Record<SlotId, { Component: ComponentType }>;
