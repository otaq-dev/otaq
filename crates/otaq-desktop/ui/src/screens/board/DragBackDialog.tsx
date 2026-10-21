// The drag-back dialog (*03 → Dragging a card back to an earlier stage*, s03-50/s03-51; FLW-18, d0108): a card
// dropped on an earlier Agent stage asks for that stage's agent's instructions ("Send SHOP-19 back to Dev?" · Send to
// Dev), one dropped on an earlier agent check for a note ("Run Security again on SHOP-18?" · Run Security). When an
// agent works where the card is, the dialog says it stops and the button reads "Stop and send to Dev". Its words are
// `board_page::drag_back_dialog`'s, opened from `board.drop`'s answer; the confirm sends `board.drag_back`. Cancel and
// Esc leave the card where it was; a refusal stays in the dialog.
import { useState } from "react";

import { openOverlay } from "../../app/overlays";
import type { OverlayProps } from "../../app/registry";
import { Button, Dialog } from "../../components";
import type { BoardDragBackParams } from "../../generated/BoardDragBackParams";
import type { DragBackDialog as DragBackView } from "../../generated/DragBackDialog";
import type { PanelSent } from "../../generated/PanelSent";
import { asRequestError } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

/** Sends the drag back with `note`; unsent text in the agent's terminal asks first, then sends again with `clear`
 * (d0048). */
export function sendDragBack(
  view: DragBackView,
  note: string,
  clear = false,
  ask: typeof request = request,
): Promise<void> {
  const params: BoardDragBackParams = { key: view.key, target: view.target, note, clear };
  return (ask("board.drag_back", params) as Promise<PanelSent>).then((sent) => {
    if (sent.outcome === "done") return;
    openOverlay("confirm", {
      title: sent.title,
      body: sent.body,
      confirmLabel: sent.clear,
      cancelLabel: sent.cancel,
      destructive: false,
      onConfirm: () => sendDragBack(view, note, true, ask),
    });
  });
}

export interface DragBackBodyProps {
  view: DragBackView;
  note: string;
  onNote: (note: string) => void;
  refusal: string | null;
}

/** What the agent working where the card is does, the text box and where the card goes after. */
export function DragBackBody({ view, note, onNote, refusal }: DragBackBodyProps) {
  return (
    <div className="review-body">
      {view.stops !== null && (
        <div className="review-callout" data-tone="paused">
          <Icon name="triangle-alert" size={14} />
          <div>{view.stops}</div>
        </div>
      )}
      <label className="review-label" htmlFor="drag-back-note">
        {view.label}
      </label>
      <textarea
        id="drag-back-note"
        className="text-input text-area"
        rows={4}
        value={note}
        placeholder={view.placeholder ?? undefined}
        onChange={(e) => {
          onNote(e.target.value);
        }}
      />
      {view.then !== null && <p className="review-muted">{view.then}</p>}
      {refusal !== null && (
        <p className="review-error" role="alert">
          {refusal}
        </p>
      )}
    </div>
  );
}

export function DragBackDialog({ params: view, close }: OverlayProps<"drag_back">) {
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const confirm = () => {
    if (busy) return;
    setBusy(true);
    setRefusal(null);
    sendDragBack(view, note).then(close, (e: unknown) => {
      setRefusal(asRequestError(e).message);
      setBusy(false);
    });
  };
  return (
    <Dialog
      title={view.title}
      width={480}
      onConfirm={confirm}
      onCancel={close}
      footer={
        <>
          <Button variant="ghost" label={view.cancel} onClick={close} />
          <Button icon={view.confirm_icon} label={view.confirm} loading={busy} onClick={confirm} />
        </>
      }
    >
      <DragBackBody view={view} note={note} onNote={setNote} refusal={refusal} />
    </Dialog>
  );
}
