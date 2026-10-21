// Agents at work during Flow setup (T-0434; ASK-06, d0108, d0086, d0088; *14 → Row · Flow step: the agent at work*
// `s14-14`/`s14-15`): the tabs over the flow agents' terminals (one per asked tool; the shown one's terminal below),
// and the Agents at work card with a row per connected tool: Working · 2 min (Watch / Watching, Stop), Not asked (Ask
// <Tool>), the plan limit (Try again at 3:45 pm), not signed in (Sign in…, then it is asked), ended without a proposal
// (Try again), Proposed; then Stop all and use a preset · Skip, use the default. Every word is Rust's
// (`onboarding.at_work` / `onboarding.flow`, `flow_at_work.rs`). The onboarding Flow step and a later project's Flow
// setup page both draw these: `FlowTerminals`, `AgentsAtWork` (its rows are `AgentRow`), `useAgentsAtWork`,
// `useAtWorkActions`.
import { type Ref, useCallback, useEffect, useRef, useState } from "react";

import { Button, Kbd, StatusPill, ToolAvatar } from "../../components";
import type { FlowAtWorkButton } from "../../generated/FlowAtWorkButton";
import type { FlowAtWorkRow } from "../../generated/FlowAtWorkRow";
import type { FlowAtWorkTab } from "../../generated/FlowAtWorkTab";
import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { OnboardingProjectParams } from "../../generated/OnboardingProjectParams";
import type { PaneParams } from "../../generated/PaneParams";
import type { PaneView } from "../../generated/PaneView";
import type { PillView } from "../../generated/PillView";
import { useServiceView, watchView } from "../../service";
import { request } from "../../tauri";
import { Terminal, type TerminalHandle } from "../../terminal/Terminal";
import { Icon } from "../../theme/Icon";
import { PANE_EVENTS } from "../pane/Pane";
import { useAgentToolActions } from "../settings/AgentTools";
import { actOnRow, askTool, AT_WORK_TICK_MS } from "./logic";

/** What changes the Flow setup: its own push, the flow agents, the tools' limits and sign-in. */
export const FLOW_SETUP_EVENTS = [
  "flow_setup_changed",
  ...PANE_EVENTS,
  "usage_changed",
  "agent_tools_changed",
] as const;

/**
 * Asks `load` on the Flow setup's events and every minute (a fresh watch each time), so "Working · 2 min" moves on. A
 * new `load` (another project) restarts it.
 */
export function useFlowSetupView<T>(load: () => Promise<T>): T | undefined {
  const [view, setView] = useState<T>();
  useEffect(() => {
    const watch = () =>
      watchView(load, FLOW_SETUP_EVENTS, (state) => {
        if (state.view !== undefined) setView(state.view);
      });
    let stop = watch();
    const timer = window.setInterval(() => {
      stop();
      stop = watch();
    }, AT_WORK_TICK_MS);
    return () => {
      window.clearInterval(timer);
      stop();
    };
  }, [load]);
  return view;
}

/** The agents at work of `project` (`onboarding.at_work`), kept up to date. */
export function useAgentsAtWork(project: string): FlowAtWorkView | undefined {
  const load = useCallback(() => {
    const params: OnboardingProjectParams = { project };
    return request("onboarding.at_work", params) as Promise<FlowAtWorkView>;
  }, [project]);
  return useFlowSetupView(load);
}

/** Sign in…'s actions answer Settings' page, which the step doesn't show. */
const ignorePage = () => undefined;

/**
 * What the rows' buttons do for `project`: each through `actOnRow`, sent with `send` (the caller's busy and error
 * handling); Sign in… opens the tool's own sign-in, and once its terminal closes the tool is checked again and asked
 * (ASK-06). Render `dialogs` once.
 */
export function useAtWorkActions(
  project: string,
  send: (sent: Promise<unknown>) => void,
  onError: (message: string) => void,
) {
  const signingIn = useRef<string | null>(null);
  const afterSignIn = useCallback(() => {
    const tool = signingIn.current;
    signingIn.current = null;
    if (tool === null) return;
    send(request("agent_tools.recheck", { tool }).then(() => askTool(project, tool, request)));
  }, [project, send]);
  const tools = useAgentToolActions(ignorePage, onError, afterSignIn);
  const act = (button: FlowAtWorkButton) => {
    const { action } = button;
    if (action.kind === "sign_in") {
      signingIn.current = action.tool;
      tools.act(action.tool, { kind: "sign_in" });
    } else send(actOnRow(project, action, request));
  };
  return { act, dialogs: tools.dialogs };
}

/**
 * A flow agent's terminal (`s14-14`, left): its name and state, the terminal, and where typing goes. Its tab may name
 * another state ("Stopped" at the limit) and foot ("Read-only · the session ended").
 */
export function FlowAgentTerminal({
  project,
  agent,
  terminal,
  pill = null,
  foot = null,
}: {
  project: string;
  agent: string;
  terminal?: Ref<TerminalHandle>;
  pill?: PillView | null;
  foot?: string | null;
}) {
  const load = useCallback(() => {
    const params: PaneParams = { project, agent, window_width: null };
    return request("pane.view", params) as Promise<PaneView>;
  }, [project, agent]);
  const view = useServiceView(load, PANE_EVENTS).view;
  const running = view?.running ?? null;
  const headerPill = pill ?? view?.header.pill ?? null;
  return (
    <section className="flow-step-agent" aria-label={view?.header.title}>
      <header className="flow-step-agent-head">
        {view && <Icon name={view.header.icon} size={14} />}
        <span className="flow-step-agent-name">{view?.header.title}</span>
        {headerPill && <StatusPill pill={headerPill} />}
      </header>
      <div className="flow-step-agent-terminal">
        {running?.session != null && (
          <Terminal
            key={running.session}
            session={running.session}
            label={`${view?.header.title ?? ""} · ${running.line.agent}`}
            readOnly={running.ended}
            ref={terminal}
          />
        )}
      </div>
      {foot !== null && (
        <div className="flow-step-agent-foot" data-read-only>
          <Icon name="lock" size={12} />
          <span>{foot}</span>
        </div>
      )}
      {running && foot === null && (
        <div className="flow-step-agent-foot">
          <Icon name={running.ended ? "circle-stop" : "keyboard"} size={12} />
          <span className="flow-step-agent-foot-text">{running.footer.text}</span>
          <span className="flow-step-spacer" />
          {running.footer.keys !== null && <Kbd keys={running.footer.keys} />}
          <span className="flow-step-agent-foot-label">{running.footer.keys_label}</span>
        </div>
      )}
    </section>
  );
}

