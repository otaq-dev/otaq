// The selection screens share (T-0212): the project on screen, the ticket open in the side panel and its tab, and
// the terminal that has the keyboard. One store for the whole page; each screen reads it with `useSelection()` and
// changes it with the functions below (or `selection.update`).
import type { PanelTab } from "../generated/PanelTab";
import { createStore, type Store, useStore } from "./store";

export interface Selection {
  /** The project on screen, by key (`SHOP`); `null` before a project is added. */
  project: string | null;
  /** The ticket open in the side panel, by key (`SHOP-22`); `null` while the panel is closed. */
  ticket: string | null;
  /** The panel's tab. */
  tab: PanelTab;
  /** The session whose terminal has the keyboard (SES-10's focused-terminal rule); `null` when none has. */
  focusedTerminal: string | null;
}

export const NOTHING_SELECTED: Selection = {
  project: null,
  ticket: null,
  tab: "terminal",
  focusedTerminal: null,
};

export function createSelection(initial: Selection = NOTHING_SELECTED): Store<Selection> {
  return createStore(initial);
}

/** The page's selection. */
export const selection = createSelection();

export function useSelection(): Selection {
  return useStore(selection);
}

/** What each project showed when the user left it, per store: switching back brings it back (T-0219). */
const left = new WeakMap<Store<Selection>, Map<string, Pick<Selection, "ticket" | "tab">>>();

/** Shows another project as the user left it: its open ticket and tab; a project not shown before starts closed. */
export function selectProject(project: string, store: Store<Selection> = selection): void {
  const kept = left.get(store) ?? new Map<string, Pick<Selection, "ticket" | "tab">>();
  left.set(store, kept);
  store.update((s) => {
    if (s.project === project) return s;
    if (s.project !== null) kept.set(s.project, { ticket: s.ticket, tab: s.tab });
    const back = kept.get(project) ?? { ticket: null, tab: s.tab };
    return { ...s, project, ...back, focusedTerminal: null };
  });
}

/** Opens `ticket` in the side panel, on `tab` if given, else on the tab already shown. */
export function openTicket(ticket: string, tab?: PanelTab, store: Store<Selection> = selection): void {
  store.update((s) => ({ ...s, ticket, tab: tab ?? s.tab }));
}

/** Closes the side panel. */
export function closeTicket(store: Store<Selection> = selection): void {
  store.update((s) => ({ ...s, ticket: null, focusedTerminal: null }));
}

export function showTab(tab: PanelTab, store: Store<Selection> = selection): void {
  store.update((s) => ({ ...s, tab }));
}

/** Records which terminal has the keyboard (`null`: none). */
export function focusTerminal(session: string | null, store: Store<Selection> = selection): void {
  store.update((s) => (s.focusedTerminal === session ? s : { ...s, focusedTerminal: session }));
}
