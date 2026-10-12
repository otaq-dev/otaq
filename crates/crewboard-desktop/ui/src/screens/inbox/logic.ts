// The inbox's own decisions, as plain functions the tests drive without a DOM: which rows ↑↓ walk, the answer box's
// pick and Send, what a row's click and ⌘J open, and what each row button sends. Every text comes from Rust
// (`inbox.view`, `crewboard-view`'s `inbox_page.rs`).
import type { OverlayStack } from "../../app/overlays";
import {
  closeTicket,
  openTicket,
  requestTerminalFocus,
  type Selection,
  selectProject,
  type TerminalFocusRequest,
} from "../../app/selection";
import { createStore, type Store } from "../../app/store";
import type { InboxNext } from "../../generated/InboxNext";
import type { InboxOption } from "../../generated/InboxOption";
import type { InboxRowAction } from "../../generated/InboxRowAction";
import type { InboxRowView } from "../../generated/InboxRowView";
import type { InboxView } from "../../generated/InboxView";
import type { PanelTab } from "../../generated/PanelTab";
import { requestTab, type TabRequest } from "../terminals/logic";

/** The rows ↑↓ walk and ↵ opens: the waiting ones, in order (resolved rows are fading out). */
export function waitingRows(view: InboxView): InboxRowView[] {
  return view.groups.flatMap((g) => g.rows).filter((r) => !r.resolved);
}

/** ↑ / ↓: one row up or down, staying inside the list. */
export function step(selected: number, by: number, count: number): number {
  if (count === 0) return 0;
  return Math.min(count - 1, Math.max(0, selected + by));
}

/** The answer picked when the box opens: the recommended option (Rust marks it picked), else nothing. */
export function initialPick(options: readonly InboxOption[]): number | null {
  return options.find((o) => o.picked)?.number ?? null;
}

/** A number key in the box: the option with that number, if there is one. */
export function pickByKey(key: string, options: readonly InboxOption[]): number | null {
  const n = Number(key);
  return Number.isInteger(n) && options.some((o) => o.number === n) ? n : null;
}

/** Whether the pick is Other…, whose field then takes the typing. */
export function picksOther(pick: number | null, options: readonly InboxOption[]): boolean {
  return options.some((o) => o.other && o.number === pick);
}

/** Send is enabled: an option is picked (Other… with text), or a free-text box has text. Rust checks it again. */
export function canSend(pick: number | null, text: string, options: readonly InboxOption[]): boolean {
  if (options.length === 0) return text.trim() !== "";
  if (pick === null) return false;
  return !picksOther(pick, options) || text.trim() !== "";
}

/** Where opening a row or ⌘J lands: the selection, the terminal focus requests, the overlays and the Terminals &
 * agents drawer's tab requests. */
export interface OpenRoutes {
  selection: Store<Selection>;
  focus: Store<TerminalFocusRequest | null>;
  overlays: Pick<OverlayStack, "open" | "close">;
  tabs: Store<TabRequest | null>;
}

/** Opens a ticket of any project on `tab`, with its terminal focused when that is the tab; the inbox closes. */
export function openItem(project: string, key: string, tab: PanelTab, routes: OpenRoutes): void {
  routes.overlays.close("inbox");
  selectProject(project, routes.selection);
  openTicket(key, tab, routes.selection);
  if (tab === "terminal") requestTerminalFocus(key, routes.focus);
}

/** Opens a row: a ticket on its tab, or a Project agent's tab in the Terminals & agents drawer of its project (its
 * terminal focused there); the inbox closes. Resolved rows open nothing. */
export function openRow(row: InboxRowView, routes: OpenRoutes): void {
  if (row.resolved) return;
  if (row.agent === null) {
    openItem(row.project, row.key, row.tab, routes);
    return;
  }
  routes.overlays.close("inbox");
  selectProject(row.project, routes.selection);
  routes.overlays.open("terminals_agents");
  requestTab(row.agent.tab, routes.tabs);
}

/** Counts up whenever the ⌘J run changes, so the run bar asks again. */
export const runChanges: Store<number> = createStore(0);

/** What ⌘J answered: the next item opens with its terminal focused, or "All clear" shows over the board. */
export function landNext(next: InboxNext, routes: OpenRoutes, changes: Store<number> = runChanges): void {
  switch (next.kind) {
    case "open":
      openItem(next.project, next.key, next.tab, routes);
      break;
    case "all_clear": {
      routes.overlays.close("inbox");
      closeTicket(routes.selection);
      routes.overlays.open("all_clear", next);
      break;
    }
  }
  changes.update((n) => n + 1);
}

/** A row's button, as `inbox.act` takes it; a new kind from Rust fails the linter here until it has a case. */
export function describeAction(action: InboxRowAction): string {
  switch (action.kind) {
    case "retry_start":
    case "usage_limit":
      return action.kind;
    case "reopen_parent":
    case "remove_dependency":
      return `${action.kind} ${action.parent}`;
    case "resume_target":
      return `${action.kind} ${action.message}`;
    case "move_to_review_anyway":
    case "another_round":
    case "answer_agent":
    case "retry_validation":
    case "retry_merge":
    case "move_to_review":
      return action.kind;
  }
}

/** "Answer the agent…" opens the row's note box; its Send carries the note. Every other button acts at once. */
export function opensNote(action: InboxRowAction): boolean {
  return action.kind === "answer_agent";
}
