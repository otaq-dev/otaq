// The header's running processes (RUN-04; *13 · Run & Shell → All running processes · header menu*): one row per Run
// across every project with its port, Open (only with an Open URL) and Stop, and Stop all. A row opens its ticket's
// Run tab.
import { useCallback } from "react";

import type { OverlayProps } from "../../app/registry";
import { openTicket } from "../../app/selection";
import { IconButton, Popover } from "../../components";
import type { ProcessesView } from "../../generated/ProcessesView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { processesAnchor } from "../shell/Header";
import { RUN_EVENTS } from "./logic";

const ignore = () => undefined;

export function ProcessesBody({
  view,
  onOpenTicket,
}: {
  view: ProcessesView;
  onOpenTicket: (key: string) => void;
}) {
  return (
    <div className="processes">
      <div className="processes-head">
        <span className="processes-title">{view.title}</span>
        {view.rows.length > 0 && (
          <button
            type="button"
            className="processes-stop-all"
            onClick={() => {
              request("run.stop_all").catch(ignore);
            }}
          >
            {view.stop_all}
          </button>
        )}
      </div>
      {view.rows.length === 0 && <p className="run-muted">{view.empty}</p>}
      {view.rows.map((r) => (
        <div key={r.key} className="processes-row">
          <span className="shell-dot" data-live />
          <span className="processes-project">{r.project_name}</span>
          <button
            type="button"
            className="processes-ticket"
            onClick={() => {
              onOpenTicket(r.key);
            }}
          >
            <span className="run-mono">{r.key}</span>
            <span className="processes-name">{r.title}</span>
          </button>
          {r.port !== null && <span className="run-mono processes-port">{r.port}</span>}
          {r.open !== null && r.url !== null && (
            <IconButton
              icon="external-link"
              label={r.open}
              onClick={() => {
                request("run.open", { url: r.url }).catch(ignore);
              }}
            />
          )}
          <IconButton
            icon="square"
            label={view.stop}
            onClick={() => {
              request("run.stop", { key: r.key }).catch(ignore);
            }}
          />
        </div>
      ))}
    </div>
  );
}

export function ProcessesOverlay({ close }: OverlayProps<"processes">) {
  const load = useCallback(() => request("run.processes") as Promise<ProcessesView>, []);
  const { view } = useServiceView(load, RUN_EVENTS);
  if (!view) return null;
  return (
    <Popover label={view.title} anchor={processesAnchor} align="end" onClose={close}>
      <ProcessesBody
        view={view}
        onOpenTicket={(key) => {
          close();
          openTicket(key, "run");
        }}
      />
    </Popover>
  );
}