/** One tab: the tool, a dot in its state's colour, and its state ("Working · 2 min"). */
function FlowTab({ tab, shown, onWatch }: { tab: FlowAtWorkTab; shown: boolean; onWatch: () => void }) {
  return (
    <button
      type="button"
      role="tab"
      className="flow-at-work-tab"
      aria-selected={shown}
      data-shown={shown || undefined}
      onClick={onWatch}
    >
      <ToolAvatar avatar={tab.avatar} />
      <span className="flow-at-work-tab-name">{tab.name}</span>
      <span className="flow-at-work-dot" style={{ background: `var(--st-${tab.dot})` }} />
      <span className="flow-at-work-tab-status">{tab.status}</span>
    </button>
  );
}

/**
 * The tabs over the terminals and the shown one's terminal (`s14-14`, left). `watched` is the shown tab's agent
 * (`watchedTab`); `terminal` takes ⌘L.
 */
export function FlowTerminals({
  project,
  view,
  watched,
  onWatch,
  terminal,
}: {
  project: string;
  view: FlowAtWorkView;
  watched: string | null;
  onWatch: (agent: string) => void;
  terminal?: Ref<TerminalHandle>;
}) {
  const tab = view.tabs.find((t) => t.agent === watched);
  return (
    <div className="flow-at-work-terminals">
      <div className="flow-at-work-tabs" role="tablist">
        {view.tabs.map((t) => (
          <FlowTab
            key={t.agent}
            tab={t}
            shown={t.agent === watched}
            onWatch={() => {
              onWatch(t.agent);
            }}
          />
        ))}
      </div>
      {tab && (
        <FlowAgentTerminal
          key={tab.agent}
          project={project}
          agent={tab.agent}
          terminal={terminal}
          pill={tab.pill}
          foot={tab.foot}
        />
      )}
    </div>
  );
}

export interface AgentRowProps {
  row: FlowAtWorkRow;
  /** Rust's "Watch" and "Watching". */
  watch: string;
  watching: string;
  /** Its terminal shows. */
  shown: boolean;
  busy: boolean;
  onWatch: (agent: string) => void;
  onAct: (button: FlowAtWorkButton) => void;
}

/** One tool's row (`s14-14`): its avatar, name and state, then Watch / Watching and its buttons. */
export function AgentRow({ row, watch, watching, shown, busy, onWatch, onAct }: AgentRowProps) {
  const { agent } = row;
  return (
    <div className="flow-at-work-row" data-state={row.state}>
      <ToolAvatar avatar={row.avatar} />
      <div className="flow-at-work-row-text">
        <span className="flow-at-work-row-name">{row.name}</span>
        <span className="flow-at-work-row-status" data-error={row.error || undefined}>
          {row.state === "not_asked" && <Icon name="circle-dashed" size={12} />}
          {row.status}
        </span>
      </div>
      <span className="flow-step-spacer" />
      {agent !== null &&
        (shown ? (
          <span className="flow-at-work-watching">{watching}</span>
        ) : (
          <Button
            variant="secondary"
            compact
            label={watch}
            onClick={() => {
              onWatch(agent);
            }}
          />
        ))}
      {row.buttons.map((b) => (
        <Button
          key={b.label}
          variant={b.quiet ? "ghost" : "secondary"}
          compact
          label={b.label}
          disabled={busy || !b.enabled}
          onClick={() => {
            onAct(b);
          }}
        />
      ))}
    </div>
  );
}

export interface AgentsAtWorkProps {
  view: FlowAtWorkView;
  watched: string | null;
  busy: boolean;
  onWatch: (agent: string) => void;
  onAct: (button: FlowAtWorkButton) => void;
  onStopAll: () => void;
  onSkip: () => void;
}

/** The Agents at work card (`s14-14`, right): its title and line, a row per tool, Stop all · Skip. */
export function AgentsAtWork({ view, watched, busy, onWatch, onAct, onStopAll, onSkip }: AgentsAtWorkProps) {
  return (
    <section className="onboarding-card flow-at-work" aria-live="polite">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{view.title}</h1>
        <p className="onboarding-text">{view.text}</p>
      </div>
      <div className="flow-at-work-rows">
        {view.rows.map((row) => (
          <AgentRow
            key={row.tool}
            row={row}
            watch={view.watch}
            watching={view.watching}
            shown={row.agent !== null && row.agent === watched}
            busy={busy}
            onWatch={onWatch}
            onAct={onAct}
          />
        ))}
      </div>
      <div className="flow-step-buttons">
        <Button variant="ghost" label={view.stop_all} disabled={busy} onClick={onStopAll} />
        <span className="flow-step-spacer" />
        <Button variant="ghost" label={view.skip} disabled={busy} onClick={onSkip} />
      </div>
    </section>
  );
}
