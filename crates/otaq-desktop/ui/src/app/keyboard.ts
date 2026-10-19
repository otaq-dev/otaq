// The dispatcher on the page (T-0215): one capturing `keydown` listener on the window, so it sees each key before the
// focused element (a terminal's xterm, a text field) does. The decisions are `keys.ts`'s. The map is the user's keys,
// asked again on every `settings_changed` (d0097: a key changed in Settings applies at once). After `g` it waits about a
// second for the second key (`sequenceWait`, which the hint shows).
import type { Keymap } from "../generated/Keymap";
import { type PageEvents, pageEvents, watchView } from "../service";
import { request } from "../tauri";
import {
  dispatchKey,
  inEditorText,
  inKeyRecorder,
  isModifierKey,
  keysKeptBy,
  type LetterHold,
  lettersKeptBy,
  logLine,
  stopsEvent,
  widgetOpen,
} from "./keys";
import { modalOpen, overlays } from "./overlays";
import type { Registries } from "./registry";
import { focusTerminal, selection } from "./selection";
import { createStore } from "./store";

/** The user's keys from Rust (§21.3); `undefined` until they arrived. */
export const keymap = createStore<Keymap | undefined>(undefined);

/** Follows the map: once the event stream is open, then on each settings change. Returns the stop. */
export function watchKeymap(onChange: (map: Keymap) => void, events: PageEvents = pageEvents): () => void {
  return watchView(
    () => request("keymap.map") as Promise<Keymap>,
    ["settings_changed"],
    (state) => {
      if (state.view) onChange(state.view);
      else if (state.error) console.error("the keyboard map couldn't load", state.error);
    },
    events,
  );
}

/** Whether Otaq waits for a `g` sequence's second key (KEY-02): the hint shows meanwhile. */
export const sequenceWait = createStore(false);

/** How long Otaq waits after `g` for the second key, in ms ("about a second", d0097). */
export const SEQUENCE_WAIT_MS = 1000;

/** Starts or ends the wait after `g`; it ends by itself after `SEQUENCE_WAIT_MS`. */
export function createSequenceWait(
  store = sequenceWait,
  timers: Pick<Window, "setTimeout" | "clearTimeout"> = window,
): (on: boolean) => void {
  let timer: number | undefined;
  return (on) => {
    if (timer !== undefined) timers.clearTimeout(timer);
    timer = undefined;
    if (on)
      timer = timers.setTimeout(() => {
        timer = undefined;
        store.set(false);
      }, SEQUENCE_WAIT_MS);
    store.set(on);
  };
}

/** The newest key log lines, newest last (only filled with `OTAQ_KEY_LOG=1`). */
export const keyLog = createStore<readonly string[]>([]);

const LOG_LINES = 12;

/** The letters the page keeps from the app now: all of them while a menu or dialog is open, else the focus's. */
function lettersHere(focused: HTMLElement | null): LetterHold {
  return widgetOpen(document) ? "all" : lettersKeptBy(focused);
}

/** Loads the map and starts dispatching every key of `target`. Returns the stop. */
export function installKeys(app: Registries, target: Window = window): () => void {
  const stopWatching = watchKeymap(keymap.set);
  const wait = createSequenceWait();
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
        waiting: sequenceWait.get,
        wait,
      },
      event,
      {
        inTerminal: selection.get().focusedTerminal !== null,
        focusKeeps: keysKeptBy(focused),
        modal: modalOpen(overlays.store.get()),
        editorText: inEditorText(focused),
        letters: lettersHere(focused),
        recording: inKeyRecorder(focused),
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
    stopWatching();
    wait(false);
    target.removeEventListener("keydown", onKey, true);
  };
}
