// The after-restart summary sheet (*16 → Opening after a reboot*; REC-01, REC-06, d0045, d0022): beside the "sessions
// were interrupted" banner, the interrupted tickets with a checkbox (all ticked) and Resume each, Resume selected,
// what each Resume did (resumed, or Couldn't resume with Retry and Start from scratch) and what the restart left
// alone. It opens by itself once per service start and from the banner's Review & resume; Later (or Esc) closes it
// and the tickets stay in the inbox. Nothing resumes without a click. What it says is `recovery.summary`'s
// (`after_restart.rs`); T-0244 adds the Project-agent and validating rows.
import { useCallback, useEffect, useState } from "react";

import { openOverlay } from "../../app/overlays";
import type { OverlayProps } from "../../app/registry";
import { Button } from "../../components";
import type { AfterRestartParams } from "../../generated/AfterRestartParams";
import type { AfterRestartRow } from "../../generated/AfterRestartRow";
import type { AfterRestartSummary } from "../../generated/AfterRestartSummary";
import type { RecoveryResumeParams } from "../../generated/RecoveryResumeParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { setUnchecked } from "./logic";

export interface SummaryBodyProps {
  view: AfterRestartSummary;
  /** Tickets whose Resume the page is waiting for. */
  resuming: ReadonlySet<string>;
  onCheck: (keys: readonly string[], checked: boolean) => void;
  onResume: (keys: readonly string[]) => void;
  onRetry: (key: string) => void;
  onUnfold: () => void;
  onLater: () => void;
}

function Row({
  row,
  view,
  resuming,
  onCheck,
  onResume,
  onRetry,
}: { row: AfterRestartRow } & Omit<SummaryBodyProps, "onUnfold" | "onLater">) {
  const waiting = resuming.has(row.key) && row.state === "interrupted";
  if (waiting || row.state !== "interrupted") {
    const state = waiting ? "resuming" : row.state;
    return (
      <div className="summary-state" data-state={state}>
        <Icon
          name={waiting ? "loader-circle" : row.state === "failed" ? "circle-x" : "circle-check"}
          size={15}
        />
        <div className="summary-state-text">
          <strong>{waiting ? row.resuming : row.headline}</strong>
          <span>{waiting ? row.resuming_detail : row.detail}</span>
          {row.state === "failed" && (
            <span className="summary-state-buttons">
              <Button
                compact
                variant="secondary"
                icon="rotate-ccw"
                label={view.retry}
                onClick={() => {
                  onRetry(row.key);
                }}
              />
              <Button
                compact
                icon="rotate-ccw"
                label={view.scratch}
                onClick={() => {
                  openOverlay("start_from_scratch", { ticket: row.key });
                }}
              />
            </span>
          )}
        </div>
      </div>
    );
  }
  return (
    <div className="summary-row">
      <input
        type="checkbox"
        className="checkbox"
        aria-label={row.key}
        checked={row.checked}
        onChange={(e) => {
          onCheck([row.key], e.target.checked);
        }}
      />
      <span className="summary-project">{row.project}</span>
      <span className="summary-key">{row.key}</span>
      <span className="summary-text">
        <span className="summary-title">{row.title}</span>
        <span className="summary-detail">{row.detail}</span>
      </span>
      <Button
        compact
        variant="secondary"
        icon="play"
        label={view.resume}
        onClick={() => {
          onResume([row.key]);
        }}
      />
    </div>
  );
}

/** The sheet for a loaded summary (also what the markup tests render). */
export function SummaryBody(props: SummaryBodyProps) {
  const { view, resuming, onCheck, onResume, onUnfold, onLater } = props;
  const { more } = view;
  return (
    <>
      <div className="summary-body">
        <h2 className="summary-heading">{view.title}</h2>
        <p className="summary-intro">{view.text}</p>
        <div className="summary-list">
          {view.rows.map((row) => (
            <Row key={row.key} row={row} {...props} />
          ))}
          {more && (
            <div className="summary-row">
              <input
                type="checkbox"
                className="checkbox"
                aria-label={more.count}
                checked={more.checked}
                disabled={more.resumable.length === 0}
                onChange={(e) => {
                  onCheck(more.resumable, e.target.checked);
                }}
              />
              <span className="summary-key">{more.count}</span>
              <span className="summary-text">
                <span className="summary-title">{more.keys}</span>
                <button type="button" className="summary-unfold" onClick={onUnfold}>
                  {more.detail}
                </button>
              </span>
              <Button
                compact
                variant="secondary"
                icon="play"
                label={view.resume}
                disabled={more.resumable.length === 0 || more.resumable.every((k) => resuming.has(k))}
                onClick={() => {
                  onResume(more.resumable);
                }}
              />
            </div>
          )}
        </div>
        {view.notes.length > 0 && (
          <div className="summary-notes">
            <span className="summary-notes-title">{view.notes_title}</span>
            {view.notes.map((note) => (
              <p key={note}>{note}</p>
            ))}
          </div>
        )}
      </div>
      <div className="summary-foot">
        <span>{view.foot}</span>
        <span className="cleanup-spacer" />
        <Button variant="ghost" label={view.later} onClick={onLater} />
        <Button
          icon="play"
          label={view.resume_selected}
          disabled={view.selected.length === 0}
          onClick={() => {
            onResume(view.selected);
          }}
        />
      </div>
    </>
  );
}

export function AfterRestart({ close }: OverlayProps<"after_restart">) {
  const [unchecked, setUncheckedKeys] = useState<ReadonlySet<string>>(new Set());
  const [unfolded, setUnfolded] = useState(false);
  const [resuming, setResuming] = useState<ReadonlySet<string>>(new Set());
  const load = useCallback(() => {
    const params: AfterRestartParams = { unchecked: [...unchecked], unfolded };
    return request("recovery.summary", params) as Promise<AfterRestartSummary | null>;
  }, [unchecked, unfolded]);
  const { view } = useServiceView(load, ["ticket_changed"]);
  // Esc is Later. A dialog opened from the sheet (Start from scratch) keeps its own Esc: it stops it on its way up.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
    };
  }, [close]);
  useEffect(() => {
    if (view === null) close();
  }, [view, close]);
  if (!view) return null;
  return (
    <div className="summary" role="dialog" aria-label={view.title}>
      <SummaryBody
        view={view}
        resuming={resuming}
        onCheck={(keys, checked) => {
          setUncheckedKeys((u) => setUnchecked(u, keys, checked));
        }}
        onResume={(keys) => {
          setResuming((r) => new Set([...r, ...keys]));
          const params: RecoveryResumeParams = { keys: [...keys] };
          request("recovery.resume", params)
            .catch(() => undefined)
            .finally(() => {
              setResuming((r) => {
                const next = new Set(r);
                for (const k of keys) next.delete(k);
                return next;
              });
            });
        }}
        onRetry={(key) => {
          request("recovery.retry", { key }).catch(() => undefined);
        }}
        onUnfold={() => {
          setUnfolded(true);
        }}
        onLater={close}
      />
    </div>
  );
}
