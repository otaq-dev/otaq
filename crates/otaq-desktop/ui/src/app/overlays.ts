// Which overlays are open, newest last. Any screen opens any overlay by id; the frame's overlay host draws the open
// ones that are registered.
import { OVERLAY_MODAL, type OverlayId, type OverlayParams } from "./registry/overlays";
import { createStore, type Store, useStore } from "./store";

export type OpenOverlay = { [K in OverlayId]: { id: K; params: OverlayParams[K] } }[OverlayId];

/** The params argument: optional where the overlay can be opened with nothing. */
type ParamsArg<K extends OverlayId> = undefined extends OverlayParams[K]
  ? [params?: OverlayParams[K]]
  : [params: OverlayParams[K]];

export interface OverlayStack {
  store: Store<readonly OpenOverlay[]>;
  /** Opens `id` on top; one already open moves to the top with the new params. */
  open: <K extends OverlayId>(id: K, ...params: ParamsArg<K>) => void;
  close: (id: OverlayId) => void;
  /** Closes the newest overlay, if any. */
  closeTop: () => void;
  /** Closes `id` if it is open, else opens it (⇧⌘I for the inbox). */
  toggle: <K extends OverlayId>(id: K, ...params: ParamsArg<K>) => void;
}

export function createOverlayStack(): OverlayStack {
  const store = createStore<readonly OpenOverlay[]>([]);
  const close = (id: OverlayId) => {
    store.update((open) => (open.some((o) => o.id === id) ? open.filter((o) => o.id !== id) : open));
  };
  const open = <K extends OverlayId>(id: K, ...[params]: ParamsArg<K>) => {
    const entry = { id, params } as OpenOverlay;
    store.update((list) => [...list.filter((o) => o.id !== id), entry]);
  };
  return {
    store,
    open,
    close,
    closeTop: () => {
      store.update((list) => (list.length === 0 ? list : list.slice(0, -1)));
    },
    toggle: (id, ...params) => {
      if (store.get().some((o) => o.id === id)) close(id);
      else open(id, ...params);
    },
  };
}

/** Whether a modal overlay is open: the app's shortcuts then don't run (`modal.rs`). */
export function modalOpen(open: readonly OpenOverlay[]): boolean {
  return open.some((o) => OVERLAY_MODAL[o.id]);
}

/** The page's overlays. */
export const overlays = createOverlayStack();

export const openOverlay = overlays.open;
export const closeOverlay = overlays.close;
export const toggleOverlay = overlays.toggle;

export function useOpenOverlays(): readonly OpenOverlay[] {
  return useStore(overlays.store);
}
