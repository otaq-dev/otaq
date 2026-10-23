// Starting a blocked ticket (DPV-03, d0049; *24 → Dragging a blocked ticket* `s24-11`): a drag past Ready for Dev, ⌘D,
// a row's Start or the sheet's Move… on a ticket whose parents haven't merged. The ticket already waits in Ready for
// Dev (the service refuses the start); the dialog says what it waits for, and Keep it waiting (focused, Esc) is the
// only button. To start it, the user removes the dependency in the ticket's Dependencies section first. Its words
// are `dependencies::dialog`'s, from `board.drop`'s answer.
import type { OverlayProps } from "../../app/registry";
import { ConfirmDialog } from "../../components";
import type { BlockedDialog as BlockedView } from "../../generated/BlockedDialog";

/** The dialog's body: what it waits for, then how to start it. The gallery draws it too. */
export function BlockedBody({ view }: { view: BlockedView }) {
  return (
    <>
      <p className="dialog-text">{view.body}</p>
      <p className="dialog-text board-blocked-hint">{view.hint}</p>
    </>
  );
}

export function BlockedDialog({ params: view, close }: OverlayProps<"blocked">) {
  return (
    <ConfirmDialog
      title={view.title}
      body={<BlockedBody view={view} />}
      confirmLabel={null}
      cancelLabel={view.keep}
      destructive={false}
      width={480}
      onConfirm={close}
      onCancel={close}
    />
  );
}
