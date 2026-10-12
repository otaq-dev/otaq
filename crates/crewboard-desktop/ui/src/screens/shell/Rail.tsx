// The project rail (UI-01, ATT-10, PRJ-01; d0024, d0058; *02 → One project, Another project needs you, Ten projects ·
// long names, 1920 × 1080 · sidebar expanded*): every project in the user's order with its agents working and items
// needing you, quiet when idle; a project that newly needs you while another is shown rings for 10 s. Tiles below
// 1600 px, a 232 px sidebar above. Add project and Cleanup at the ends. Rows, order and wording are `shell.rail`'s.
import { useCallback, useEffect, useState } from "react";

import { openOverlay } from "../../app/overlays";
import { selectProject, useSelection } from "../../app/selection";
import { Kbd, Tooltip } from "../../components";
import type { CleanupBroom } from "../../generated/CleanupBroom";
import type { RailItem } from "../../generated/RailItem";
import type { RailView } from "../../generated/RailView";
import type { ShellParams } from "../../generated/ShellParams";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { railMode } from "./logic";

/** The service events that change the rail's projects or counts. */
export const RAIL_EVENTS = [
  "projects_changed",
  "ticket_changed",
  "ticket_deleted",
  "session_state",
  "inbox_changed",
] as const;

function useWindowWidth(): number {
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const onResize = () => {
      setWidth(window.innerWidth);
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  }, []);
  return width;
}

function Broom({ broom, withLabel }: { broom: CleanupBroom | undefined; withLabel: boolean }) {
  const count = broom?.count ?? 0;
  return (
    <Tooltip text={broom?.tooltip ?? "Cleanup"} detail={broom ? [broom.detail] : undefined}>
      <button
        type="button"
        className="rail-broom"
        aria-label={broom?.tooltip ?? "Cleanup"}
        onClick={() => {
          openOverlay("cleanup");
        }}
      >
        <span className="rail-broom-icon">
          <Icon name="brush-cleaning" size={withLabel ? 14 : 16} />
          {count > 0 && <span className="rail-broom-dot" />}
        </span>
        {withLabel && <span className="rail-broom-label">Cleanup</span>}
      </button>
    </Tooltip>
  );
}

function choose(item: RailItem) {
  selectProject(item.key);
}

function addProject() {
  openOverlay("add_project");
}

export interface RailProps {
  view: RailView;
  broom?: CleanupBroom | undefined;
}

/** The 64 px rail of tiles. */
export function RailTiles({ view, broom }: RailProps) {
  return (
    <nav className="rail" aria-label="Projects">
      {view.items.map((item) => (
        <Tooltip key={item.key} text={item.title} detail={item.detail} note={item.note ?? undefined}>
          <button
            type="button"
            className="rail-item"
            data-selected={item.selected || undefined}
            data-idle={item.idle || undefined}
            aria-current={item.selected || undefined}
            aria-label={item.title}
            onClick={() => {
              choose(item);
            }}
          >
            {item.selected && <span className="rail-bar" />}
            <span className="rail-tile" data-ringing={item.ringing || undefined}>
              {item.key}
              {item.needs_you > 0 && <span className="rail-needs">{item.needs_you}</span>}
              {item.agents_working > 0 && (
                <span className="rail-working">
                  <Icon name="activity" size={8} />
                  {item.agents_working}
                </span>
              )}
            </span>
            <span className="rail-name">{item.name}</span>
          </button>
        </Tooltip>
      ))}
      <Tooltip text="Add project">
        <button type="button" className="rail-add" aria-label="Add project" onClick={addProject}>
          <Icon name="plus" size={16} />
        </button>
      </Tooltip>
      <div className="rail-fill" />
      <Broom broom={broom} withLabel={false} />
    </nav>
  );
}

/** The 232 px sidebar of wide windows: names and counts. */
export function RailSidebar({ view, broom }: RailProps) {
  return (
    <nav className="sidebar" aria-label="Projects">
      <div className="sidebar-label">
        <span>PROJECTS</span>
        {view.switch_keys !== null && <Kbd keys={view.switch_keys} />}
      </div>
      {view.items.map((item) => (
        <Tooltip key={item.key} text={item.title} detail={item.detail} note={item.note ?? undefined}>
          <button
            type="button"
            className="sidebar-row"
            data-selected={item.selected || undefined}
            data-idle={item.idle || undefined}
            data-ringing={item.ringing || undefined}
            aria-current={item.selected || undefined}
            onClick={() => {
              choose(item);
            }}
          >
            <span className="sidebar-tile">
              <Icon name="folder-git-2" size={12} />
            </span>
            <span className="sidebar-name">{item.title}</span>
            {item.agents_working > 0 && (
              <span className="sidebar-working">
                <Icon name="activity" size={11} />
                {item.agents_working}
              </span>
            )}
            {item.needs_you > 0 && <span className="sidebar-needs">{item.needs_you}</span>}
            {item.idle && <span className="sidebar-idle">idle</span>}
          </button>
        </Tooltip>
      ))}
      <button type="button" className="sidebar-row sidebar-add" onClick={addProject}>
        <Icon name="plus" size={14} />
        <span>Add project</span>
      </button>
      <div className="rail-fill" />
      <Broom broom={broom} withLabel />
    </nav>
  );
}

/** The rail slot: follows the service and keeps the page's selection on a project that exists. */
export function ShellRail() {
  const { project } = useSelection();
  const [tick, setTick] = useState(0);
  const load = useCallback(async () => {
    const params: ShellParams = { project };
    return { asked: project, rail: (await request("shell.rail", params)) as RailView };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new tick is a new load: a ring that is over stops drawing
  }, [project, tick]);
  const { view } = useServiceView(load, RAIL_EVENTS);
  const loadBroom = useCallback(() => request("shell.cleanup") as Promise<CleanupBroom>, []);
  const broom = useServiceView(loadBroom, []).view;
  const width = useWindowWidth();

  // The first project when nothing is selected yet, or when the shown one was removed. Only an answer for the
  // selection now counts: one asked before a click would switch back.
  useEffect(() => {
    if (view && view.asked === project && view.rail.selected !== null && view.rail.selected !== project) {
      selectProject(view.rail.selected);
    }
  }, [view, project]);

  const ringEnds = view?.rail.ring_ends_in_ms;
  useEffect(() => {
    if (ringEnds === undefined || ringEnds === null) return;
    const timer = window.setTimeout(() => {
      setTick((n) => n + 1);
    }, ringEnds + 50);
    return () => {
      window.clearTimeout(timer);
    };
  }, [ringEnds]);

  if (view === undefined) return null;
  return railMode(width, view.rail.sidebar_from_width) === "sidebar" ? (
    <RailSidebar view={view.rail} broom={broom} />
  ) : (
    <RailTiles view={view.rail} broom={broom} />
  );
}
