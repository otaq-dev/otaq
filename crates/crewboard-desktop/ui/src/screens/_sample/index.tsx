// The template screen (T-0212): one folder, one exported `Screen` that registers its parts, and one line in the
// screens index. It is not in the index; `registry.test.ts` adds it there to show that nothing else changes.
import { toggleOverlay } from "../../app/overlays";
import type { OverlayProps, Screen } from "../../app/registry";

function SampleOverlay({ close }: OverlayProps<"cleanup">) {
  return (
    <button type="button" onClick={close}>
      Close cleanup
    </button>
  );
}

function SampleSection() {
  return <p>Validation</p>;
}

export const sample: Screen = (app) => {
  // Ids no screen owns yet: T-0235 builds Cleanup and T-0240 Validation; each moves the sample to another free id
  // then. Every panel tab has its screen since T-0232, so the sample registers none.
  app.overlays.register("cleanup", { Component: SampleOverlay });
  app.settings.global.register("validation", { Component: SampleSection });
  app.shortcuts.on("switch_tab", ({ number }) => {
    // Another tab's number: leave it to the handler registered before (the panel's).
    if (number !== 6) return false;
    toggleOverlay("cleanup");
    return true;
  });
};
