// The flow editor (T-0471, FED-01): the `flow_editor` overlay over the window. `openFlowEditor` (`logic.ts`) opens it;
// what it says is Rust's (`src/api/flow_editor.rs`, `otaq-view`'s `flow_editor.rs`).
import type { Screen } from "../../app/registry";
import { FlowEditorOverlay } from "./FlowEditor";
import "./flow_editor.css";

export const flowEditor: Screen = (app) => {
  app.overlays.register("flow_editor", { Component: FlowEditorOverlay });
};
