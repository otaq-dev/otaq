// The project Files view's interaction state (UIT-04): the filter, the folders opened, the file picked, the source
// picked in the picker and what is typed in it, per project while the app runs. Everything worded or counted comes
// from Rust (`files_view.rs`).
import { createStore, type Store } from "../../app/store";
import type { FilesViewFilter } from "../../generated/FilesViewFilter";
import type { FilesViewPicker } from "../../generated/FilesViewPicker";
import type { FilesViewPickerSection } from "../../generated/FilesViewPickerSection";
import { folders, searchFocus } from "../files/logic";

/** While an agent works, the view asks again this often: commits have no event (FIL-10). */
export const POLL_MS = 5_000;

/** The service events that change the tickets' files: a turn ends, a merge (FIL-10). */
export const PROJECT_FILES_EVENTS = ["card_status_changed", "ticket_changed"] as const;

export interface ViewState {
  filter: FilesViewFilter;
  open: ReadonlySet<string>;
  /** The file picked in the tree or the search. */
  file: string | null;
  /** A ticket or epic picked in the picker (its key); `null`: the base branch with every ticket's changes. */
  picked: string | null;
}

export const FIRST: ViewState = { filter: "all", open: new Set(), file: null, picked: null };

/** Each project's view while the app runs, so leaving the view and coming back keeps it. */
export const viewStates: Store<ReadonlyMap<string, ViewState>> = createStore<ReadonlyMap<string, ViewState>>(
  new Map(),
);

/** Picking a file: All, the file selected and the folders above it open. */
export function pickFile(state: ViewState, path: string): ViewState {
  const open = new Set(state.open);
  for (const folder of folders(path)) open.add(folder);
  return { ...state, filter: "all", file: path, open };
}

/** The picker's sections with only the rows whose key or title holds `query` (any case); empty sections go. */
export function pickerSections(picker: FilesViewPicker, query: string): FilesViewPickerSection[] {
  const q = query.trim().toLowerCase();
  if (q === "") return picker.sections;
  return picker.sections
    .map((s) => ({
      ...s,
      rows: s.rows.filter((r) => `${r.key ?? ""} ${r.title}`.toLowerCase().includes(q)),
    }))
    .filter((s) => s.rows.length > 0);
}

/** A request to focus the view's search box (⌥⌘P), counted so the same key twice focuses twice. */
export const viewSearchFocus: Store<number> = createStore(0);

/**
 * ⌥⌘P in the Files view (FIL-11): with a ticket's sheet open the tab's handler takes it (`false`); else the view's
 * search box, or the picked ticket's tab's, is asked for.
 */
export function goToFile(project: string, ticketOpen: boolean): boolean {
  if (ticketOpen) return false;
  if ((viewStates.get().get(project) ?? FIRST).picked !== null) searchFocus.update((n) => n + 1);
  else viewSearchFocus.update((n) => n + 1);
  return true;
}
