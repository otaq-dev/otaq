// The header (UI-01; *02 · App shell → Header states*, *03 · Board → Header*): the shown project with its base branch
// and folder, the search entry, then the things every project shares (d0024): tokens today, the plan meter, the agents
// chip (a control with its limits popover when limits are on), running processes, the inbox button, New ticket and
// Settings. Everything it says comes from `shell.header`; each button opens its overlay by id. The row is the title
// bar: its empty parts move the window, and the title bar's ends (T-0214) give it its outer padding.
import { createRef, useCallback, useEffect, useState } from "react";

import { openOverlay, toggleOverlay, useOpenOverlays } from "../../app/overlays";
import { useRegistries } from "../../app/registry";
import { useSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import { Button, IconButton, Kbd, MetaChip, Tooltip } from "../../components";
import type { HeaderView } from "../../generated/HeaderView";
import type { PlanMeterCell } from "../../generated/PlanMeterCell";
import type { ShellParams } from "../../generated/ShellParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { TerminalsButton } from "../terminals";

/** The service events that change what the header shows. */
export const HEADER_EVENTS = [
  "projects_changed",
  "ticket_changed",
  "ticket_deleted",
  "session_state",
  "inbox_changed",
  "usage_changed",
  "run_changed",
  "settings_changed",
] as const;

/** The plan meter's reset times pass with the clock, not with an event: the header asks again this often. */
const CLOCK_REFRESH_MS = 60_000;

/** The header's last view, for the limits popover drawn by the overlay host. */
export const headerStore = createStore<HeaderView | undefined>(undefined);

/** The agents chip, which the limits popover sits under. */
export const agentsAnchor = createRef<HTMLButtonElement>();

function PlanCell({ cell }: { cell: PlanMeterCell }) {
  return (
    <>
      <span className="header-muted">{cell.label}</span>
      <span className="header-mono" data-amber={cell.amber || undefined}>
        {cell.value}
      </span>
    </>
  );
}

export interface HeaderBarProps {
  view: HeaderView;
  /** Whether the limits popover is open (the chip's outline turns accent). */
  limitsOpen?: boolean;
  /** The search entry opens the board's filter (T-0221's ⌘K handler). */
  onSearch: () => void;
}

export function HeaderBar({ view, limitsOpen = false, onSearch }: HeaderBarProps) {
  const { agents, inbox } = view;
  const chip = (
    <>
      <span className="header-dot" data-active={agents.active || undefined} />
      <span className="header-mono">{agents.count}</span>
      <span className="header-muted">{agents.label}</span>
    </>
  );
  return (
    <div className="header">
      {view.project && (
        <div className="header-project">
          <span className="header-project-name">{view.project.name}</span>
          <MetaChip icon="git-branch" label={view.project.base_branch} />
          <span className="header-project-path">{view.project.path}</span>
        </div>
      )}
      <div className="header-gap" />
      {view.project && (
        <button type="button" className="header-search" onClick={onSearch}>
          <Icon name="search" size={13} />
          <span className="header-search-text">{view.search.text}</span>
          {view.search.keys !== null && <Kbd keys={view.search.keys} />}
        </button>
      )}
      <div className="header-gap" />
      {view.usage && (
        <Tooltip text={view.usage.tooltip[0]} detail={[view.usage.tooltip[1]]}>
          <span className="header-usage">
            <Icon name="coins" size={13} />
            <span className="header-mono">{view.usage.number}</span>
            <span className="header-muted">{view.usage.label}</span>
          </span>
        </Tooltip>
      )}
      {view.plan_meter && (
        <Tooltip text={view.plan_meter.title} detail={view.plan_meter.lines} note={view.plan_meter.note}>
          <span className="header-plan">
            <PlanCell cell={view.plan_meter.five_hour} />
            <span className="header-muted">·</span>
            <PlanCell cell={view.plan_meter.week} />
          </span>
        </Tooltip>
      )}
      {agents.control ? (
        <button
          ref={agentsAnchor}
          type="button"
          className="header-chip header-agents"
          data-control
          data-open={limitsOpen || undefined}
          aria-haspopup="dialog"
          aria-expanded={limitsOpen}
          onClick={() => {
            toggleOverlay("limits");
          }}
        >
          {chip}
          <Icon name="chevron-down" size={12} />
        </button>
      ) : view.agents_tooltip ? (
        <Tooltip text={view.agents_tooltip[0]} detail={[view.agents_tooltip[1]]}>
          <span className="header-chip header-agents">{chip}</span>
        </Tooltip>
      ) : (
        <span className="header-chip header-agents">{chip}</span>
      )}
      {view.processes && (
        <button
          type="button"
          className="header-chip"
          onClick={() => {
            toggleOverlay("processes");
          }}
        >
          <Icon name="play" size={11} />
          <span className="header-mono">{view.processes.count}</span>
          <span className="header-muted">{view.processes.label}</span>
        </button>
      )}
      <div className="header-divider" />
      <Tooltip text={view.inbox_hint.text} keys={view.inbox_hint.keys ?? undefined}>
        <button
          type="button"
          className="header-inbox"
          data-active={inbox.active || undefined}
          onClick={() => {
            toggleOverlay("inbox");
          }}
        >
          <Icon name="inbox" size={14} />
          <span>{inbox.label}</span>
          <span className="header-inbox-count">{inbox.count}</span>
        </button>
      </Tooltip>
      <Tooltip text={view.new_ticket.text} keys={view.new_ticket.keys ?? undefined}>
        <Button
          label={view.new_ticket.text}
          icon="plus"
          onClick={() => {
            openOverlay("new_ticket");
          }}
        />
      </Tooltip>
      <TerminalsButton />
      <Tooltip text={view.settings.text} keys={view.settings.keys ?? undefined}>
        <IconButton
          icon="settings"
          label={view.settings.text}
          onClick={() => {
            openOverlay("settings");
          }}
        />
      </Tooltip>
    </div>
  );
}

/** The header slot: the view for the shown project, refetched on every event that changes a count. */
export function ShellHeader() {
  const app = useRegistries();
  const { project } = useSelection();
  const limitsOpen = useOpenOverlays().some((o) => o.id === "limits");
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const timer = window.setInterval(() => {
      setTick((n) => n + 1);
    }, CLOCK_REFRESH_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, []);
  const load = useCallback(() => {
    const params: ShellParams = { project };
    return request("shell.header", params) as Promise<HeaderView>;
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new tick is a new load: the plan meter's "resets in" moves with the clock
  }, [project, tick]);
  const { view } = useServiceView(load, HEADER_EVENTS);
  useEffect(() => {
    headerStore.set(view);
  }, [view]);
  if (view === undefined) return null;
  return (
    <HeaderBar
      view={view}
      limitsOpen={limitsOpen}
      onSearch={() => {
        app.shortcuts.run({ action: "search" });
      }}
    />
  );
}
