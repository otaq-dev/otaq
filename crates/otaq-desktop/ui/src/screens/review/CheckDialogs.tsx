// The dialogs of a human check and an open PR (FLW-14, FLW-16; d0108; `s03-50`): **Fail** ("Fail Review on SHOP-18?",
// what's wrong, Fail and send to Dev) and **Send back with PR comments** (an optional note, Send back), and what Pass
// opens. Their words come from `review.fail_view` and `review.send_back_pr_view` (`check_dialogs.rs`); Fail sends
// `review.send_back` (`ticket.fail`), Send back `review.send_back_pr`, Pass `review.pass`.
import { useCallback, useEffect, useId, useState } from "react";

import { openOverlay } from "../../app/overlays";
import type { OverlayParams } from "../../app/registry/overlays";
import type { OverlayProps } from "../../app/registry";
import { Button, Dialog, TextArea } from "../../components";
import type { PanelSent } from "../../generated/PanelSent";
import type { ReviewPassed } from "../../generated/ReviewPassed";
import type { SendBackPrParams } from "../../generated/SendBackPrParams";
import type { TicketFailView } from "../../generated/TicketFailView";
import type { TicketSendBackPrView } from "../../generated/TicketSendBackPrView";
import { asRequestError, useServiceView } from "../../service";
import { request } from "../../tauri";
import { afterSend } from "./Dialogs";

/** What a Pass opens next: nothing once the card moved on, else today's delivery choices (`menu`: the choice between
 * Local merge and Open PR, also while recorded decisions are open, DEC-05). */
export function passOverlay(
  ticket: string,
  passed: ReviewPassed,
  menu: boolean,
):
  | { id: "local_merge"; params: OverlayParams["local_merge"] }
  | { id: "open_pr"; params: OverlayParams["open_pr"] }
  | null {
  switch (passed) {
    case "moved":
      return null;
    case "local_merge":
      return { id: "local_merge", params: menu ? { ticket, menu: true } : { ticket } };
    case "open_pr":
      return { id: "open_pr", params: { ticket } };
    case "either":
      return { id: "local_merge", params: { ticket, menu: true } };
  }
}

/** Pass on a human check (the card's and the panel's button): a refusal is Rust's toast. */
export function passTicket(ticket: string, menu: boolean): Promise<void> {
  return (request("review.pass", { key: ticket }) as Promise<ReviewPassed>).then(
    (passed) => {
      const next = passOverlay(ticket, passed, menu);
      if (next?.id === "local_merge") openOverlay("local_merge", next.params);
      if (next?.id === "open_pr") openOverlay("open_pr", next.params);
    },
    () => undefined,
  );
}

/** The note box both dialogs share, with its label above it. */
function NoteBox({
  label,
  placeholder,
  value,
  onChange,
}: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <>
      <label className="review-label" htmlFor={id}>
        {label}
      </label>
      <TextArea id={id} value={value} placeholder={placeholder} label={label} onChange={onChange} />
    </>
  );
}

/** Sends with the dialog's busy state and the service's refusal under the box. */
function useSend(send: (clear: boolean) => Promise<unknown>, close: () => void) {
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const confirm = () => {
    if (busy) return;
    setBusy(true);
    setRefusal(null);
    send(false).then(close, (e: unknown) => {
      setRefusal(asRequestError(e).message);
      setBusy(false);
    });
  };
  return { busy, refusal, confirm };
}

/** Fail on a human check (`s03-50` → *Fail asks for comments*): the comments go to the nearest earlier Agent stage. */
export function FailDialog({ params, close }: OverlayProps<"fail">) {
  const ticket = params.ticket;
  const load = useCallback(
    () => request("review.fail_view", { key: ticket }) as Promise<TicketFailView>,
    [ticket],
  );
  const { view, error } = useServiceView(load, []);
  const [note, setNote] = useState("");
  const send = (clear: boolean): Promise<unknown> =>
    (request("review.send_back", { key: ticket, note, clear, comments: 0 }) as Promise<PanelSent>).then(
      (sent) => {
        afterSend(ticket, sent, () => send(true));
      },
    );
  const { busy, refusal, confirm } = useSend(send, close);
  useEffect(() => {
    if (error && !view) close();
  }, [error, view, close]);
  if (!view) return null;
  return (
    <Dialog
      title={view.title}
      width={480}
      hint={view.hint}
      onConfirm={confirm}
      onCancel={close}
      footer={
        <>
          <Button variant="ghost" label={view.cancel} onClick={close} />
          <Button icon="undo-2" label={view.confirm} loading={busy} onClick={confirm} />
        </>
      }
    >
      <div className="review-body">
        <NoteBox label={view.label} placeholder={view.placeholder} value={note} onChange={setNote} />
        {view.footnote !== null && <p className="review-muted">{view.footnote}</p>}
        {refusal !== null && <p className="review-error">{refusal}</p>}
      </div>
    </Dialog>
  );
}

/** Send back with PR comments (`s03-50` → *Sending an open PR back*): the note is optional. */
export function SendBackPrDialog({ params, close }: OverlayProps<"send_back_pr">) {
  const ticket = params.ticket;
  const load = useCallback(
    () => request("review.send_back_pr_view", { key: ticket }) as Promise<TicketSendBackPrView>,
    [ticket],
  );
  const { view, error } = useServiceView(load, []);
  const [note, setNote] = useState("");
  const send = (clear: boolean): Promise<unknown> =>
    (
      request("review.send_back_pr", {
        key: ticket,
        note,
        clear,
      } satisfies SendBackPrParams) as Promise<PanelSent>
    ).then((sent) => {
      afterSend(ticket, sent, () => send(true));
    });
  const { busy, refusal, confirm } = useSend(send, close);
  useEffect(() => {
    if (error && !view) close();
  }, [error, view, close]);
  if (!view) return null;
  return (
    <Dialog
      title={view.title}
      width={480}
      hint={view.hint}
      onConfirm={confirm}
      onCancel={close}
      footer={
        <>
          <Button variant="ghost" label={view.cancel} onClick={close} />
          <Button icon="undo-2" label={view.confirm} loading={busy} onClick={confirm} />
        </>
      }
    >
      <div className="review-body">
        <p className="review-text">{view.text}</p>
        <NoteBox label={view.label} placeholder={view.placeholder} value={note} onChange={setNote} />
        {refusal !== null && <p className="review-error">{refusal}</p>}
      </div>
    </Dialog>
  );
}
