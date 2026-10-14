// The epic panel (EPC-01, EPC-04; d0059; *05 → Epic panel · SHOP-40 Wishlist*): External ID, progress, its local
// branch and the child list (a row opens that ticket), Add ticket to epic with the next key, and the rule for when
// the epic moves to Ready for Review. Shown only for an epic.
import { useCallback } from "react";

import { openOverlay } from "../../app/overlays";
import type { TicketProps } from "../../app/registry";
import { openTicket } from "../../app/selection";
import { StatusPill } from "../../components";
import type { EpicView } from "../../generated/EpicView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { progressPercent, SECTION_EVENTS } from "./logic";

export function EpicBody({ ticket, view }: { ticket: string; view: EpicView }) {
  return (
    <section className="section section-epic" aria-label={ticket}>
      <div className="epic-meta">
        {view.external_id !== null && <span className="epic-chip">{view.external_id}</span>}
        <span
          className="epic-progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={view.total}
          aria-valuenow={view.merged}
          aria-label={view.progress}
        >
          <span
            className="epic-progress-fill"
            style={{ width: `${String(progressPercent(view.merged, view.total))}%` }}
          />
        </span>
        <span className="epic-count">{view.progress}</span>
        <span className="section-spacer" />
        <span className="epic-branch">
          <Icon name="git-branch" size={11} />
          {view.branch}
        </span>
      </div>
      <div className="epic-heading">{view.heading}</div>
      <div className="epic-children">
        {view.children.map((child) => (
          <button
            key={child.key}
            type="button"
            className="epic-child"
            data-finished={child.finished || undefined}
            onClick={() => {
              openTicket(child.key);
            }}
          >
            <span className="epic-child-key">{child.key}</span>
            <span className="epic-child-title">{child.title}</span>
            {child.deps !== null && <span className="epic-child-deps">{child.deps}</span>}
            <StatusPill pill={child.pill} />
          </button>
        ))}
      </div>
      <div className="epic-add">
        <button
          type="button"
          className="section-link"
          onClick={() => {
            openOverlay("new_ticket", { epic: ticket });
          }}
        >
          <Icon name="plus" size={12} />
          {view.add}
        </button>
        {view.next_key !== null && <span className="epic-next">{view.next_key}</span>}
      </div>
      <p className="epic-note">{view.note}</p>
    </section>
  );
}

export function EpicSection({ ticket }: TicketProps) {
  const load = useCallback(
    () => request("sections.epic", { key: ticket }) as Promise<EpicView | null>,
    [ticket],
  );
  const { view } = useServiceView(load, SECTION_EVENTS);
  return view ? <EpicBody ticket={ticket} view={view} /> : null;
}
