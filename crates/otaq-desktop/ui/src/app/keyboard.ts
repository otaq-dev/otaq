// The dispatcher on the page (T-0215): one capturing `keydown` listener on the window, so it sees each key before the
// focused element (a terminal's xterm, a text field) does. The decisions are `keys.ts`'s.
import type { Keymap } from "../generated/Keymap";
import { request } from "../tauri";
import { dispatchKey, inEditorText, isModifierKey, keysKeptBy, logLine, stopsEvent } from "./keys";
import { modalOpen, overlays } from "./overlays";
import type { Registries } from "./registry";
import { focusTerminal, selection } from "./selection";
import { createStore } from "./store";

/** The §21.3 map from Rust; `undefined` until it arrived. */
export const keymap = createStore<Keymap | undefined>(undefined);

/** The newest key log lines, newest last (only filled with `OTAQ_KEY_LOG=1`). */
export const keyLog = createStore<readonly string[]>([]);

const LOG_LINES = 12;

/** Loads the map and starts dispatching every key of `target`. Returns the stop. */
export function installKeys(app: Registries, target: Window = window): () => void {
  let stopped = false;
  request("keymap.map").then(
    (map) => {
      if (!stopped) keymap.set(map as Keymap);
    },
    (error: unknown) => {
      console.error("the keyboard map couldn't load", error);
    },
  );
  const onKey = (event: KeyboardEvent) => {
    if (isModifierKey(event) || event.isComposing) return;
    const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const outcome = dispatchKey(
      {
        keymap: keymap.get,
        run: (action) => app.shortcuts.run(action),
        leaveTerminal: () => {
          focusTerminal(null);
          focused?.blur();
        },
        mac: navigator.userAgent.includes("Mac"),
      },
      event,
      {
        inTerminal: selection.get().focusedTerminal !== null,
        focusKeeps: keysKeptBy(focused),
        modal: modalOpen(overlays.store.get()),
        editorText: inEditorText(focused),
      },
    );
    if (stopsEvent(outcome)) {
      event.preventDefault();
      event.stopPropagation();
    }
    if (keymap.get()?.log === true) {
      const line = logLine(event, outcome);
      console.info(`key: ${line}`);
      keyLog.update((lines) => [...lines, line].slice(-LOG_LINES));
    }
  };
  target.addEventListener("keydown", onKey, true);
  return () => {
    stopped = true;
    target.removeEventListener("keydown", onKey, true);
  };
}
