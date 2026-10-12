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

export const sample: Screen = (app) => {
  // An id no screen owns yet: T-0235 builds Cleanup and moves the sample to another free id then. Every panel tab
  // has its screen since T-0232 and every settings section since T-0240, so the sample registers neither.
  app.overlays.register("cleanup", { Component: SampleOverlay });
  app.shortcuts.on("switch_tab", ({ number }) => {
    // Another tab's number: leave it to the handler registered before (the panel's).
    if (number !== 6) return false;
    toggleOverlay("cleanup");
    return true;
  });
};
