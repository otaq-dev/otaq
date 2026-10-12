// The frame's slots (*02 · App shell → Window anatomy*), each filled by one screen.
import type { ComponentType } from "react";

export const SLOT_IDS = [
  /** The start of the header row: the inset macOS traffic lights (T-0214). */
  "title_bar_start",
  /** The header (T-0219). The header row is also the title bar. */
  "header",
  /** The end of the header row: the Windows and Linux caption buttons (T-0214). */
  "title_bar_end",
  /** The project rail (T-0219). */
  "rail",
  /** The banner area above board and panel, pushing them down (T-0231). */
  "banners",
  /** The board (T-0220). */
  "board",
  /** The side panel, shown while a ticket is open (T-0222). It sets its own width. */
  "panel",
  /** The toasts, over everything (T-0231). */
  "toasts",
] as const;

export type SlotId = (typeof SLOT_IDS)[number];

export type SlotEntries = Record<SlotId, { Component: ComponentType }>;
