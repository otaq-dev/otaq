// Pause in the title bar (T-0457; PAU-08; d0112): the button after Activity (the `header_pause` slot) and ⇧⌘., which
// opens its scope menu from anywhere, a focused terminal included (0097's Stop all row). The paused banner, the
// restart toast, the cards' "Paused" line, Activity's "5 paused" and the rail's tiles are drawn by their own screens
// from Rust's views. No confirmation anywhere: a pick pauses or resumes at once.
import type { Screen } from "../../app/registry";
import { togglePauseMenu } from "./logic";
import { HeaderPause } from "./Pause";
import "./pause.css";

export const pause: Screen = (app) => {
  app.slots.register("header_pause", { Component: HeaderPause });
  app.shortcuts.on("pause", togglePauseMenu);
};
