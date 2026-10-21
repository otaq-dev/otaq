// The Apply now window (T-0439, APN-02; *11 → Row · Apply now* `s11-17`/`s11-18`): "Apply the flow change now?" over
// the whole window, from the flow change banner. Every word and state comes from `apply_now.view`; this keeps the
// picks until Apply now, whether Keep their place is opened, the line a refresh wrote, and the request in flight.
// Esc and Cancel close it and nothing changes.
import { useCallback, useEffect, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { Button, Dialog, Select } from "../../components";
import type { ApplyNowChanged } from "../../generated/ApplyNowChanged";
import type { ApplyNowMove } from "../../generated/ApplyNowMove";
import type { ApplyNowView } from "../../generated/ApplyNowView";
import { asRequestError, useServiceView } from "../../service";
import { Icon } from "../../theme/Icon";
import { applyNow, type Ask, loadApplyNow, type Picks, REFETCH_ON, withPick } from "./applyNowLogic";
import "./applyNow.css";

function MoveLine({ row, onPick }: { row: ApplyNowMove; onPick: (id: string) => void }) {
  return (
    <div className="apply-now-row">
      <div className="apply-now-ticket">
        <div className="apply-now-ticket-name">
          <span className="apply-now-key">{row.ticket}</span>
          <span className="apply-now-title">{row.title}</span>
        </div>
        <span className="apply-now-line">{row.line}</span>
      </div>
      <Icon name="arrow-right" size={14} />
      <div className="apply-now-picker">
        <Select
          label={`Where ${row.ticket} goes`}
          value={row.picked_name}
          items={row.choices}
          onChange={onPick}
        />
      </div>
    </div>
  );
}

/** The window's body and footer for one view. */
export function ApplyNowDialog({
  view,
  busy,
  error,
  onPick,
  onApply,
  onCancel,
}: {
  view: ApplyNowView;
  busy: boolean;
  error: string | null;
  onPick: (ticket: string, id: string) => void;
  onApply: () => void;
  onCancel: () => void;
}) {
  const [keepOpen, setKeepOpen] = useState(false);
  const collapsed = view.keep_collapsed !== null && !keepOpen;
  return (
    <Dialog
      title={view.title}
      width={560}
      onCancel={onCancel}
      onConfirm={busy ? undefined : onApply}
      footer={
        <>
          <Button variant="ghost" label={view.cancel} onClick={onCancel} />
          <Button variant="primary" icon="play" label={view.apply} loading={busy} onClick={onApply} />
        </>
      }
    >
      <p className="dialog-text apply-now-summary">{view.summary}</p>
      {view.changed !== null && (
        <p className="apply-now-changed" role="status">
          <Icon name="refresh-cw" size={13} />
          <span>{view.changed}</span>
        </p>
      )}
      {view.moves.length > 0 && (
        <section className="apply-now-section" aria-label={view.moves_label}>
          <h3 className="apply-now-label">{view.moves_label}</h3>
          {view.moves.map((row) => (
            <MoveLine
              key={row.ticket}
              row={row}
              onPick={(id) => {
                onPick(row.ticket, id);
              }}
            />
          ))}
        </section>
      )}
      {view.keep_label !== null && (
        <section className="apply-now-section" aria-label={view.keep_label}>
          {view.keep_collapsed === null ? (
            <h3 className="apply-now-label apply-now-keep-head">{view.keep_label}</h3>
          ) : (
            <button
              type="button"
              className="apply-now-keep-head"
              aria-expanded={!collapsed}
              onClick={() => {
                setKeepOpen((o) => !o);
              }}
            >
              <Icon name={collapsed ? "chevron-right" : "chevron-down"} size={13} />
              <span className="apply-now-label">{view.keep_label}</span>
              <span className="apply-now-keep-fill" />
              {collapsed && <span className="apply-now-keep-line">{view.keep_collapsed}</span>}
            </button>
          )}
          {!collapsed &&
            view.keep.map((row) => (
              <div key={row.ticket} className="apply-now-row">
                <div className="apply-now-ticket">
                  <div className="apply-now-ticket-name">
                    <span className="apply-now-key">{row.ticket}</span>
                    <span className="apply-now-title">{row.title}</span>
                  </div>
                  <span className="apply-now-line">{row.line}</span>
                </div>
              </div>
            ))}
        </section>
      )}
      {view.hint !== null && (
        <p className="apply-now-hint">
          <Icon name="info" size={13} />
          <span>{view.hint}</span>
        </p>
      )}
      {error !== null && (
        <p className="apply-now-error" role="alert">
          {error}
        </p>
      )}
    </Dialog>
  );
}

/** The window on `project`'s waiting change; it closes itself once nothing waits. */
export function ApplyNowWindow({ project, close, ask }: { project: string; close: () => void; ask?: Ask }) {
  const [picks, setPicks] = useState<Picks>({});
  const [changed, setChanged] = useState<ApplyNowChanged | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => loadApplyNow(project, picks, changed, ask), [project, picks, changed, ask]);
  const state = useServiceView(load, REFETCH_ON);
  const view = state.view?.view ?? null;
  const gone = state.view !== undefined && state.view.view === null;
  useEffect(() => {
    if (gone) close();
  }, [gone, close]);
  const apply = useCallback(() => {
    if (view === null || busy) return;
    setBusy(true);
    setError(null);
    applyNow(view, picks, ask).then(
      (answer) => {
        setBusy(false);
        if (answer.close) close();
        else setChanged(answer.changed);
      },
      (e: unknown) => {
        setBusy(false);
        setError(asRequestError(e).message);
      },
    );
  }, [view, picks, busy, close, ask]);
  if (view === null) return null;
  return (
    <ApplyNowDialog
      view={view}
      busy={busy}
      error={error}
      onPick={(ticket, id) => {
        setPicks((p) => withPick(p, ticket, id));
      }}
      onApply={apply}
      onCancel={close}
    />
  );
}

export function ApplyNowOverlay({ params, close }: OverlayProps<"apply_now">) {
  // Another project's change starts afresh: no picks.
  return <ApplyNowWindow key={params.project} project={params.project} close={close} />;
}
