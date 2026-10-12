// The header's search and filter (T-0221; UI-02, EPC-01, d0058 ⌘K): what the user typed and the chips, kept for the
// project it was typed in while the app runs. Switching tabs keeps it; switching projects starts the other project
// with no filter (as the GPUI app did). Rust reads it (`board.view` hides cards, `board.search` words the field); the
// page only keeps it (UIT-04).
import type { BoardFilter } from "../../generated/BoardFilter";
import { createStore, type Store } from "../../app/store";

export interface KeptFilter extends BoardFilter {
  /** The project it was typed in. */
  project: string | null;
}

export const NO_FILTER: BoardFilter = { tokens: [], text: "" };

export const boardFilter: Store<KeptFilter> = createStore<KeptFilter>({ project: null, ...NO_FILTER });

/** The filter of `project`: the kept one if it was typed there, else none. Same object while unchanged. */
export function filterOf(kept: KeptFilter, project: string | null): BoardFilter {
  return kept.project === project ? kept : NO_FILTER;
}

export function isActive(filter: BoardFilter): boolean {
  return filter.tokens.length > 0 || filter.text.trim() !== "";
}

export function setFilter(
  project: string,
  filter: BoardFilter,
  store: Store<KeptFilter> = boardFilter,
): void {
  store.set({ project, tokens: filter.tokens, text: filter.text });
}

/** Esc: no chips, nothing typed. */
export function clearFilter(project: string, store: Store<KeptFilter> = boardFilter): void {
  setFilter(project, NO_FILTER, store);
}

/** A suggestion picked (or ↵ on a half-typed token): it becomes a chip and the field empties. */
export function addToken(project: string, token: string, store: Store<KeptFilter> = boardFilter): void {
  const now = filterOf(store.get(), project);
  const tokens = now.tokens.includes(token) ? now.tokens : [...now.tokens, token];
  setFilter(project, { tokens, text: "" }, store);
}

/** A chip's ×. */
export function removeToken(project: string, index: number, store: Store<KeptFilter> = boardFilter): void {
  const now = filterOf(store.get(), project);
  setFilter(project, { tokens: now.tokens.filter((_, i) => i !== index), text: now.text }, store);
}

/**
 * Rust's answer to what was `sent`: when it finished a token (`is:needs-me ` became a chip), the field takes its chips
 * and the rest of the text, unless the user typed on meanwhile. Otherwise the field stays as typed. `null`: no change.
 */
export function finishedTokens(sent: BoardFilter, answer: BoardFilter, now: BoardFilter): BoardFilter | null {
  const unchanged = now.text === sent.text && now.tokens.join("\n") === sent.tokens.join("\n");
  if (!unchanged || answer.tokens.join("\n") === sent.tokens.join("\n")) return null;
  return answer;
}

/** ⌘K asks the field for the keyboard: each request counts up, so pressing it twice is two requests. */
export const searchFocusRequests: Store<number> = createStore(0);

export function requestSearchFocus(store: Store<number> = searchFocusRequests): void {
  store.update((n) => n + 1);
}
