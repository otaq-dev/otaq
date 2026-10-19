// The app frame (*02 → Row · Window 1440 × 900 (0086 · V)*, SHL-01): the header row (also the title bar), the banner
// area across the window that pushes everything under it down, then **rail | Project agent pane | main area**. The
// main area has the Board | Files tabs at its top, then the board or the Files view; the ticket panel opens over its
// right side (SHL-05). ⌘\ folds the pane to a 28 px strip; the expanded panel (⇧⌘E) takes the pane's room too. The
// overlay and toast hosts go over everything. Each slot is filled by the screen registered for it; an empty slot
// shows its name so the layout can be checked before its screen exists.
import { useCallback, useEffect, useLayoutEffect, useRef, type PointerEvent, type ReactNode } from "react";

import "./frame.css";
import { playEnterWhenReady, playLeave, Tooltip, usePresence } from "../components";
import type { LayoutView } from "../generated/LayoutView";
import type { MainTab } from "../generated/MainTab";
import type { SizeClass } from "../generated/SizeClass";
import { DRAG_REGION } from "../tauri";
import { Icon } from "../theme/Icon";
import { useTheme } from "../theme/theme";
import {
  clampPane,
  layoutShortcuts,
  layoutView,
  loadLayout,
  mainWidth,
  paneShows,
  paneWidth,
  type ShellLayout,
  shellLayout,
  showTab,
  shownTab,
  sizeClass,
  tabCounts,
  togglePane,
  useWindowWidth,
  withDrag,
} from "./layout";
import { closeOverlay, useOpenOverlays } from "./overlays";
import { type OverlayId, type OverlayProps, type SlotId, useRegistries } from "./registry";
import { useSelection } from "./selection";
import { useStore } from "./store";

/** The element the overlay host draws into; local menus and popovers may portal there too. */
export const OVERLAY_HOST_ID = "overlay-host";

/** How far an arrow key moves the focused pane edge. */
const KEY_STEP = 16;

interface SlotProps {
  id: SlotId;
  /** Shown while no screen fills the slot. */
  placeholder?: ReactNode;
}

function Slot({ id, placeholder }: SlotProps) {
  const entry = useRegistries().slots.get(id);
  if (entry) return <entry.Component />;
  return placeholder ?? null;
}

function Placeholder({ name }: { name: string }) {
  return <div className="frame-placeholder">{name}</div>;
}

function OverlayHost() {
  const app = useRegistries();
  const open = useOpenOverlays();
  return (
    <div className="frame-layer" id={OVERLAY_HOST_ID} data-slot="overlays">
      {open.map((overlay) => {
        const entry = app.overlays.get(overlay.id);
        if (!entry) return null;
        // Each id's component takes that id's params; `OpenOverlay` pairs them, which TypeScript can't follow here.
        const Component = entry.Component as (props: OverlayProps<OverlayId>) => ReactNode;
        return (
          <Component
            key={overlay.id}
            params={overlay.params}
            close={() => {
              closeOverlay(overlay.id);
            }}
          />
        );
      })}
    </div>
  );
}

/**
 * The ticket panel's layer: a drawer, sliding in from the right edge once the ticket has loaded and back out when it
 * closes, staying mounted until the slide ends (OT-6, 0095). Switching tickets keeps it mounted, so the slide doesn't play again; resizing
 * changes only its width, so the drag stays instant.
 */
function PanelLayer({ open }: { open: boolean }) {
  const layer = useRef<HTMLDivElement>(null);
  const { mounted, leaving, done } = usePresence(open);
  useLayoutEffect(
    () => (mounted && layer.current ? playEnterWhenReady(layer.current, "panel") : undefined),
    [mounted],
  );
  useLayoutEffect(() => (leaving ? playLeave(layer.current, "panel", done) : undefined), [leaving, done]);
  if (!mounted) return null;
  return (
    <div ref={layer} className="frame-panel" data-slot="panel">
      <Slot id="panel" placeholder={<Placeholder name="Ticket panel" />} />
    </div>
  );
}

