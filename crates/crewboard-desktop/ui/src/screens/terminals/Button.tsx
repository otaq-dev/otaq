// The header's "Terminals & agents" button (*02 → Terminals & agents button · a Project agent waits*): ⇧⌘T in its
// tooltip, a turquoise dot while a Project agent of the shown project waits for you (its prompt or question named in
// the tooltip), and New Project agent ⇧⌘A under it. It toggles the drawer.
import { useCallback } from "react";

import { toggleOverlay, useOpenOverlays } from "../../app/overlays";
import { useSelection } from "../../app/selection";
import { IconButton, Tooltip } from "../../components";
import type { DrawerButton } from "../../generated/DrawerButton";
import { useServiceView } from "../../service";
import { request } from "../../tauri";

/** The service events that move the dot. */
export const BUTTON_EVENTS = ["project_agent_changed", "inbox_changed", "session_state"] as const;

export function TerminalsButtonView({ view, open }: { view: DrawerButton; open: boolean }) {
  return (
    <Tooltip
      text={view.label}
      keys={view.keys ?? undefined}
      detail={view.waiting === null ? undefined : [view.waiting]}
      note={view.note}
    >
      <span className="terminals-button" data-dot={view.dot || undefined}>
        <IconButton
          icon="square-terminal"
          label={view.label}
          shown={open ? "pressed" : undefined}
          onClick={() => {
            toggleOverlay("terminals_agents");
          }}
        />
      </span>
    </Tooltip>
  );
}

export function TerminalsButton() {
  const { project } = useSelection();
  const open = useOpenOverlays().some((o) => o.id === "terminals_agents");
  const key = project ?? "";
  const load = useCallback(() => request("drawer.button", { project: key }) as Promise<DrawerButton>, [key]);
  const { view } = useServiceView(load, BUTTON_EVENTS);
  if (project === null || view === undefined) return null;
  return <TerminalsButtonView view={view} open={open} />;
}
