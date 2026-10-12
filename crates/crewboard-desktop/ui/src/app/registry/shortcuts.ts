// The shortcut handlers: `KeyAction` → handler, filled by screens. The key map and the dispatcher that turns a key
// into a `KeyAction` are T-0215's (the Rust keymap stays the only map); it calls `run`.
import type { KeyAction } from "../../generated/KeyAction";

export type KeyActionName = KeyAction["action"];

/** The action of that name, with its number for `project` and `switch_tab`. */
export type ActionNamed<N extends KeyActionName> = Extract<KeyAction, { action: N }>;

/** Returns `false` to leave the key to the handler registered before it; anything else handles it. */
export type ShortcutHandler<N extends KeyActionName> = (action: ActionNamed<N>) => unknown;

export interface Shortcuts {
  /** Adds a handler, tried before those added earlier. Returns its removal (for a component's effect). */
  on<N extends KeyActionName>(name: N, handler: ShortcutHandler<N>): () => void;
  /** Runs the newest handler for `action` that takes it. Whether one did. */
  run(action: KeyAction): boolean;
  /** The actions with at least one handler. */
  names(): KeyActionName[];
}

export function createShortcuts(): Shortcuts {
  const handlers = new Map<KeyActionName, ((action: KeyAction) => unknown)[]>();
  return {
    on(name, handler) {
      const entry = handler as (action: KeyAction) => unknown;
      handlers.set(name, [...(handlers.get(name) ?? []), entry]);
      return () => {
        const rest = (handlers.get(name) ?? []).filter((h) => h !== entry);
        if (rest.length > 0) handlers.set(name, rest);
        else handlers.delete(name);
      };
    },
    run(action) {
      const list = handlers.get(action.action) ?? [];
      return list.toReversed().some((handler) => handler(action) !== false);
    },
    names: () => [...handlers.keys()],
  };
}