/** The pane's right edge: drag to resize (320–560 px), double-click for the class's default; ←/→ when focused. */
function PaneEdge({
  view,
  width,
  resize,
}: {
  view: LayoutView;
  width: number;
  resize: ((width: number | null) => void) | undefined;
}) {
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!resize) return;
    e.preventDefault();
    const edge = e.currentTarget;
    const left = edge.parentElement?.getBoundingClientRect().left ?? 0;
    edge.setPointerCapture(e.pointerId);
    const move = (m: globalThis.PointerEvent) => {
      resize(clampPane(m.clientX - left, view));
    };
    const up = () => {
      edge.removeEventListener("pointermove", move);
      edge.removeEventListener("pointerup", up);
    };
    edge.addEventListener("pointermove", move);
    edge.addEventListener("pointerup", up);
  };
  /* eslint-disable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex --
     a focusable separator with a value is a widget in WAI-ARIA (a splitter); jsx-a11y treats every separator as static */
  return (
    <div
      className="frame-pane-edge"
      role="separator"
      aria-orientation="vertical"
      aria-label={view.pane_resize}
      aria-valuenow={width}
      aria-valuemin={view.pane_min}
      aria-valuemax={view.pane_max}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onKeyDown={(e) => {
        const step = e.key === "ArrowRight" ? KEY_STEP : e.key === "ArrowLeft" ? -KEY_STEP : 0;
        if (step === 0 || !resize) return;
        e.preventDefault();
        resize(clampPane(width + step, view));
      }}
      onDoubleClick={() => {
        resize?.(null);
      }}
    />
  );
  /* eslint-enable jsx-a11y/no-noninteractive-element-interactions, jsx-a11y/no-noninteractive-tabindex -- the splitter ends here */
}

/** The pane's column: the pane at its width, its strip while hidden (⌘\), nothing under an expanded panel. */
function PaneColumn({
  view,
  layout,
  cls,
  ticketOpen,
  resize,
}: {
  view: LayoutView;
  layout: ShellLayout;
  cls: SizeClass;
  ticketOpen: boolean;
  resize: ((width: number | null) => void) | undefined;
}) {
  const shows = paneShows(layout, ticketOpen);
  const width = paneWidth(layout, cls, view);
  // The pane stays mounted while folded, so its terminal keeps its place; only the strip shows.
  return (
    <div className="frame-pane" data-slot="pane" data-shows={shows}>
      <div className="frame-pane-body" style={{ width }} hidden={shows !== "pane"}>
        <Slot id="pane" />
        <PaneEdge view={view} width={width} resize={resize} />
      </div>
      {shows === "strip" && (
        <Tooltip text={view.show_pane} keys={view.pane_keys ?? undefined}>
          <button
            type="button"
            className="frame-pane-strip"
            style={{ width: view.strip }}
            aria-label={view.show_pane}
            onClick={togglePane}
          >
            <span className="frame-pane-strip-bot">
              <Icon name="bot" size={15} />
            </span>
            <Slot id="pane_strip" />
            <Icon name="chevrons-right" size={13} />
          </button>
        </Tooltip>
      )}
    </div>
  );
}

/**
 * Board | Files at the main area's top, with ⇧⌘B · ⇧⌘O at the right end (*Main tabs*); not drawn while the Files view
 * is parked (`tab_row`).
 */
function MainTabs({ view, tab }: { view: LayoutView; tab: MainTab }) {
  const counts = useStore(tabCounts);
  return (
    <div className="frame-tabs" role="tablist">
      {view.tabs.map((t) => {
        const selected = tab === t.tab;
        const count = counts[t.tab];
        return (
          <button
            key={t.tab}
            type="button"
            role="tab"
            className="frame-tab"
            aria-selected={selected}
            onClick={() => {
              showTab(t.tab);
            }}
          >
            <Icon name={t.icon} size={13} />
            <span className="frame-tab-label">{t.label}</span>
            {count !== undefined && <span className="frame-tab-count">{count}</span>}
          </button>
        );
      })}
      <span className="frame-tabs-keys">{view.tab_keys}</span>
    </div>
  );
}

