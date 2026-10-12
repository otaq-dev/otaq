// The History tab (UI-04, REC-03; d0045; *05 → SHOP-20 · History*, *16 → History · Claude session IDs*): every
// session with its time, duration, tokens and full Claude session ID (copyable, so `claude --resume <id>` finds
// the conversation), and the ticket's events, newest first.
import { useCallback, useState } from "react";

import type { TicketProps } from "../../app/registry";
import { Tooltip } from "../../components";
import type { HistoryView } from "../../generated/HistoryView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { SECTION_EVENTS } from "./logic";

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

export function HistoryBody({ view }: { view: HistoryView }) {
  const [event, when, duration, tokens] = view.columns;
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
        <div key={`${row.when}-${row.title}-${String(i)}`} className="history-row" role="row">
          <span className="history-icon">
            <Icon name={row.is_session ? "square-terminal" : "history"} size={13} />
          </span>
          <span className="history-event" role="cell">
            <span className="history-title">{row.title}</span>
            {row.detail !== null && (
              <span className="history-detail">
                <span className="history-detail-text" data-mono={row.claude_session_id !== null || undefined}>
                  {row.detail}
                </span>
                {row.claude_session_id !== null && (
                  <CopyId id={row.claude_session_id} hint={view.copy_hint} copied={view.copied} />
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
      ))}
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
