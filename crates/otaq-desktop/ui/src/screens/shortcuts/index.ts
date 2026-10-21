// The ? shortcut list (T-0394, KEY-07): its overlay and `show_shortcuts` (`?` while nobody is typing, and the
// palette's "Show all shortcuts", T-0393). It is modal: no app key runs behind it, Esc closes it.
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { ShortcutsOverlay } from "./ShortcutList";
import "./shortcuts.css";

export const shortcuts: Screen = (app) => {
  app.overlays.register("shortcuts", { Component: ShortcutsOverlay });
  app.shortcuts.on("show_shortcuts", () => {
    openOverlay("shortcuts");
  });
};
