// Area 10 · Needs-you inbox (T-0230): the inbox overlay (⇧⌘I and the header button), ⌘J through the queue with the
// run bar on top of the panel and "All clear", and Resume all's confirmation. What it says is Rust's
// (`src/api/inbox.rs`, `otaq-view`'s `inbox_page.rs`).
import { toggleOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { AllClear } from "./AllClear";
import { Inbox, runNext } from "./Inbox";
import "./inbox.css";
import { ResumeAll } from "./ResumeAll";
import { RunBar } from "./RunBar";

export const inbox: Screen = (app) => {
  app.overlays.register("inbox", { Component: Inbox });
  app.overlays.register("resume_all", { Component: ResumeAll });
  app.overlays.register("all_clear", { Component: AllClear });
  app.panelSections.register("inbox_run", { Component: RunBar });
  app.shortcuts.on("open_inbox", () => {
    toggleOverlay("inbox");
  });
  // From anywhere, a focused terminal included (keys.ts passes the app's keys).
  app.shortcuts.on("next_inbox_item", () => {
    void runNext();
  });
};
