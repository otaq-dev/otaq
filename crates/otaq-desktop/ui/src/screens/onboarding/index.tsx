// Area 14 · First run & onboarding (T-0233): the onboarding overlay, opened at start when no project is stored yet
// (and on its tools step by Check tools), the Add project dialog, and folders dropped on the window. What they say is
// Rust's (`src/api/onboarding.rs`, `onboarding_page.rs`).
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import type { OnboardingStart } from "../../generated/OnboardingStart";
import type { ServiceConnection } from "../../generated/ServiceConnection";
import { pageEvents, watchConnection } from "../../service";
import { request } from "../../tauri";
import { filesDropped } from "../../terminal/drops";
import { AddProjectDialog } from "./AddProjectDialog";
import { folderDropped } from "./drops";
import { OnboardingOverlay } from "./Onboarding";
import { askUsageDataWhenNeeded, UsageDataOverlay } from "./UsageData";
import "./onboarding.css";

/** Opens "Help improve Otaq" when the user hasn't answered yet (d0085). */
export function askUsageData(ask: typeof request = request): void {
  askUsageDataWhenNeeded(ask, (dialog) => {
    openOverlay("usage_data", dialog);
  }).catch(() => undefined);
}

/**
 * Once the service is connected the first time: onboarding opens when no project is stored yet (UI-08), or on the Flow
 * step of first run's project while its flow isn't settled (ASK-04, `onboarding.start` reads its Flow setup); otherwise
 * "Help improve Otaq" when it was never answered (d0085; onboarding asks it itself when it ends).
 */
export function openOnboardingWhenNeeded(
  ask: typeof request = request,
  watch: (onChange: (c: ServiceConnection) => void) => () => void = watchConnection,
): () => void {
  let asked = false;
  return watch((connection) => {
    if (asked || connection.state !== "connected") return;
    asked = true;
    ask("onboarding.start").then(
      (answer) => {
        const start = answer as OnboardingStart;
        if (!start.show) askUsageData(ask);
        else if (start.step !== undefined && start.project !== undefined)
          openOverlay("onboarding", { step: start.step, project: start.project });
        else openOverlay("onboarding");
      },
      () => undefined,
    );
  });
}

export const onboarding: Screen = (app) => {
  app.overlays.register("onboarding", { Component: OnboardingOverlay });
  app.overlays.register("add_project", { Component: AddProjectDialog });
  app.overlays.register("usage_data", { Component: UsageDataOverlay });
  openOnboardingWhenNeeded();
  pageEvents.listen((event) => {
    // On a terminal the paths are pasted into it (d0099); elsewhere the first one is a folder for Add project.
    if (event.kind === "files_dropped") filesDropped(event, folderDropped);
  });
};
