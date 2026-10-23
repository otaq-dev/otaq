// The app frame (*24 → Row · The two views*, *Row · The Project agent*; SHB-01, d0113): the header row (also the title
// bar), the banner area across the window that pushes everything under it down, then **rail | Project agent panel
// (only while open, 420 px, pushing the board) | main area**. The main area holds the board (no Board | Files tabs:
// the project Files view stays parked, d0090); the ticket panel opens over its right side (SHL-05) and the Ask bar
// floats at its bottom centre. The overlay and toast hosts go over everything. Each slot is filled by the screen
// registered for it; an empty slot shows its name so the layout can be checked before its screen exists.
import { useCallback, useEffect, useLayoutEffect, useRef, type ReactNode } from "react";

import "./frame.css";
import { playEnterWhenReady, playLeave, usePresence } from "../components";
import type { LayoutView } from "../generated/LayoutView";
import { DRAG_REGION } from "../tauri";
import { useTheme } from "../theme/theme";
import { layoutView, loadLayout, mainWidth, type ShellLayout, shellLayout } from "./layout";
import { closeOverlay, useOpenOverlays } from "./overlays";
import { type OverlayId, type OverlayProps, type SlotId, useRegistries } from "./registry";
import { OVERLAY_MAIN_AREA } from "./registry/overlays";
import { useSelection } from "./selection";
import { useStore } from "./store";

/** The element the overlay host draws into; local menus and popovers may portal there too. */
export const OVERLAY_HOST_ID = "overlay-host";

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

/** The open overlays of one host: the window's, or the main area's (`OVERLAY_MAIN_AREA`). */
function Overlays({ mainArea }: { mainArea: boolean }) {
  const app = useRegistries();
  const open = useOpenOverlays();
  return (
    <>
      {open.map((overlay) => {
        if (Boolean(OVERLAY_MAIN_AREA[overlay.id]) !== mainArea) return null;
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
    </>
  );
}

function OverlayHost() {
  return (
    <div className="frame-layer" id={OVERLAY_HOST_ID} data-slot="overlays">
      <Overlays mainArea={false} />
    </div>
  );
}

/**
 * The main area's own overlay layer (FLW-04's preview): over the board, the ticket panel and the Ask bar, while the
 * rail, the header and the Project agent panel beside it stay usable. Empty, it takes no clicks.
 */
function MainAreaHost() {
  return (
    <div className="frame-main-layer" data-slot="main_overlays">
      <Slot id="main_area" />
      <Overlays mainArea />
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

/**
 * The Project agent panel's column (SHB-01, d0113): the panel at 420 px between the rail and the main area while it is
 * open, pushing the board; nothing while it is closed. It stays mounted closed, so its terminal keeps its place.
 */
function AgentColumn({ view, open }: { view: LayoutView; open: boolean }) {
  return (
    <div className="frame-agent" data-slot="pane" data-open={open || undefined}>
      <div className="frame-agent-body" style={{ width: view.agent_panel }} hidden={!open}>
        <Slot id="pane" />
      </div>
    </div>
  );
}

export interface FrameBodyProps {
  /** `undefined` until `layout.view` answered: the Project agent panel waits for it. */
  view: LayoutView | undefined;
  layout: ShellLayout;
  ticketOpen: boolean;
  bannerFallback?: ReactNode;
  /** Gets the main area's element, to measure it. */
  mainRef?: (element: HTMLDivElement | null) => void;
}

/**
 * Everything under the hosts: header, banners, rail, the Project agent panel while open, the main area with the
 * ticket panel and the Ask bar over it.
 */
export function FrameBody({ view, layout, ticketOpen, bannerFallback, mainRef }: FrameBodyProps) {
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
      {view && <AgentColumn view={view} open={layout.agentOpen} />}
      <div className="frame-main" ref={mainRef} data-expanded={expanded || undefined}>
        <div className="frame-board" data-slot="board">
          <Slot id="board" placeholder={<Placeholder name="Board" />} />
        </div>
        <PanelLayer open={ticketOpen} />
        {/* Over the board, the List and the ticket, at the bottom centre; it hides itself while the panel is open. */}
        <div className="frame-ask" data-slot="ask_bar">
          <Slot id="ask_bar" />
        </div>
        <MainAreaHost />
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
  useEffect(loadLayout, []);
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
        ticketOpen={ticket !== null}
        bannerFallback={bannerFallback}
        mainRef={mainRef}
      />
      <OverlayHost />
      <div className="frame-layer" data-slot="toasts">
        <Slot id="toasts" />
      </div>
    </div>
  );
}
