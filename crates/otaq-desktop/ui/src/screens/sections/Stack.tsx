// The Stack section (DEP-05, d0006; *05 → SHOP-20 · Ready for Dev, waits for merge*, *12 → Stack list · detail*):
// the chain with each ticket's status and "waits for" line; a row opens that ticket. Without links it is one line
// ("No dependencies · nothing waits on SHOP-12"); "+ Add dependency" opens the Ticket tab's form.
import { useCallback, useState } from "react";

import type { TicketProps } from "../../app/registry";
import { openTicket, requestTicketEdit } from "../../app/selection";
import { StackRow } from "../../components";
import type { StackView } from "../../generated/StackView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { SECTION_EVENTS, stackOpen } from "./logic";

export function StackBody({ ticket, view }: { ticket: string; view: StackView }) {
  const [toggled, setToggled] = useState<boolean | null>(null);
  const open = stackOpen(toggled, view.rows.length);
  return (
    <section className="section section-stack" aria-label={view.title}>
      <div className="section-head">
        <button
          type="button"
          className="section-toggle"
          aria-expanded={open}
          disabled={view.rows.length === 0}
          title={open ? view.hide_hint : view.show_hint}
          onClick={() => {
            setToggled(!open);
          }}
        >
          <Icon name={open ? "chevron-down" : "chevron-right"} size={12} />
          <Icon name="link-2" size={12} />
          <span className="section-title">{view.title}</span>
          <span className="section-summary">{open ? "" : view.summary}</span>
        </button>
        {open && <span className="section-count">{view.count}</span>}
        <button
          type="button"
          className="section-link"
          onClick={() => {
            requestTicketEdit(ticket);
          }}
        >
          {view.add_dependency}
        </button>
      </div>
      {open && (
        <div className="section-stack-rows">
          {view.rows.map((row, i) => (
            <StackRow
              key={`${row.key}-${String(i)}`}
              depth={row.depth}
              ticketKey={row.key}
              title={row.title}
              waits={row.waits}
              pill={row.pill}
              current={row.current}
              onSelect={() => {
                if (!row.current) openTicket(row.key);
              }}
            />
          ))}
        </div>
      )}
    </section>
  );
}

export function StackSection({ ticket }: TicketProps) {
  const load = useCallback(
    () => request("sections.stack", { key: ticket }) as Promise<StackView | null>,
    [ticket],
  );
  const { view } = useServiceView(load, SECTION_EVENTS);
  if (!view) return null;
  // A new ticket starts with the list opened by its own links.
  return <StackBody key={ticket} ticket={ticket} view={view} />;
}
