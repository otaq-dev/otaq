// The app frame (*02 · App shell → Window anatomy*): the header row (also the title bar), the project rail, the
// banner area across the window that pushes the rail, board and panel down, the board, the side panel while a ticket is open, and the overlay and
// toast hosts over everything. Each slot is filled by the screen registered for it; an empty slot shows its name so
// the layout can be checked before its screen exists.
import type { ReactNode } from "react";

import "./frame.css";
import { closeOverlay, useOpenOverlays } from "./overlays";
import { type OverlayId, type OverlayProps, type SlotId, useRegistries } from "./registry";
import { useSelection } from "./selection";
import { DRAG_REGION } from "../tauri";
import { useTheme } from "../theme/theme";

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

export interface FrameProps {
  /** Shown in the banner area while no banners screen is registered (the service's start error, Q40). */
  bannerFallback?: ReactNode;
}

export function Frame({ bannerFallback }: FrameProps) {
  // The stored theme on the root element, swapped live on every settings change (UIT-07).
  useTheme();
  const { ticket } = useSelection();
  return (
    <div className="frame">
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
      <div className="frame-main">
        <div className="frame-board" data-slot="board">
          <Slot id="board" placeholder={<Placeholder name="Board" />} />
        </div>
        {ticket !== null && (
          <div className="frame-panel" data-slot="panel">
            <Slot id="panel" placeholder={<Placeholder name="Ticket panel" />} />
          </div>
        )}
      </div>
      <OverlayHost />
      <div className="frame-layer" data-slot="toasts">
        <Slot id="toasts" />
      </div>
    </div>
  );
}
