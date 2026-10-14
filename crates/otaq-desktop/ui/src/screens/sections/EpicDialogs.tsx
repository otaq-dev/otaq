// The epic dialogs (EPC-06, EPC-07, TKT-07; d0033, d0044; *05 → Epic dialogs*, *08 → Move to epic · base changes
// (SHOP-51)*): Move to epic… picks an open epic, previews the new key and says the old key keeps working (and how the
// base changes); Close epic… lists the agents it stops, the tickets it also closes and the ones that stay merged.
// Their texts come from `sections.move_dialog` / `sections.close_epic_dialog`; a refusal shows inside.
import { useCallback, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { openTicket, selection } from "../../app/selection";
import { ConfirmDialog } from "../../components";
import type { CloseEpicDialogView } from "../../generated/CloseEpicDialogView";
import type { MoveDialogView } from "../../generated/MoveDialogView";
import { asRequestError, useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { canMove, followMove, pickedChoice } from "./logic";

/** Runs a confirm once at a time; its refusal stays on screen, success closes the dialog. */
function useConfirm(close: () => void) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = <T,>(send: () => Promise<T>, then?: (value: T) => void) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    send().then(
      (value) => {
        then?.(value);
        close();
      },
      (e: unknown) => {
        setError(asRequestError(e).message);
        setBusy(false);
      },
    );
  };
  return { busy, error, run };
}

function Refusal({ error }: { error: string | null }) {
  return error === null ? null : <p className="section-dialog-error">{error}</p>;
}

export function MoveDialogBody({
  view,
  picked,
  onPick,
  error,
}: {
  view: MoveDialogView;
  picked: string | null;
  onPick: (epic: string) => void;
  error: string | null;
}) {
  const choice = pickedChoice(view, picked);
  return (
    <div className="section-dialog-body">
      <div className="move-choices" role="radiogroup" aria-label={view.title}>
        {view.choices.map((c) => (
          <button
            key={c.key}
            type="button"
            role="radio"
            aria-checked={c.key === choice?.key}
            className="move-choice"
            onClick={() => {
              onPick(c.key);
            }}
          >
            <Icon name="layers" size={13} />
            <span className="move-choice-key">{c.key}</span>
            <span className="move-choice-title">{c.title}</span>
            <span className="move-choice-progress">{c.progress}</span>
          </button>
        ))}
      </div>
      {choice && (
        <div className="move-keys">
          <span className="move-old">{view.old_key}</span>
          <span className="move-arrow">→</span>
          <span className="move-new">{choice.new_key}</span>
          <span className="move-alias">{choice.alias_note}</span>
        </div>
      )}
      {choice?.base_note != null && (
        <p className="section-dialog-box" data-tone="paused">
          <Icon name="git-branch" size={13} />
          <span>{choice.base_note}</span>
        </p>
      )}
      <p className="section-dialog-note">{view.disk_note}</p>
      <Refusal error={view.refusal ?? error} />
    </div>
  );
}

export function MoveToEpicDialog({ params, close }: OverlayProps<"move_to_epic">) {
  const load = useCallback(
    () => request("sections.move_dialog", { key: params.ticket }) as Promise<MoveDialogView>,
    [params.ticket],
  );
  const { view } = useServiceView(load, []);
  const [picked, setPicked] = useState<string | null>(null);
  const confirm = useConfirm(close);
  if (!view) return null;
  const choice = pickedChoice(view, picked);
  return (
    <ConfirmDialog
      title={choice?.heading ?? view.title}
      width={480}
      confirmLabel={choice?.confirm ?? view.title}
      confirmIcon="layers"
      confirmDisabled={confirm.busy || !canMove(view, picked)}
      body={<MoveDialogBody view={view} picked={picked} onPick={setPicked} error={confirm.error} />}
      onConfirm={() => {
        if (!choice || !canMove(view, picked)) return;
        confirm.run(
          () => request("sections.move_to_epic", { key: params.ticket, epic: choice.key }) as Promise<string>,
          (newKey) => {
            const follow = followMove(selection.get().ticket, params.ticket, newKey);
            if (follow !== null) openTicket(follow);
          },
        );
      }}
      onCancel={close}
    />
  );
}

export function CloseEpicBody({ view, error }: { view: CloseEpicDialogView; error: string | null }) {
  return (
    <div className="section-dialog-body">
      {view.agents.map((line) => (
        <p key={line} className="section-dialog-box" data-tone="paused">
          <Icon name="triangle-alert" size={13} />
          <span>{line}</span>
        </p>
      ))}
      {view.also !== null && (
        <p className="section-dialog-box">
          <Icon name="layers" size={13} />
          <span>{view.also}</span>
        </p>
      )}
      <p className="section-dialog-note">{view.note}</p>
      <Refusal error={error} />
    </div>
  );
}

export function CloseEpicDialog({ params, close }: OverlayProps<"close_epic">) {
  const load = useCallback(
    () => request("sections.close_epic_dialog", { epic: params.epic }) as Promise<CloseEpicDialogView>,
    [params.epic],
  );
  const { view } = useServiceView(load, []);
  const confirm = useConfirm(close);
  if (!view) return null;
  return (
    <ConfirmDialog
      title={view.title}
      width={480}
      destructive
      confirmLabel={view.confirm}
      confirmIcon="circle-slash"
      confirmDisabled={confirm.busy}
      body={<CloseEpicBody view={view} error={confirm.error} />}
      onConfirm={() => {
        confirm.run(() => request("sections.close_epic", { epic: params.epic }));
      }}
      onCancel={close}
    />
  );
}
