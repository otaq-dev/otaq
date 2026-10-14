// A small shared store for the page's interaction state (UIT-04): what is selected, which overlays are open. Board
// data never goes here; it comes from the service as views (`useServiceView`).
import { useSyncExternalStore } from "react";

export interface Store<T> {
  get: () => T;
  set: (value: T) => void;
  update: (change: (current: T) => T) => void;
  /** Calls `listener` after each change. Returns the stop. */
  subscribe: (listener: () => void) => () => void;
}

export function createStore<T>(initial: T): Store<T> {
  let value = initial;
  const listeners = new Set<() => void>();
  const set = (next: T) => {
    if (Object.is(next, value)) return;
    value = next;
    for (const l of listeners) l();
  };
  return {
    get: () => value,
    set,
    update: (change) => {
      set(change(value));
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** The store's value, re-rendering on each change. */
export function useStore<T>(store: Store<T>): T {
  return useSyncExternalStore(store.subscribe, store.get, store.get);
}
