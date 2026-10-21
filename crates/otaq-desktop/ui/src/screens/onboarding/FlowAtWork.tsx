// The onboarding agent at work (T-0430; ONB-05, d0108, d0086, d0088; *14 → Row · Flow step: the agent at work*
// `s14-14`/`s14-15`): the card beside the agent's terminal while it reads how the user works (Working · 2 min, Stop
// and use a preset), and its failures: stopped at the plan limit (Try again at 3:45 pm), its tool not signed in (Sign
// in…), ended without a proposal (Try again). Every word is Rust's (`onboarding.at_work`, `flow_at_work.rs`); the
// card asks again on the agent's and the usage's events, and once a minute for the elapsed time.
import { useEffect, useState } from "react";

import { Button } from "../../components";
import type { FlowAtWorkButton } from "../../generated/FlowAtWorkButton";
import type { FlowAtWorkParams } from "../../generated/FlowAtWorkParams";
import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import { watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { PANE_EVENTS } from "../pane/Pane";
import { AT_WORK_TICK_MS } from "./logic";

/** What changes the card: the agent (its state, its limit stop), its tool's limit, and the tools' sign-in. */
const AT_WORK_EVENTS = [...PANE_EVENTS, "usage_changed", "agent_tools_changed"] as const;

/** The card of the onboarding agent `agent`, asked again on its events and every minute (a fresh watch each time). */
export function useFlowAtWork(project: string, agent: string): FlowAtWorkView | undefined {
  const [view, setView] = useState<FlowAtWorkView>();
  useEffect(() => {
    const params: FlowAtWorkParams = { project, agent };
    const watch = () =>
      watchView(
        () => request("onboarding.at_work", params) as Promise<FlowAtWorkView>,
        AT_WORK_EVENTS,
        (state) => {
          if (state.view) setView(state.view);
        },
      );
    let stop = watch();
    const timer = window.setInterval(() => {
      stop();
      stop = watch();
    }, AT_WORK_TICK_MS);
    return () => {
      window.clearInterval(timer);
      stop();
    };
  }, [project, agent]);
  return view;
}

export interface FlowAtWorkProps {
  view: FlowAtWorkView;
  busy: boolean;
  onStop: () => void;
  onSkip: () => void;
  onPresets: () => void;
  onRetry: (button: FlowAtWorkButton) => void;
}

/** The card (`s14-14`): its title and line, then the elapsed time or the red line, then its buttons. */
export function FlowAtWork({ view, busy, onStop, onSkip, onPresets, onRetry }: FlowAtWorkProps) {
  const { retry } = view;
  return (
    <section className="onboarding-card flow-at-work" data-kind={view.kind} aria-live="polite">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{view.title}</h1>
        <p className="onboarding-text">{view.text}</p>
      </div>
      {view.elapsed !== null && (
        <p className="flow-at-work-elapsed">
          <Icon name="loader-circle" size={14} />
          <span>{view.elapsed}</span>
        </p>
      )}
      {view.problem !== null && (
        <p className="flow-at-work-problem">
          <Icon name="circle-x" size={14} />
          <span>{view.problem}</span>
        </p>
      )}
      <div className="flow-step-buttons">
        {view.stop !== null && <Button variant="ghost" label={view.stop} disabled={busy} onClick={onStop} />}
        {view.skip !== null && <Button variant="ghost" label={view.skip} disabled={busy} onClick={onSkip} />}
        <span className="flow-step-spacer" />
        {view.use_preset !== null && (
          <Button variant="secondary" label={view.use_preset} disabled={busy} onClick={onPresets} />
        )}
        {retry !== null && (
          <Button
            label={retry.label}
            icon={retry.icon}
            disabled={busy || !retry.enabled}
            loading={busy}
            onClick={() => {
              onRetry(retry);
            }}
          />
        )}
      </div>
    </section>
  );
}