export interface FrameBodyProps {
  /** `undefined` until `layout.view` answered: the pane and the tabs wait for it. */
  view: LayoutView | undefined;
  layout: ShellLayout;
  /** The window's width, for its size class. */
  windowWidth: number;
  ticketOpen: boolean;
  bannerFallback?: ReactNode;
  /** Keeps a dragged pane width for this window's class; the gallery's frames leave it out. */
  resizePane?: (cls: SizeClass, width: number | null) => void;
  /** Gets the main area's element, to measure it. */
  mainRef?: (element: HTMLDivElement | null) => void;
}

/** Everything under the hosts: header, banners, rail, pane, main area and the panel over it. */
export function FrameBody({
  view,
  layout,
  windowWidth,
  ticketOpen,
  bannerFallback,
  resizePane,
  mainRef,
}: FrameBodyProps) {
  const cls = view ? sizeClass(windowWidth, view.classes) : null;
  const tab = view ? shownTab(layout, view) : layout.tab;
  const expanded = layout.panelExpanded && ticketOpen;
  return (
    <>
      {/* The header row is the title bar: pressing its empty parts moves the window (T-0214). */}
      <div className="frame-header" data-slot="header" {...DRAG_REGION}>
        <Slot id="title_bar_start" />
        <div className="frame-header-content">
          <Slot id="header" placeholder={<Placeholder name="Header" />} />
        </div>
        <Slot id="title_bar_end" />
      </div>
      <div className="frame-rail" data-slot="rail">
        <Slot id="rail" placeholder={<Placeholder name="Rail" />} />
      </div>
      <div className="frame-banners" data-slot="banners">
        <Slot id="banners" placeholder={bannerFallback} />
      </div>
      {view && cls && (
        <PaneColumn
          view={view}
          layout={layout}
          cls={cls}
          ticketOpen={ticketOpen}
          resize={
            resizePane &&
            ((width) => {
              resizePane(cls, width);
            })
          }
        />
      )}
      <div className="frame-main" ref={mainRef} data-expanded={expanded || undefined}>
        {view?.tab_row && <MainTabs view={view} tab={tab} />}
        {tab === "files" && view ? (
          <div className="frame-files" data-slot="files">
            <Slot id="files" placeholder={<p className="frame-files-empty">{view.files_empty}</p>} />
          </div>
        ) : (
          <div className="frame-board" data-slot="board">
            <Slot id="board" placeholder={<Placeholder name="Board" />} />
          </div>
        )}
        <PanelLayer open={ticketOpen} />
      </div>
    </>
  );
}

export interface FrameProps {
  /** Shown in the banner area while no banners screen is registered (the service's start error, Q40). */
  bannerFallback?: ReactNode;
}

export function Frame({ bannerFallback }: FrameProps) {
  // The stored theme on the root element, swapped live on every settings change (UIT-07).
  useTheme();
  const { ticket } = useSelection();
  const layout = useStore(shellLayout);
  const view = useStore(layoutView);
  const windowWidth = useWindowWidth();
  const app = useRegistries();
  useEffect(loadLayout, []);
  useEffect(() => layoutShortcuts(app), [app]);
  // The panel's widest is the main area minus what the board keeps, so the frame measures it.
  const observer = useRef<ResizeObserver | null>(null);
  const mainRef = useCallback((element: HTMLDivElement | null) => {
    observer.current?.disconnect();
    observer.current = null;
    if (element === null) return;
    observer.current = new ResizeObserver(() => {
      mainWidth.set(element.clientWidth);
    });
    observer.current.observe(element);
  }, []);
  return (
    <div className="frame">
      <FrameBody
        view={view}
        layout={layout}
        windowWidth={windowWidth}
        ticketOpen={ticket !== null}
        bannerFallback={bannerFallback}
        resizePane={(cls, width) => {
          shellLayout.update((l) => ({ ...l, paneWidths: withDrag(l.paneWidths, cls, width) }));
        }}
        mainRef={mainRef}
      />
      <OverlayHost />
      <div className="frame-layer" data-slot="toasts">
        <Slot id="toasts" />
      </div>
    </div>
  );
}
