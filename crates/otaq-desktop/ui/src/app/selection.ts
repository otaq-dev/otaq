// The selection screens share (T-0212): the project on screen, the ticket open in the side panel and its tab, and
// the terminal that has the keyboard. One store for the whole page; each screen reads it with `useSelection()` and
// changes it with the functions below (or `selection.update`).
import type { AgentRole } from "../generated/AgentRole";
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

/** A request to give a ticket's terminal the keyboard once its Terminal tab shows it (a notification click, T-0231). */
export interface TerminalFocusRequest {
  ticket: string;
  /** Counts up with each request, so asking twice for the same ticket is two requests. */
  seq: number;
  /** The Terminal tab's side to focus (the panel's "Open terminal · Validator"); else the side the tab lands on. */
  side?: "agent" | "validator";
}

/** The newest terminal focus request; the Terminal tab (T-0224) focuses the terminal when it is for its ticket. */
export const terminalFocusRequests: Store<TerminalFocusRequest | null> =
  createStore<TerminalFocusRequest | null>(null);

export function requestTerminalFocus(
  ticket: string,
  store: Store<TerminalFocusRequest | null> = terminalFocusRequests,
  side?: TerminalFocusRequest["side"],
): void {
  store.update((last) => ({ ticket, seq: (last?.seq ?? 0) + 1, ...(side && { side }) }));
}

/** A request to show a ticket's Ticket tab in its form (Edit on the panel's bar or "…" menu, T-0222). */
export interface TicketEditRequest {
  ticket: string;
  /** Counts up with each request, so asking twice for the same ticket is two requests. */
  seq: number;
}

/** The newest edit request; the Ticket tab (T-0228) opens its form when it is for its ticket. */
export const ticketEditRequests: Store<TicketEditRequest | null> = createStore<TicketEditRequest | null>(
  null,
);

/** Shows the Ticket tab of `ticket` and asks it for its form. */
export function requestTicketEdit(
  ticket: string,
  store: Store<TicketEditRequest | null> = ticketEditRequests,
  shown: Store<Selection> = selection,
): void {
  openTicket(ticket, "ticket", shown);
  store.update((last) => ({ ticket, seq: (last?.seq ?? 0) + 1 }));
}

/** A request to open a ticket's panel with one agent row's picker open (Change agent / Change validator, T-0345). */
export interface AgentRowFocusRequest {
  ticket: string;
  role: AgentRole;
  /** Counts up with each request, so asking twice for the same row is two requests. */
  seq: number;
}

/** The newest agent row request; the panel sends it as `panel.view`'s `focus_agent_row` and takes it once shown. */
export const agentRowFocusRequests: Store<AgentRowFocusRequest | null> =
  createStore<AgentRowFocusRequest | null>(null);

/** Opens `ticket` on its Ticket tab with the `role` row's picker open and focused (T-0344 draws it). */
export function requestAgentRowFocus(
  ticket: string,
  role: AgentRole,
  store: Store<AgentRowFocusRequest | null> = agentRowFocusRequests,
  shown: Store<Selection> = selection,
): void {
  openTicket(ticket, "ticket", shown);
  store.update((last) => ({ ticket, role, seq: (last?.seq ?? 0) + 1 }));
}

/** The panel showing `ticket` takes the request meant for it, once. */
export function takeAgentRowFocus(
  ticket: string,
  store: Store<AgentRowFocusRequest | null> = agentRowFocusRequests,
): void {
  if (store.get()?.ticket === ticket) store.set(null);
}
