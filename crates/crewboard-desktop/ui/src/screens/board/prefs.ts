// What the user did to each project's board, kept while the app runs so switching projects brings it back (T-0219):
// Done folded or opened by hand, Dev unfolded, and where each column was scrolled. The page's own state (UIT-04).
import type { BoardColumnId } from "../../generated/BoardColumnId";
import { createStore, type Store } from "../../app/store";

export interface BoardPrefs {
  /** `null` follows the board's width (Rust's `done_collapsed`). */
  doneCollapsed: boolean | null;
  devUnfolded: boolean;
}

export const DEFAULT_PREFS: BoardPrefs = { doneCollapsed: null, devUnfolded: false };

export type PrefsByProject = Readonly<Record<string, BoardPrefs>>;

export const boardPrefs: Store<PrefsByProject> = createStore<PrefsByProject>({});

export function prefsOf(all: PrefsByProject, project: string): BoardPrefs {
  return all[project] ?? DEFAULT_PREFS;
}

export function changePrefs(
  project: string,
  change: (prefs: BoardPrefs) => BoardPrefs,
  store: Store<PrefsByProject> = boardPrefs,
): void {
  store.update((all) => ({ ...all, [project]: change(prefsOf(all, project)) }));
}

/** Where a project's board was scrolled: each column, and the board itself (narrow windows, the panel open). Kept
 * outside the store: scrolling re-renders nothing. */
export interface ScrollPlace {
  columns: Partial<Record<BoardColumnId, number>>;
  left: number;
}

export const scrollPlaces = new Map<string, ScrollPlace>();

export function scrollPlace(project: string): ScrollPlace {
  let place = scrollPlaces.get(project);
  if (!place) {
    place = { columns: {}, left: 0 };
    scrollPlaces.set(project, place);
  }
  return place;
}

export function rememberColumnScroll(project: string, column: BoardColumnId, top: number): void {
  scrollPlace(project).columns[column] = top;
}

export function rememberBoardScroll(project: string, left: number): void {
  scrollPlace(project).left = left;
}
