// Area 02 · The command palette (T-0393, KEY-06, d0097): the `command_palette` overlay on ⌘K / ⇧⌘P (from anywhere,
// terminals included: the dispatcher routes them here), and the recent items it lists when empty. Every action a
// handler took (from a key or the palette), every ticket opened and every switch to another project, from anywhere,
// goes to the service (`palette.record`), which keeps them across restarts. What it says is Rust's
// (`src/api/palette.rs`, `otaq-view`'s `palette.rs`).
import { toggleOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { type Selection, selection } from "../../app/selection";
import type { PaletteUse } from "../../generated/PaletteUse";
import { request } from "../../tauri";
import { usesOf } from "./logic";
import { Palette } from "./Palette";
import "./palette.css";

/** Sends one use to the service; a failure loses only that one. */
export function recordUse(used: PaletteUse, ask: typeof request = request): void {
  ask("palette.record", used).catch(() => undefined);
}

/** Records each ticket opened and project switched to in `store`. Returns the stop. */
export function recordSelection(
  store = selection,
  record: (used: PaletteUse) => void = recordUse,
): () => void {
  let before: Selection = store.get();
  return store.subscribe(() => {
    const after = store.get();
    const used = usesOf(before, after);
    before = after;
    if (used) record(used);
  });
}

export const palette: Screen = (app) => {
  app.overlays.register("command_palette", { Component: Palette });
  app.shortcuts.on("command_palette", () => {
    toggleOverlay("command_palette", { terminal: selection.get().focusedTerminal !== null });
  });
  // Rust keeps only the actions that go there (`palette::recorded`).
  app.shortcuts.onRan((action) => {
    recordUse({ kind: "action", action });
  });
  recordSelection();
};
