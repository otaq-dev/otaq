// The "Epics in flight" strip folded to its one line by a click on its header (EPC-10), per project while the app
// runs. The strip and this fold go with epic lanes (EPL-01, T-0462).
import { createStore, type Store } from "../../../app/store";

export const stripFolds: Store<Readonly<Record<string, boolean>>> = createStore({});

export function stripFolded(all: Readonly<Record<string, boolean>>, project: string): boolean {
  return all[project] ?? false;
}

/** A click on the strip's header: folds it to one line, or unfolds it, for this project only. */
export function toggleStripFold(project: string, store = stripFolds): void {
  store.update((all) => ({ ...all, [project]: !stripFolded(all, project) }));
}
