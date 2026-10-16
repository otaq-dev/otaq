// The panel's own dialogs (CLS-01, CLS-05; d0033, d0052): *Close ticket · confirm*, which names the running agent
// and the tickets that wait for this one, and *Submit for review · dialog* with its optional note. Their texts come
// from `panel.close_dialog` / `panel.submit_dialog`; a refusal ("2 files are not committed…") shows inside. Opened from
// a lead's suggestion (LEAD-08), Close adds "Suggested by Wishlist lead: “…”" and its confirm goes through the
// suggestion (`suggestion.confirm`).
import { useCallback, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { ConfirmDialog, TextArea } from "../../components";
import type { PanelCloseDialog } from "../../generated/PanelCloseDialog";
import type { PanelSubmitDialog } from "../../generated/PanelSubmitDialog";
import { asRequestError, useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { confirmSuggestion } from "../inbox/logic";
import { SuggestedBy } from "../inbox/SuggestedBy";

/** Runs a confirm once at a time; its refusal stays on screen, success closes the dialog. */
function useConfirm(close: () => void) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = (send: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    send().then(close, (e: unknown) => {
      setError(asRequestError(e).message);
      setBusy(false);
    });
  };
  return { busy, error, run };
}

function Refusal({ error }: { error: string | null }) {
  return error === null ? null : <p className="panel-dialog-error">{error}</p>;
}

export function CloseTicketDialog({ params, close }: OverlayProps<"close_ticket">) {
  const load = useCallback(
    () => request("panel.close_dialog", { key: params.ticket }) as Promise<PanelCloseDialog>,
    [params.ticket],
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
      body={
        <div className="panel-dialog-body">
          {params.suggestion && <SuggestedBy line={params.suggestion.line} />}
          {view.lines.map((line) => (
            <p key={line.text} className="panel-dialog-line" data-agent={line.agent || undefined}>
              <Icon name={line.icon} size={13} />
              <span>{line.text}</span>
            </p>
          ))}
          <p>{view.footer}</p>
          <Refusal error={confirm.error} />
        </div>
      }
      onConfirm={() => {
        const { suggestion } = params;
        confirm.run(() =>
          suggestion
            ? confirmSuggestion(params.ticket, suggestion, "close")
            : request("panel.close", { key: params.ticket }),
        );
      }}
      onCancel={close}
    />
  );
}

export function SubmitForReviewDialog({ params, close }: OverlayProps<"submit_for_review">) {
  const load = useCallback(
    () => request("panel.submit_dialog", { key: params.ticket }) as Promise<PanelSubmitDialog>,
    [params.ticket],
  );
  const { view } = useServiceView(load, []);
  const [note, setNote] = useState("");
  const confirm = useConfirm(close);
  if (!view) return null;
  return (
    <ConfirmDialog
      title={view.title}
      width={480}
      confirmLabel={view.confirm}
      confirmIcon="send"
      confirmDisabled={confirm.busy}
      body={
        <div className="panel-dialog-body">
          <p className="panel-dialog-intro">
            <Icon name="info" size={13} />
            <span>{view.intro}</span>
          </p>
          <label className="panel-dialog-label" htmlFor="panel-submit-note">
            {view.note_label}
          </label>
          <TextArea
            id="panel-submit-note"
            value={note}
            placeholder={view.note_placeholder}
            onChange={setNote}
          />
          <Refusal error={confirm.error} />
          <p>{view.footer}</p>
        </div>
      }
      onConfirm={() => {
        confirm.run(() => request("panel.submit", { key: params.ticket, note }));
      }}
      onCancel={close}
    />
  );
}
