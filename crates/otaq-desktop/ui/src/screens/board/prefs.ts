// Where each project's board was scrolled, kept while the app runs so switching projects brings it back (T-0219):
// each column, and the board itself. Everything else about how a board looks (Board or List, folds, lanes) is a view
// preference the service keeps (SHB-05, `ProjectSettings.view`), read with `board.view` and changed with
// `board.set_view`.
import type { BoardColumnId } from "../../generated/BoardColumnId";

/** Where a project's board was scrolled: each column, and the board itself. Kept outside any store: scrolling
 * re-renders nothing. */
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
