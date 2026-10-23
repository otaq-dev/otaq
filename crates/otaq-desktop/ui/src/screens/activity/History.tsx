// The sheet's Activity (SHT-05, T-0466; canvas `s25-07`; History: UI-04, REC-03, CMT-14; d0045, d0109): the
// ticket's story oldest first under the filters All · Comments · Stages · Description: every session with its time,
// duration, tokens and full tool session ID (copyable, so `claude --resume <id>` finds the conversation), the ticket's
// events, one entry per validation round (VAL-12) and the validation total. Description changes and comments carry
// their author's mark and open under the row: a before / after ("DESCRIPTION", "COMMENT"; a description edit opens
// by itself), or what a comment said with its report, which opens in the report view (a deleted comment's too). The
// rows, their kinds and the filters are Rust's (`sections.history`, `history.rs`); the page keeps the filter picked
// and the rows opened.
import { Fragment, useCallback, useState } from "react";

import type { TicketProps } from "../../app/registry";
import { Tooltip } from "../../components";
import type { HistoryOpens } from "../../generated/HistoryOpens";
import type { HistoryView } from "../../generated/HistoryView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { Mark, ReportChip } from "../editor/CommentThread";
import { openReport } from "../editor/reportOpener";
import type { ActivityFilter } from "../../generated/ActivityFilter";
import { SECTION_EVENTS } from "../sections/logic";

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

/** What a description or comment row opens: its label, "Before / after", the lines and the comment's report. */
function Opened({ opens, ticket }: { opens: HistoryOpens; ticket: string }) {
  return (
    <div className="history-opens">
      <div className="history-opens-head">
        <span className="history-opens-label">{opens.label}</span>
        {opens.caption !== null && <span className="history-opens-caption">{opens.caption}</span>}
      </div>
      {opens.lines.map((l, i) => (
        <div key={i} className="history-opens-line" data-kind={l.kind}>
          <span className="history-opens-sign">{l.sign}</span>
          <span className="history-opens-text">{l.text}</span>
        </div>
      ))}
      {opens.report !== null && (
        <ReportChip
          chip={opens.report.chip}
          onOpen={() => {
            const n = opens.report?.n;
            if (n !== undefined) openReport({ kind: "comment", ticket, n });
          }}
        />
      )}
    </div>
  );
}

/** All · Comments · Stages · Description, and "oldest first". */
function Filters({
  view,
  filter,
  onFilter,
}: {
  view: HistoryView;
  filter: ActivityFilter;
  onFilter: (filter: ActivityFilter) => void;
}) {
  return (
    <div className="activity-filters">
      <div className="segmented activity-filter" role="radiogroup" aria-label="Activity">
        {view.filters.map((f) => (
          <button
            key={f.filter}
            type="button"
            role="radio"
            className="segment"
            aria-checked={f.filter === filter}
            onClick={() => {
              onFilter(f.filter);
            }}
          >
            {f.label}
          </button>
        ))}
      </div>
      <span className="activity-order">{view.order}</span>
    </div>
  );
}

/** The rows `filter` shows: every row under All, else the rows of its kind. */
export function shownRows(view: HistoryView, filter: ActivityFilter): number[] {
  const kind = view.filters.find((f) => f.filter === filter)?.kind ?? null;
  return view.rows.flatMap((row, i) => (kind === null || row.kind === kind ? [i] : []));
}

export function HistoryBody({
  view,
  opened,
  filter: initial = "all",
}: {
  view: HistoryView;
  /** The rows open at first; by default the description edits, whose before / after is the point. */
  opened?: readonly number[];
  filter?: ActivityFilter;
}) {
  const [event, when, duration, tokens] = view.columns;
  const [filter, setFilter] = useState<ActivityFilter>(initial);
  // The rows that are open (by row).
  const [open, setOpen] = useState<ReadonlySet<number>>(
    () => new Set(opened ?? view.rows.flatMap((row, i) => (row.kind === "description" ? [i] : []))),
  );
  const toggle = (i: number) => {
    const next = new Set(open);
    if (!next.delete(i)) next.add(i);
    setOpen(next);
  };
  const shown = shownRows(view, filter);
  return (
    <div className="activity">
      <Filters view={view} filter={filter} onFilter={setFilter} />
      <div className="history" role="table" aria-label="Activity">
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
        {shown.length === 0 && <p className="history-empty">{view.empty}</p>}
        {shown.map((i) => {
          const row = view.rows[i];
          if (row === undefined) return null;
          return (
            <Fragment key={`${row.when}-${row.title}-${String(i)}`}>
              <div className="history-row" role="row" data-opens={row.opens !== null || undefined}>
                {row.mark !== null ? (
                  <span className="history-icon history-mark">
                    <Mark row={{ mark: row.mark }} />
                  </span>
                ) : (
                  <span className="history-icon" data-palette={row.palette ?? undefined}>
                    <Icon name={row.icon} size={13} />
                  </span>
                )}
                <span className="history-event" role="cell">
                  {row.opens !== null ? (
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
                      <span
                        className="history-detail-text"
                        data-mono={row.tool_session_id !== null || undefined}
                      >
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
              {row.opens !== null && open.has(i) && (
                <div className="history-report">
                  <Opened opens={row.opens} ticket={view.key} />
                </div>
              )}
            </Fragment>
          );
        })}
        {view.total !== null && (
          <div className="history-total">
            <span>{view.total.label}</span>
            <span className="history-total-tokens">{view.total.tokens}</span>
          </div>
        )}
      </div>
    </div>
  );
}

/** The tab registered in the sheet. */
export function ActivityTab({ ticket }: TicketProps) {
  const load = useCallback(
    () => request("sections.history", { key: ticket }) as Promise<HistoryView | null>,
    [ticket],
  );
  const { view } = useServiceView(load, SECTION_EVENTS);
  return view ? <HistoryBody view={view} /> : null;
}
