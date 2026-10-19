// The panel's Shell tab (RUN-05; *13 · Run & Shell → Shell tab · BILL-4, Dark · Shell*): a plain terminal in the
// ticket's worktree with `$PORT` = the Run port, in T-0216's component. It is not Claude, so its strip says so in
// neutral colours. The shell opens the first time the tab shows (the service keeps it) and closes when the ticket is
// Done; one a restart stopped stays stopped until Reopen (d0045).
import { useCallback, useState } from "react";

import type { TicketProps } from "../../app/registry/panel";
import { Button, Kbd } from "../../components";
import type { ShellTabView } from "../../generated/ShellTabView";
import { useServiceView } from "../../service";
import { request } from "../../tauri";
import { Terminal } from "../../terminal/Terminal";
import { INITIAL_STATE, type TerminalState } from "../../terminal/state";
import { Icon } from "../../theme/Icon";

export function ShellStrip({ view, onReopen }: { view: ShellTabView; onReopen: () => void }) {
  return (
    <div className="shell-strip">
      <span className="shell-label">
        <span className="shell-dot" data-live={view.live || undefined} />
        {view.label}
      </span>
      {view.path !== null && <span className="run-mono run-muted">{view.path}</span>}
      {view.port !== null && <span className="run-mono run-muted">{view.port}</span>}
      <span className="run-spacer" />
      {view.session !== null && !view.live && view.stopped === null && (
        <Button compact variant="secondary" icon="rotate-cw" label={view.reopen} onClick={onReopen} />
      )}
    </div>
  );
}

export function ShellTabBody({
  view,
  state,
  onState,
  onReopen,
}: {
  view: ShellTabView;
  state: TerminalState;
  onState: (state: TerminalState) => void;
  onReopen: () => void;
}) {
  let body;
  if (view.problem !== null) body = <div className="run-idle shell-problem">{view.problem}</div>;
  else if (view.stopped !== null && !view.live)
    body = (
      <div className="shell-stopped">
        <Icon name="square" size={16} />
        <strong>{view.stopped.title}</strong>
        <span>{view.stopped.note}</span>
        <Button icon="rotate-cw" label={view.reopen} onClick={onReopen} />
      </div>
    );
  else if (view.session !== null)
    body = (
      <Terminal key={view.session} session={view.session} label={view.terminal_label} onState={onState} />
    );
  return (
    <div className="shell-tab" data-focused={state.focused || undefined}>
      <ShellStrip view={view} onReopen={onReopen} />
      <div className="shell-well">
        <div className="shell-terminal">{body}</div>
        <div className="shell-hint">
          <Icon name="info" size={13} />
          <span>{view.hint}</span>
          <span className="run-spacer" />
          {state.focused && (
            <span className="shell-leave">
              {view.leave.keys !== null && <Kbd keys={view.leave.keys} />}
              <span>{view.leave.text}</span>
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

export function ShellTab({ ticket }: TicketProps) {
  // `tick` asks again after Reopen started a new shell (the service ignores it).
  const [tick, setTick] = useState(0);
  const load = useCallback(
    () => request("run.shell", { key: ticket, tick }) as Promise<ShellTabView>,
    [ticket, tick],
  );
  // `settings_changed`: "leave terminal" names the user's key (KEY-08).
  const { view, error } = useServiceView(load, ["session_state", "settings_changed"]);
  const [state, setState] = useState<TerminalState>(INITIAL_STATE);
  if (!view || view.key !== ticket) return <p className="run-problem">{error?.message ?? ""}</p>;
  return (
    <ShellTabBody
      view={view}
      state={state}
      onState={setState}
      onReopen={() => {
        // Reopen is the user's click: one new shell.
        request("run.shell", { key: ticket, reopen: true }).then(
          () => {
            setTick((t) => t + 1);
          },
          () => undefined,
        );
      }}
    />
  );
}
