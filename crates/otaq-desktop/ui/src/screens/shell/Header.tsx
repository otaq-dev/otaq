// The header (SHL-02, SHL-03; d0087; *02 · App shell → Row · Title bars (0086 · V)*, *Row · Header states (0086 ·
// V)*): four items. The shown project with its base branch (the folder on the name's hover, "Copy path" in its menu),
// the search entry, Activity (agents and processes across all projects, the amber usage warning; a click or ⇧⌘U opens
// its popover) and Needs you. Everything it says comes from `shell.header`. The row is the title bar: its empty parts
// move the window, and the title bar's ends (T-0214) give it its outer padding.
import { createRef, type ReactNode, type RefObject, useCallback, useEffect, useRef, useState } from "react";

import { toggleOverlay, useOpenOverlays } from "../../app/overlays";
import { useRegistries } from "../../app/registry";
import { useSelection } from "../../app/selection";
import { createStore } from "../../app/store";
import { AnchoredMenu, Kbd, MetaChip, Tooltip } from "../../components";
import type { ActivityButton } from "../../generated/ActivityButton";
import type { HeaderProject } from "../../generated/HeaderProject";
import type { HeaderView } from "../../generated/HeaderView";
import type { MenuItem } from "../../generated/MenuItem";
import type { ShellParams } from "../../generated/ShellParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

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

/** The header's last view: the Activity popover asks again with each new one. */
export const headerStore = createStore<HeaderView | undefined>(undefined);

/** Activity, which its popover (and the processes menu) sits under. */
export const activityAnchor = createRef<HTMLButtonElement>();

const ignore = () => undefined;

/** The project's name and base branch; the name's hover is the folder, its menu "Copy path". */
function ProjectItem({ project }: { project: HeaderProject }) {
  const trigger = useRef<HTMLButtonElement>(null);
  const [menu, setMenu] = useState<MenuItem[] | null>(null);
  const open = () => {
    setMenu([
      {
        id: "copy_path",
        label: project.copy_path,
        icon: "copy",
        shortcut: null,
        danger: false,
        disabled: false,
        hint: null,
        separated: false,
        checked: false,
        header: false,
      },
    ]);
  };
  return (
    <div className="header-project">
      <Tooltip text={project.path}>
        <button
          ref={trigger}
          type="button"
          className="header-project-name"
          aria-haspopup="menu"
          aria-expanded={menu !== null}
          onClick={open}
        >
          {project.name}
        </button>
      </Tooltip>
      <MetaChip icon="git-branch" label={project.base_branch} />
      {menu && (
        <AnchoredMenu
          anchor={trigger}
          label={project.name}
          items={menu}
          width={200}
          onChoose={() => {
            navigator.clipboard.writeText(project.repo_path).catch(ignore);
          }}
          onClose={(how) => {
            setMenu(null);
            if (how === "key") trigger.current?.focus();
          }}
        />
      )}
    </div>
  );
}

export interface ActivityChipProps {
  activity: ActivityButton;
  /** Whether its popover is open (the outline turns accent). */
  open?: boolean;
  /** The live header's button is the popover's anchor; the gallery's copies are not. */
  anchor?: RefObject<HTMLButtonElement | null>;
}

/** Activity: "● 6 running · ▷ 3", the amber warning when the view has one, and its hover. A click toggles the popover. */
export function ActivityChip({ activity, open = false, anchor }: ActivityChipProps) {
  const { agents, warning } = activity;
  return (
    <Tooltip text={activity.hover.title} detail={activity.hover.lines}>
      <button
        ref={anchor}
        type="button"
        className="header-activity"
        data-open={open || undefined}
        data-limit={warning?.limit || undefined}
        aria-label={activity.label}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => {
          toggleOverlay("activity");
        }}
      >
        <span className="header-dot" data-active={agents.active || undefined} />
        <span className="header-mono">{agents.count}</span>
        <span className="header-muted">{agents.label}</span>
        {activity.processes !== null && (
          <span className="header-processes">
            <span className="header-muted">·</span>
            <span className="header-mono">▷ {activity.processes}</span>
          </span>
        )}
        {warning && (
          <span className="header-warning" style={{ color: `var(--${warning.token})` }}>
            <Icon name="gauge" size={12} />
            <span>{warning.text}</span>
          </span>
        )}
        <Icon name="chevron-down" size={12} />
      </button>
    </Tooltip>
  );
}

export interface HeaderBarProps {
  view: HeaderView;
  /** Whether the Activity popover is open (the button's outline turns accent). */
  activityOpen?: boolean;
  /** The search entry opens the board's filter (T-0221's ⌘K handler). */
  onSearch: () => void;
  /** The board's search and filter field (the `header_search` slot, T-0221), drawn in place of the entry. */
  search?: ReactNode;
}

export function HeaderBar({ view, activityOpen = false, onSearch, search }: HeaderBarProps) {
  const { inbox } = view;
  return (
    <div className="header">
      {view.project && <ProjectItem project={view.project} />}
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
      <ActivityChip activity={view.activity} open={activityOpen} anchor={activityAnchor} />
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
    </div>
  );
}

/** The header slot: the view for the shown project, refetched on every event that changes a count. */
export function ShellHeader() {
  const app = useRegistries();
  const { project } = useSelection();
  const activityOpen = useOpenOverlays().some((o) => o.id === "activity");
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
      activityOpen={activityOpen}
      search={Search && <Search />}
      onSearch={() => {
        app.shortcuts.run({ action: "search" });
      }}
    />
  );
}
