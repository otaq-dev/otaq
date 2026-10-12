// The template screen (T-0212): one folder, one exported `Screen` that registers its parts, and one line in the
// screens index. It is not in the index; `registry.test.ts` adds it there to show that nothing else changes.
import { toggleOverlay } from "../../app/overlays";
import type { OverlayProps, Screen, TicketProps } from "../../app/registry";

function SampleTab({ ticket }: TicketProps) {
  return <p>Shell of {ticket}</p>;
}

function SampleOverlay({ close }: OverlayProps<"processes">) {
  return (
    <button type="button" onClick={close}>
      Close processes
    </button>
  );
}

function SampleSection() {
  return <p>Validation</p>;
}

export const sample: Screen = (app) => {
  // A tab no screen owns yet (T-0223 took History; T-0232 builds Shell and moves the sample again).
  app.panelTabs.register("shell", { Component: SampleTab });
  app.overlays.register("processes", { Component: SampleOverlay });
  // An id no screen owns yet (T-0240 builds it; it moves the sample to another free id then).
  app.settings.global.register("validation", { Component: SampleSection });
  app.shortcuts.on("switch_tab", ({ number }) => {
    // Another tab's number: leave it to the handler registered before (the panel's).
    if (number !== 6) return false;
    toggleOverlay("processes");
    return true;
  });
};
