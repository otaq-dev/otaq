// Add project later (*14 → Adding a second project later*): from the rail's +, ⌘P → Add project, the empty
// board's "Add project folder…" or a folder dropped on the window. Tools were checked already, so it is one dialog:
// the folder, the key, the base branch; Esc or Cancel closes it. Adding shows the new project's board behind it, and
// the dialog moves to the same go-ahead as onboarding's Flow step, shorter (ONB-08, d0108; *14 → Row · Another
// project, and the presets* `s14-18`/`s14-19`): Look and propose a flow, Use a preset instead (the presets in the
// dialog) or Skip, use the default (Esc too), which only closes it. Either proposal opens in the usual preview over the
// board (`flow_dialog.ts`). One `Dialog` throughout, so the box doesn't leave and come back between its moments.
import { useEffect, useRef, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { Button, Dialog } from "../../components";
import type { FlowLookParams } from "../../generated/FlowLookParams";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import { asRequestError } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { AddButton, AddProjectFields, useAddProject } from "./AddProjectForm";
import {
  type AddDialogState,
  dialogOpens,
  lookFromDialog,
  presetFromDialog,
  projectAdded,
} from "./flow_dialog";
import { Preset } from "./FlowStep";

/** The go-ahead in the dialog (`s14-18`): the intro, what the agent looks at, the provider and tokens line. */
export function FlowDialogGoAhead({ step }: { step: OnboardingFlowStep }) {
  return (
    <>
      <p className="onboarding-text">{step.intro}</p>
      <div className="flow-step-line">
        <Icon name="sparkles" size={15} />
        <p>{step.line}</p>
      </div>
      <p className="flow-step-provider">
        <Icon name="info" size={12} />
        <span>{step.provider}</span>
      </p>
    </>
  );
}

/** The presets in the dialog (Use a preset instead, or Nothing to look at yet with its reason). */
export function FlowDialogPresets({
  step,
  busy,
  onPick,
}: {
  step: OnboardingFlowStep;
  busy: boolean;
  onPick: (file: string) => void;
}) {
  return (
    <>
      <p className="onboarding-text">{step.presets.text}</p>
      {step.presets.reason !== null && <p className="flow-step-reason">{step.presets.reason}</p>}
      <h3 className="flow-step-heading">{step.presets.heading}</h3>
      <div className="flow-step-presets">
        {step.presets.cards.map((card) => (
          <Preset
            key={card.file}
            card={card}
            disabled={busy}
            onPick={() => {
              onPick(card.file);
            }}
          />
        ))}
      </div>
    </>
  );
}

/** The dialog's buttons once the project is added: Skip, use the default · Use a preset instead · Look and propose. */
export function FlowDialogButtons({
  step,
  busy,
  onSkip,
  onPresets,
  onLook,
}: {
  step: OnboardingFlowStep;
  busy: boolean;
  onSkip: () => void;
  onPresets: () => void;
  onLook: () => void;
}) {
  return (
    <>
      <Button variant="ghost" label={step.skip} disabled={busy} onClick={onSkip} />
      <span className="onboarding-spacer" />
      <Button variant="secondary" label={step.use_preset} disabled={busy} onClick={onPresets} />
      <Button
        label={step.propose}
        icon="sparkles"
        disabled={busy || !step.can_propose}
        loading={busy}
        onClick={onLook}
      />
    </>
  );
}

function AddProjectBody({ folder, close }: { folder: string | null; close: () => void }) {
  const [dialog, setDialog] = useState<AddDialogState>({ at: "form" });
  const [step, setStep] = useState<OnboardingFlowStep>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const body = useRef<HTMLDivElement>(null);
  const state = useAddProject(false, folder, (key) => {
    setDialog(projectAdded(key));
  });
  const { view } = state;

  // What there is to look at runs `gh` and `glab`: asked once, when the project is added.
  const project = dialog.at === "form" ? null : dialog.project;
  useEffect(() => {
    if (project === null) return;
    const params: FlowLookParams = { project };
    request("onboarding.flow_dialog", params).then(
      (answer) => {
        const loaded = answer as OnboardingFlowStep;
        setStep(loaded);
        setDialog((d) => dialogOpens(d, loaded));
      },
      (e: unknown) => {
        setError(asRequestError(e).message);
      },
    );
  }, [project]);

  // The go-ahead's first button takes the keyboard; on the presets, the first preset.
  const at = step ? dialog.at : null;
  useEffect(() => {
    const box = body.current?.closest(".dialog");
    if (at === "go_ahead")
      box?.querySelector<HTMLElement>(".button[data-variant='primary']:not(:disabled)")?.focus();
    if (at === "presets") box?.querySelector<HTMLElement>(".flow-step-preset")?.focus();
  }, [at]);

  const act = (sent: Promise<unknown>) => {
    setBusy(true);
    setError(null);
    sent.then(close, (e: unknown) => {
      setBusy(false);
      setError(asRequestError(e).message);
    });
  };
  const failure = error !== null && <p className="onboarding-failure">{error}</p>;

  if (dialog.at === "form" || !step)
    return (
      <Dialog
        title={view?.title ?? ""}
        width={480}
        onConfirm={dialog.at === "form" ? state.add : undefined}
        onCancel={close}
        footer={
          <>
            <span className="onboarding-spacer" />
            {view && <Button label={view.cancel} variant="ghost" onClick={close} />}
            {dialog.at === "form" && <AddButton state={state} />}
          </>
        }
      >
        {/* Added: the form stays while the go-ahead loads, unless that failed. */}
        {failure || <AddProjectFields state={state} />}
      </Dialog>
    );

  const look = () => {
    if (step.can_propose && !busy) act(lookFromDialog(step.project, request));
  };
  return (
    <Dialog
      title={step.title}
      hint="close"
      width={640}
      onConfirm={dialog.at === "go_ahead" ? look : undefined}
      onCancel={close}
      footer={
        <FlowDialogButtons
          step={step}
          busy={busy}
          onSkip={close}
          onPresets={() => {
            if (dialog.at === "presets")
              body.current?.querySelector<HTMLElement>(".flow-step-preset")?.focus();
            else setDialog({ at: "presets", project: step.project });
          }}
          onLook={look}
        />
      }
    >
      <div className="add-project-flow" ref={body}>
        {dialog.at === "go_ahead" ? (
          <FlowDialogGoAhead step={step} />
        ) : (
          <FlowDialogPresets
            step={step}
            busy={busy}
            onPick={(file) => {
              act(presetFromDialog(step.project, file, request));
            }}
          />
        )}
        {failure}
      </div>
    </Dialog>
  );
}

export function AddProjectDialog({ params, close }: OverlayProps<"add_project">) {
  const folder = params?.folder ?? null;
  // Another folder (dropped while it is open) starts the form again.
  return <AddProjectBody key={folder ?? ""} folder={folder} close={close} />;
}
