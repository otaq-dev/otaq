// Resume all's confirmation (*10 → Resume all · confirm*): the interrupted sessions it resumes, with their tickets,
// and the start-interrupted ones it leaves out. Opened from the inbox's group head and the "sessions were
// interrupted" banner (T-0231); the confirm sends `notices.resume_all`, so nothing resumes without the click (d0045).
import { useCallback, useEffect } from "react";

import type { OverlayProps } from "../../app/registry";
import { ConfirmDialog } from "../../components";
import type { InboxResumeAllDialog } from "../../generated/InboxResumeAllDialog";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

/** The dialog for a loaded confirmation (also what the markup tests render). */
export function ResumeAllBody({ dialog, close }: { dialog: InboxResumeAllDialog; close: () => void }) {
  return (
    <ConfirmDialog
      title={dialog.title}
      hint={dialog.cancel_hint}
      body={
        <>
          <p className="dialog-text">{dialog.text}</p>
          <ul className="inbox-resume-list">
            {dialog.included.map((t) => (
              <li key={t.key}>
                <Icon name="zap-off" size={13} />
                <span className="inbox-key">{t.key}</span>
                <span>{t.title}</span>
              </li>
            ))}
          </ul>
          {dialog.not_included !== null && (
            <p className="inbox-resume-left-out">
              <Icon name="circle-pause" size={13} />
              <span>{dialog.not_included}</span>
            </p>
          )}
        </>
      }
      confirmLabel={dialog.confirm}
      confirmIcon="play"
      onConfirm={() => {
        close();
        request("notices.resume_all").catch(() => undefined);
      }}
      onCancel={close}
    />
  );
}

/** The `resume_all` overlay; it closes at once when nothing is left to resume. */
export function ResumeAll({ close }: OverlayProps<"resume_all">) {
  const load = useCallback(
    () => request("inbox.resume_all_confirm") as Promise<InboxResumeAllDialog | null>,
    [],
  );
  const { view } = useServiceView(load, ["inbox_changed"]);
  useEffect(() => {
    if (view === null) close();
  }, [view, close]);
  return view ? <ResumeAllBody dialog={view} close={close} /> : null;
}
