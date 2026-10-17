// The History tab (UI-04, REC-03; d0045; *05 → SHOP-20 · History*, *16 → History · Claude session IDs*): every
// session with its time, duration, tokens and full Claude session ID (copyable, so `claude --resume <id>` finds
// the conversation), and the ticket's events, newest first; one entry per validation round (VAL-12; *05 → History ·
// validation rounds*) that opens its report under it, and the validation total.
import { Fragment, useCallback, useState } from "react";

import type { TicketProps } from "../../app/registry";
import { Tooltip } from "../../components";
import type { HistoryView } from "../../generated/HistoryView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { SECTION_EVENTS } from "./logic";
import { ReportBody } from "./Report";

const ignore = () => undefined;

function CopyId({ id, hint, copied }: { id: string; hint: string; copied: string }) {
  const [done, setDone] = useState(false);
  return (
    <Tooltip text={done ? copied : hint}>
      <button
        type="button"
        className="history-copy"
        aria-label={hint}
        onClick={() => {
          navigator.clipboard.writeText(id).then(() => {
            setDone(true);
          }, ignore);
        }}
      >
        <Icon name={done ? "check" : "copy"} size={11} />
      </button>
    </Tooltip>
  );
}

export function HistoryBody({ view, opened = [] }: { view: HistoryView; opened?: readonly number[] }) {
  const [event, when, duration, tokens] = view.columns;
  // The rounds whose report is open (by row).
  const [open, setOpen] = useState<ReadonlySet<number>>(new Set(opened));
  const toggle = (i: number) => {
    const next = new Set(open);
    if (!next.delete(i)) next.add(i);
    setOpen(next);
  };
  return (
    <div className="history" role="table" aria-label="History">
      <div className="history-row history-head" role="row">
        <span className="history-icon" />
        <span className="history-event" role="columnheader">
          {event}
        </span>
        <span className="history-cell" role="columnheader">
          {when}
        </span>
        <span className="history-cell history-duration" role="columnheader">
          {duration}
        </span>
        <span className="history-cell" role="columnheader">
          {tokens}
        </span>
      </div>
      {view.rows.length === 0 && <p className="history-empty">{view.empty}</p>}
      {view.rows.map((row, i) => (
        <Fragment key={`${row.when}-${row.title}-${String(i)}`}>
          <div className="history-row" role="row" data-opens={row.report !== null || undefined}>
            <span className="history-icon" data-palette={row.palette ?? undefined}>
              <Icon name={row.icon} size={13} />
            </span>
            <span className="history-event" role="cell">
              {row.report !== null ? (
                <button
                  type="button"
                  className="history-title history-round"
                  aria-expanded={open.has(i)}
                  onClick={() => {
                    toggle(i);
                  }}
                >
                  {row.title}
                  <Icon name={open.has(i) ? "chevron-down" : "chevron-right"} size={11} />
                </button>
              ) : (
                <span className="history-title">{row.title}</span>
              )}
              {row.detail !== null && (
                <span className="history-detail">
                  <span className="history-detail-text" data-mono={row.tool_session_id !== null || undefined}>
                    {row.detail}
                  </span>
                  {row.tool_session_id !== null && (
                    <CopyId id={row.tool_session_id} hint={view.copy_hint} copied={view.copied} />
                  )}
                </span>
              )}
            </span>
            <span className="history-cell" role="cell">
              {row.when}
            </span>
            <span className="history-cell history-duration" role="cell">
              {row.duration}
            </span>
            <span className="history-cell" role="cell">
              {row.tokens}
            </span>
          </div>
          {row.report !== null && open.has(i) && (
            <div className="history-report">
              <ReportBody report={row.report} ticket={view.key} />
            </div>
          )}
        </Fragment>
      ))}
      {view.total !== null && (
        <div className="history-total">
          <span>{view.total.label}</span>
          <span className="history-total-tokens">{view.total.tokens}</span>
        </div>
      )}
    </div>
  );
}

export function HistoryTab({ ticket }: TicketProps) {
  const load = useCallback(
    () => request("sections.history", { key: ticket }) as Promise<HistoryView | null>,
    [ticket],
  );
  const { view } = useServiceView(load, SECTION_EVENTS);
  return view ? <HistoryBody view={view} /> : null;
}
