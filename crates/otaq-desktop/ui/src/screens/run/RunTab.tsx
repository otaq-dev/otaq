// The panel's Run tab (RUN-01, RUN-03; *13 · Run & Shell → Not started … Stopped by restart, Run · backend*): the
// state card with the Run port, Run / Stop, Open only when the project sets an Open URL, and the Run's output below
// in T-0216's terminal, read-only. Nothing starts without a click.
import { useCallback, useState } from "react";

import { openOverlay } from "../../app/overlays";
import type { TicketProps } from "../../app/registry/panel";
import { Button } from "../../components";
import type { RunButtonView } from "../../generated/RunButtonView";
import type { RunCardView } from "../../generated/RunCardView";
import type { RunTabView } from "../../generated/RunTabView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Terminal } from "../../terminal/Terminal";
import { Icon } from "../../theme/Icon";
import { pressOf, RUN_EVENTS, startRun } from "./logic";

const ignore = () => undefined;

export function RunCard({
  card,
  busy,
  onPress,
}: {
  card: RunCardView;
  busy: boolean;
  onPress: (button: RunButtonView) => void;
}) {
  return (
    <div className="run-card" data-tone={card.tone}>
      <div className="run-card-head">
        <Icon name={card.icon} size={14} />
        <span className="run-card-title">{card.title}</span>
        {card.port !== null && <span className="run-mono">{card.port}</span>}
      </div>
      {card.command !== null && <span className="run-command">{card.command}</span>}
      {card.message !== null && <p className="run-message">{card.message}</p>}
      <div className="run-buttons">
        {card.buttons.map((b) => (
          <Button
            key={b.kind}
            compact
            variant={b.primary ? "primary" : "secondary"}
            icon={b.icon}
            label={b.label}
            disabled={!b.enabled}
            loading={busy && (b.kind === "run" || b.kind === "retry" || b.kind === "stop")}
            onClick={() => {
              onPress(b);
            }}
          />
        ))}
        {card.note !== null && <span className="run-note">{card.note}</span>}
      </div>
    </div>
  );
}

export function RunTabBody({ view, ticket }: { view: RunTabView; ticket: string }) {
  const [busy, setBusy] = useState(false);
  const press = (button: RunButtonView) => {
    const p = pressOf(button);
    switch (p.kind) {
      case "request":
        if (busy) return;
        setBusy(true);
        (p.method === "run.start" ? startRun(ticket) : request(p.method, { key: ticket }))
          .catch(ignore)
          .finally(() => {
            setBusy(false);
          });
        return;
      case "open":
        request("run.open", { url: p.url }).catch(ignore);
        return;
      case "edit_command":
        openOverlay("run_command", { project: view.project, ticket });
        return;
      case "copy":
        navigator.clipboard.writeText(p.text).catch(ignore);
        return;
      case "none":
        return;
    }
  };
  return (
    <div className="run-tab">
      <RunCard card={view.card} busy={busy} onPress={press} />
      <span className="run-output-label">{view.output_label}</span>
      <div className="run-output">
        {view.session === null ? (
          <div className="run-idle">{view.idle}</div>
        ) : (
          <Terminal key={view.session} session={view.session} label={`${ticket} · run`} readOnly />
        )}
      </div>
    </div>
  );
}

export function RunTab({ ticket }: TicketProps) {
  const load = useCallback(() => request("run.tab", { key: ticket }) as Promise<RunTabView>, [ticket]);
  const { view, error } = useServiceView(load, RUN_EVENTS);
  if (!view || view.key !== ticket) return <p className="run-problem">{error?.message ?? ""}</p>;
  return <RunTabBody view={view} ticket={ticket} />;
}
