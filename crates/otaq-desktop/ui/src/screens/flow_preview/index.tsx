// The flow preview (T-0420, FLW-04): the `flow_preview` overlay over the main area beside the Project agent pane, and
// the follower in the main area's layer that opens it whenever a proposal arrives, for any project. Settings → Flow
// opens it with `proposeFlow` / `openFlowPreview` (`logic.ts`). What it says is Rust's (`src/api/flow_preview.rs`,
// `otaq-view`'s `flow_preview.rs`).
import { useEffect } from "react";

import type { Screen } from "../../app/registry";
import { FlowPreviewSheet } from "./FlowPreview";
import "./flow_preview.css";
import { followProposals } from "./logic";

/** Opens the preview when a proposal arrives; draws nothing. */
function ProposalFollower() {
  useEffect(() => followProposals(), []);
  return null;
}

export const flowPreview: Screen = (app) => {
  app.overlays.register("flow_preview", { Component: FlowPreviewSheet });
  app.slots.register("main_area", { Component: ProposalFollower });
};
