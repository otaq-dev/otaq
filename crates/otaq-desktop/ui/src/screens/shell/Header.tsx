// The header (SHL-02, SHL-03; d0087; *02 · App shell → Row · Header states (0086 · V)*): the shown project with its base
// branch, the search entry, Activity (agents and processes across all projects, the amber usage warning) and the inbox
// button. Everything it says comes from `shell.header`; each button opens its overlay by id. The row is the title
// bar: its empty parts move the window, and the title bar's ends (T-0214) give it its outer padding. (T-0325 changed
// the view; T-0329 draws the V header and the Activity popover.)
import { createRef, type ReactNode, useCallback, useEffect, useState } from "react";

import { toggleOverlay, useOpenOverlays } from "../../app/overlays";
import { useRegistries } from "../../app/registry";
import { useSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import { Kbd, MetaChip, Tooltip } from "../../components";
import type { HeaderView } from "../../generated/HeaderView";
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

/** Activity's reset times pass with the clock, not with an event: the header asks again this often. */
const CLOCK_REFRESH_MS = 60_000;

/** The header's last view, for the limits popover drawn by the overlay host. */
export const headerStore = createStore<HeaderView | undefined>(undefined);

/** Activity, which the limits popover sits under. */
export const agentsAnchor = createRef<HTMLButtonElement>();
/** Activity's "▷ 3", which the processes menu sits under (T-0232). */
export const processesAnchor = createRef<HTMLButtonElement>();

export interface HeaderBarProps {
  view: HeaderView;
  /** Whether the limits popover is open (the chip's outline turns accent). */
  limitsOpen?: boolean;
  /** The search entry opens the board's filter (T-0221's ⌘K handler). */
  onSearch: () => void;
  /** The board's search and filter field (the `header_search` slot, T-0221), drawn in place of the entry. */
  search?: ReactNode;
}

export function HeaderBar({ view, limitsOpen = false, onSearch, search }: HeaderBarProps) {
  const { activity, inbox } = view;
  const { agents } = activity;
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
      {view.project && search}
      {view.project && !search && (
        <button type="button" className="header-search" onClick={onSearch}>
          <Icon name="search" size={13} />
          <span className="header-search-text">{view.search.text}</span>
          {view.search.keys !== null && <Kbd keys={view.search.keys} />}
        </button>
      )}
      <div className="header-gap" />
      <Tooltip text={activity.hover.title} detail={activity.hover.lines}>
        <button
          ref={agentsAnchor}
          type="button"
          className="header-chip header-agents"
          data-control={agents.control || undefined}
          data-open={limitsOpen || undefined}
          aria-label={activity.label}
          onClick={() => {
            if (agents.control) toggleOverlay("limits");
            else if (activity.processes !== null) toggleOverlay("processes");
          }}
        >
          <span className="header-dot" data-active={agents.active || undefined} />
          <span className="header-mono">{agents.count}</span>
          <span className="header-muted">{agents.label}</span>
          {activity.processes !== null && (
            <span ref={processesAnchor} className="header-muted">
              · ▷ {activity.processes}
            </span>
          )}
          {activity.warning && (
            <span className="header-mono" data-amber>
              {activity.warning.text}
            </span>
          )}
        </button>
      </Tooltip>
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
      <TerminalsButton />
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
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new tick is a new load: Activity's reset times move with the clock
  }, [project, tick]);
  const { view } = useServiceView(load, HEADER_EVENTS);
  useEffect(() => {
    headerStore.set(view);
  }, [view]);
  if (view === undefined) return null;
  const Search = app.slots.get("header_search")?.Component;
  return (
    <HeaderBar
      view={view}
      limitsOpen={limitsOpen}
      search={Search && <Search />}
      onSearch={() => {
        app.shortcuts.run({ action: "search" });
      }}
    />
  );
}
