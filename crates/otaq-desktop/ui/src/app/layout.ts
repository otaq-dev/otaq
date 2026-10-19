// The window's layout state (SHL-01, SHL-05, T-0327): which main tab shows, whether the pane is hidden, the dragged
// pane and panel widths per window size class, and whether the panel is expanded. All of it is page state, so each
// window keeps its own, for the app's life. The sizes and the size classes are Rust's (`layout.view`, `panel.rs`);
// this file keeps only the moment of interaction: the drags and the switches.
import { useEffect, useState } from "react";

import type { LayoutView } from "../generated/LayoutView";
import type { MainTab } from "../generated/MainTab";
import type { SizeClass } from "../generated/SizeClass";
import type { SizeClassBound } from "../generated/SizeClassBound";
import { type PageEvents, pageEvents } from "../service";
import { request } from "../tauri";
import type { Registries } from "./registry";
import { createStore, useStore } from "./store";

/** A width dragged per size class; a class without one shows its default. */
export type ClassDrags = Partial<Record<SizeClass, number>>;

export interface ShellLayout {
  tab: MainTab;
  /** Hide / show the pane (⌘B) folded the pane to its strip. */
  paneHidden: boolean;
  paneWidths: ClassDrags;
  panelWidths: ClassDrags;
  /** Expand (`e`): the panel takes the main area and the pane's room. */
  panelExpanded: boolean;
}

export const INITIAL_LAYOUT: ShellLayout = {
  tab: "board",
  paneHidden: false,
  paneWidths: {},
  panelWidths: {},
  panelExpanded: false,
};

export const shellLayout = createStore<ShellLayout>(INITIAL_LAYOUT);

/** Rust's sizes and texts; `undefined` until `layout.view` answered (the pane and the panel wait for it). */
export const layoutView = createStore<LayoutView | undefined>(undefined);

/** The main area's width now, measured by the frame: the panel's widest is this minus what the board keeps. */
export const mainWidth = createStore<number>(0);

/**
 * The text after each tab's name ("26" after Board), set by the screen that knows it (the board, T-0328; the Files
 * view, M24) from its own Rust view.
 */
export const tabCounts = createStore<Partial<Record<MainTab, string>>>({});

let loading = false;
let following = false;

function fetchLayout(): void {
  request("layout.view").then(
    (view) => {
      layoutView.set(view as LayoutView);
    },
    (error: unknown) => {
      loading = false;
      console.error("the layout couldn't load", error);
    },
  );
}

/**
 * Asks for `layout.view` once, and again on every `settings_changed`: its strip and tabs name the user's keys
 * (KEY-08).
 */
export function loadLayout(events: PageEvents = pageEvents): void {
  if (!following) {
    following = true;
    events
      .listen((event) => {
        if (event.kind === "service" && event.name === "settings_changed") fetchLayout();
      })
      .ready.catch(() => undefined);
  }
  if (loading || layoutView.get() !== undefined) return;
  loading = true;
  fetchLayout();
}

/** The class of a window `width` px wide: the first in Rust's table it is narrower than (`layout::size_class`). */
export function sizeClass(width: number, classes: readonly SizeClassBound[]): SizeClass | null {
  const found = classes.find((c) => c.below === null || width < c.below) ?? classes.at(-1);
  return found?.class ?? null;
}

/** The pane's width in a window of `cls`: the drag kept for that class, else its default (`layout::pane_width`). */
export function paneWidth(layout: ShellLayout, cls: SizeClass, view: LayoutView): number {
  const dragged = layout.paneWidths[cls];
  return dragged === undefined ? view.pane_defaults[cls] : clampPane(dragged, view);
}

export function clampPane(width: number, view: LayoutView): number {
  return Math.min(Math.max(width, view.pane_min), view.pane_max);
}

/** What the pane's column shows: the pane, its strip (⌘B), or nothing while an open panel is expanded. */
export function paneShows(layout: ShellLayout, ticketOpen: boolean): "pane" | "strip" | "none" {
  if (layout.panelExpanded && ticketOpen) return "none";
  return layout.paneHidden ? "strip" : "pane";
}

/** `drags` with a width kept for `cls` (`null`: back to the class's default). */
export function withDrag(drags: ClassDrags, cls: SizeClass, width: number | null): ClassDrags {
  return width === null
    ? Object.fromEntries(Object.entries(drags).filter(([c]) => c !== cls))
    : { ...drags, [cls]: width };
}

/**
 * The tab the main area shows: the stored one while the view has it, else the Board, as Rust's `MainTab::open` (a
 * stored Files tab while the Files view is parked, d0087).
 */
export function shownTab(layout: ShellLayout, view: LayoutView): MainTab {
  return view.tabs.some((t) => t.tab === layout.tab) ? layout.tab : "board";
}

export function showTab(tab: MainTab): void {
  shellLayout.update((l) => ({ ...l, tab }));
}

export function togglePane(): void {
  shellLayout.update((l) => ({ ...l, paneHidden: !l.paneHidden }));
}

export function toggleExpanded(): void {
  shellLayout.update((l) => ({ ...l, panelExpanded: !l.panelExpanded }));
}

/** Board, Files view and Hide / show the pane (d0097, d0087; the Files view's key is unbound while it's parked).
 * Returns their removal. */
export function layoutShortcuts(app: Registries): () => void {
  const stops = [
    app.shortcuts.on("show_board", () => {
      showTab("board");
    }),
    app.shortcuts.on("show_files", () => {
      showTab("files");
    }),
    app.shortcuts.on("toggle_pane", togglePane),
  ];
  return () => {
    for (const stop of stops) stop();
  };
}

export function useWindowWidth(): number {
  // No document while the markup tests render.
  const [width, setWidth] = useState(() =>
    typeof document === "undefined" ? 0 : document.documentElement.clientWidth,
  );
  useEffect(() => {
    const onResize = () => {
      setWidth(document.documentElement.clientWidth);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  }, []);
  return width;
}

/** This window's size class, once `layout.view` arrived. */
export function useSizeClass(): SizeClass | null {
  const view = useStore(layoutView);
  const width = useWindowWidth();
  return view ? sizeClass(width, view.classes) : null;
}
