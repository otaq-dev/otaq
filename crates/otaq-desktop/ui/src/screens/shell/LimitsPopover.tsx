// The agent limits popover under Activity's "3 / 5 agents" (SCH-03, d0016; *02 → Row · Header states (0086 · V) →
// Agent limits on*): the same two values as Settings. Each press saves at once and reaches the scheduler without restarting
// anything; running agents are never stopped. The steps and their bounds are Rust's (`limits.rs`).
import { useEffect, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { useStore } from "../../app/store";
import { Popover } from "../../components";
import type { ActivityPopover } from "../../generated/ActivityPopover";
import type { HeaderView } from "../../generated/HeaderView";
import type { LimitField } from "../../generated/LimitField";
import type { LimitStep } from "../../generated/LimitStep";
import type { LimitStepper } from "../../generated/LimitStepper";
import type { LimitsPopover as PopoverView } from "../../generated/LimitsPopover";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { agentsAnchor, headerStore } from "./Header";

function Stepper({ stepper, onStep }: { stepper: LimitStepper; onStep: (up: boolean) => void }) {
  return (
    <div className="limits-row">
      <span className="limits-label">{stepper.label}</span>
      <div className="stepper" role="group" aria-label={stepper.label}>
        <button
          type="button"
          aria-label="Fewer"
          disabled={!stepper.can_lower}
          onClick={() => {
            onStep(false);
          }}
        >
          <Icon name="minus" size={12} />
        </button>
        <span className="stepper-value">{stepper.value}</span>
        <button
          type="button"
          aria-label="More"
          disabled={!stepper.can_raise}
          onClick={() => {
            onStep(true);
          }}
        >
          <Icon name="plus" size={12} />
        </button>
      </div>
    </div>
  );
}

export function LimitsBody({
  view,
  onStep,
}: {
  view: PopoverView;
  onStep: (field: LimitField, up: boolean) => void;
}) {
  return (
    <div className="limits">
      <strong className="limits-title">{view.title}</strong>
      <Stepper
        stepper={view.max_agents}
        onStep={(up) => {
          onStep("max_agents", up);
        }}
      />
      <Stepper
        stepper={view.back_pressure}
        onStep={(up) => {
          onStep("back_pressure", up);
        }}
      />
      <p className="limits-note">{view.note}</p>
    </div>
  );
}

/** The `limits` overlay. A press's answer shows at once, until the header's next view (after `settings_changed`). */
export function LimitsOverlay({ close }: OverlayProps<"limits">) {
  const header = useStore(headerStore);
  const [answer, setAnswer] = useState<{ after: HeaderView | undefined; view: PopoverView }>();
  // The stepper heads Activity's popover; asked again with each new header view.
  const [stored, setStored] = useState<PopoverView | null>(null);
  useEffect(() => {
    request("shell.activity").then(
      (popover) => {
        setStored((popover as ActivityPopover).limits);
      },
      () => undefined,
    );
  }, [header]);
  const view = answer && answer.after === header ? answer.view : stored;
  // Limits turned off (in Settings) while open: the chip is no control any more.
  if (!view) return null;
  const step = (field: LimitField, up: boolean) => {
    const params: LimitStep = { field, up };
    request("shell.step_limit", params).then(
      (next) => {
        setAnswer({ after: headerStore.get(), view: next as PopoverView });
      },
      // A refused save keeps the values shown; the header's next view says what is stored.
      () => undefined,
    );
  };
  return (
    <Popover label={view.title} anchor={agentsAnchor} align="end" onClose={close}>
      <LimitsBody view={view} onStep={step} />
    </Popover>
  );
}
