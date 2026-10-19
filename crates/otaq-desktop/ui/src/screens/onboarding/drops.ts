// A folder dropped on the window (*02 → No projects yet*: "or drop a folder anywhere in this window"; the door's
// `files_dropped` event, when it didn't land on a terminal, see `terminal/drops.ts`): onboarding's project step takes it
// when onboarding is open, else Add project opens with it.
import { openOverlay, overlays } from "../../app/overlays";
import { createStore, type Store } from "../../app/store";

export interface DroppedFolder {
  path: string;
  /** Counts up with each drop, so the same folder dropped twice is two drops. */
  seq: number;
}

/** The newest folder dropped while onboarding is open. */
export const droppedFolder: Store<DroppedFolder | null> = createStore<DroppedFolder | null>(null);

export function folderDropped(path: string, store: Store<DroppedFolder | null> = droppedFolder): void {
  if (overlays.store.get().some((o) => o.id === "onboarding")) {
    store.update((last) => ({ path, seq: (last?.seq ?? 0) + 1 }));
    return;
  }
  openOverlay("add_project", { folder: path });
}
