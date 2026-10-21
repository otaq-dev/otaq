// The Flow step (ONB-04…ONB-07, d0108; *14 → Row · Flow step: sources* `s14-12`/`s14-13`, *the proposal*
// `s14-16`/`s14-17`, *the presets* `s14-18`/`s14-19`): after Add your first project, the go-ahead (asked once, with
// the line naming what the agent will look at), the presets, the onboarding agent at work beside its terminal, and its
// proposal in the step with Use the default · Ask for changes · Use this flow. What it says is Rust's
// (`onboarding.flow`, `onboarding_page.rs`; `flow_preview.view` with `onboarding`, `flow_preview.rs`); the state
// machine is `logic.ts`'s. The go-ahead, the presets and the proposal are their own components, for the Add project
// dialog's go-ahead (T-0431); the agent at work is `FlowAtWork.tsx`'s (T-0430).
import { type Ref, useCallback, useEffect, useRef, useState } from "react";

import { Button, IconButton, Kbd, StatusPill } from "../../components";
import type { FlowAtWorkButton } from "../../generated/FlowAtWorkButton";
import type { FlowPreviewChoices } from "../../generated/FlowPreviewChoices";
import type { FlowPreviewLoad } from "../../generated/FlowPreviewLoad";
import type { FlowPreviewParams } from "../../generated/FlowPreviewParams";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { PaneParams } from "../../generated/PaneParams";
import type { PaneView } from "../../generated/PaneView";
import type { PillView } from "../../generated/PillView";
import type { PresetCard } from "../../generated/PresetCard";
import { keymap } from "../../app/keyboard";
import { selection } from "../../app/selection";
import { asRequestError, useServiceView, watchView } from "../../service";
import { request } from "../../tauri";
import { Terminal, type TerminalHandle } from "../../terminal/Terminal";
import { Icon } from "../../theme/Icon";
import { PreviewBody, PreviewFooter, REFETCH_ON } from "../flow_preview/FlowPreview";
import { applyPreview, cancelPreview, NO_CHOICES, previewHeld } from "../flow_preview/logic";
import { PANE_EVENTS } from "../pane/Pane";
import { useAgentToolActions } from "../settings/AgentTools";
import { FlowAtWork, type FlowAtWorkProps, useFlowAtWork } from "./FlowAtWork";
import {
  type FlowStepState,
  flowAgent,
  flowStarted,
  flowStepOpens,
  focusesAgent,
  proposalGone,
  proposalShown,
  proposePreset,
  retryFlowAgent,
  startFlowAgent,
  stopFlowAgent,
} from "./logic";

/** The go-ahead's and the presets' buttons: Skip, use the default · Use a preset instead · Look and propose a flow. */
function GoAheadButtons({
  step,
  busy,
  onSkip,
  onPresets,
  onLook,
}: {
  step: OnboardingFlowStep;
  busy: boolean;
  onSkip: () => void;
  onPresets: () => void;
  onLook: () => void;
}) {
  return (
    <div className="flow-step-buttons">
      <Button variant="ghost" label={step.skip} disabled={busy} onClick={onSkip} />
      <span className="flow-step-spacer" />
      <Button variant="secondary" label={step.use_preset} disabled={busy} onClick={onPresets} />
      <Button
        label={step.propose}
        icon="sparkles"
        disabled={busy || !step.can_propose}
        loading={busy}
        onClick={onLook}
      />
    </div>
  );
}

export interface GoAheadProps {
  step: OnboardingFlowStep;
  busy: boolean;
  onSkip: () => void;
  onPresets: () => void;
  onLook: () => void;
}

/** The go-ahead (`s14-12`): the title, the intro, what the agent looks at, the provider and tokens line. */
export function FlowGoAhead(props: GoAheadProps) {
  const { step } = props;
  return (
    <section className="onboarding-card flow-step-card">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{step.title}</h1>
        <p className="onboarding-text">{step.intro}</p>
      </div>
      <div className="flow-step-line">
        <Icon name="sparkles" size={15} />
        <p>{step.line}</p>
      </div>
      <p className="flow-step-provider">
        <Icon name="info" size={12} />
        <span>{step.provider}</span>
      </p>
      <GoAheadButtons {...props} />
    </section>
  );
}

/** One preset (*the four presets*): its name and how it delivers, its line and the small diagram of its columns. */
export function Preset({
  card,
  disabled,
  onPick,
}: {
  card: PresetCard;
  disabled: boolean;
  onPick: () => void;
}) {
  return (
    <button type="button" className="flow-step-preset" disabled={disabled} onClick={onPick}>
      <span className="flow-step-preset-head">
        <span className="flow-step-preset-name">{card.name}</span>
        <span className="flow-step-preset-delivers">{card.delivers}</span>
      </span>
      <span className="flow-step-preset-line">{card.line}</span>
      <span className="flow-step-preset-columns">
        {card.columns.map((column, i) => (
          <span key={`${column.label}-${String(i)}`} className="flow-step-preset-column">
            <Icon name={column.icon} size={12} />
            <span>{column.label}</span>
          </span>
        ))}
      </span>
    </button>
  );
}

/**
 * The presets (`s14-12 → Nothing to look at yet`): Use a preset instead, or the step itself when there is nothing to
 * look at (then with the reason, and Look and propose a flow off). A pick proposes it.
 */
export function FlowPresets(props: GoAheadProps & { onPick: (file: string) => void }) {
  const { step, busy, onPick } = props;
  const list = useRef<HTMLDivElement>(null);
  return (
    <section className="onboarding-card flow-step-card">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{step.title}</h1>
        <p className="onboarding-text">{step.presets.text}</p>
      </div>
      {step.presets.reason !== null && <p className="flow-step-reason">{step.presets.reason}</p>}
      <h2 className="flow-step-heading">{step.presets.heading}</h2>
      <div className="flow-step-presets" ref={list}>
        {step.presets.cards.map((card) => (
          <Preset
            key={card.file}
            card={card}
            disabled={busy}
            onPick={() => {
              onPick(card.file);
            }}
          />
        ))}
      </div>
      <GoAheadButtons
        {...props}
        onPresets={() => {
          // Already on the presets: the keyboard moves to the first one.
          list.current?.querySelector<HTMLElement>(".flow-step-preset")?.focus();
        }}
      />
    </section>
  );
}

/**
 * The onboarding agent's terminal (`s14-14`, `s14-16`, left): its name and state, the terminal, and where typing goes.
 * The agent at work's card may name another state ("Stopped" at the limit) and foot ("Read-only · the session ended").
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

/**
 * The onboarding agent at work (`s14-14`): its terminal on the left, the card on the right. The card's words follow
 * the agent (working, stopped at the limit, ended); Stop and use a preset ends it, Try again continues it.
 */
function FlowWorking({
  project,
  agent,
  terminal,
  busy,
  onStop,
  onSkip,
  onPresets,
  onRetry,
}: {
  project: string;
  agent: string;
  terminal: Ref<TerminalHandle>;
} & Omit<FlowAtWorkProps, "view">) {
  const view = useFlowAtWork(project, agent);
  return (
    <div className="flow-step-split">
      <FlowAgentTerminal
        project={project}
        agent={agent}
        terminal={terminal}
        pill={view?.pill ?? null}
        foot={view?.foot ?? null}
      />
      {view && (
        <FlowAtWork
          view={view}
          busy={busy}
          onStop={onStop}
          onSkip={onSkip}
          onPresets={onPresets}
          onRetry={onRetry}
        />
      )}
    </div>
  );
}

/**
 * The proposal in the step (`s14-16`/`s14-17`, right): FLW-04's preview with the onboarding buttons. Ask for changes
 * hands the keyboard to the agent's terminal; the preview updates when it proposes again.
 */
export function FlowProposal({
  view,
  choices,
  busy,
  error,
  onChoices,
  onError,
  onUseDefault,
  onAsk,
  onUse,
}: {
  view: FlowPreviewView;
  choices: FlowPreviewChoices;
  busy: boolean;
  error: string | null;
  onChoices: (choices: FlowPreviewChoices) => void;
  onError: (message: string) => void;
  onUseDefault: () => void;
  onAsk: () => void;
  onUse: () => void;
}) {
  const ignore = () => undefined;
  return (
    <section className="flow-preview flow-step-preview" aria-label={view.title}>
      <header className="flow-preview-head">
        <div className="flow-preview-titles">
          <h2>{view.title}</h2>
          <p>{view.subtitle}</p>
        </div>
        <IconButton icon="x" label={view.close_label} disabled={busy} onClick={onUseDefault} />
      </header>
      <div className="flow-preview-body">
        <PreviewBody view={view} choices={choices} onChoices={onChoices} onError={onError} />
        {error !== null && (
          <p className="flow-preview-error" role="alert">
            {error}
          </p>
        )}
      </div>
      <PreviewFooter
        view={view}
        mode={busy ? "busy" : "buttons"}
        text=""
        onText={ignore}
        onCancel={onUseDefault}
        onAsk={onAsk}
        onSend={ignore}
        onBack={ignore}
        onApply={onUse}
      />
    </section>
  );
}

/** Sign in…'s actions answer Settings' page, which the step doesn't show. */
const ignorePage = () => undefined;

/**
 * The Flow step for the project just added. `onDone` ends onboarding on the project's board (with the onboarding
 * agent, if one started, open in its pane): after Skip, Use this flow or Use the default.
 */
export function FlowStep({ project, onDone }: { project: string; onDone: (agent: string | null) => void }) {
  const [step, setStep] = useState<OnboardingFlowStep>();
  const [state, setState] = useState<FlowStepState>({ at: "go_ahead" });
  const [choices, setChoices] = useState<FlowPreviewChoices>(NO_CHOICES);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const terminal = useRef<TerminalHandle>(null);
  const box = useRef<HTMLDivElement>(null);
  // Use this flow or Use the default is on its way: the proposal closing is the step ending, not a way back.
  const ending = useRef(false);

  // What there is to look at runs `gh` and `glab`: asked once, when the step opens.
  useEffect(() => {
    request("onboarding.flow", { project }).then(
      (answer) => {
        const loaded = answer as OnboardingFlowStep;
        setStep(loaded);
        setState(flowStepOpens(loaded));
      },
      (e: unknown) => {
        setError(asRequestError(e).message);
      },
    );
  }, [project]);

  // The usual preview doesn't open over the step: it shows the project's proposals itself.
  useEffect(() => {
    previewHeld.set(project);
    return () => {
      previewHeld.set(null);
    };
  }, [project]);

  // The project's open proposal: the agent's arrives while it works; a later one replaces it in place. An answer to an
  // older ask is dropped (the watch restarts when `first` or the picks change).
  const first = state.at === "proposal" ? state.first : null;
  const [loaded, setLoaded] = useState<FlowPreviewLoad>();
  useEffect(() => {
    const params: FlowPreviewParams = { project, choices, onboarding: { first } };
    const load = () => request("flow_preview.view", params) as Promise<FlowPreviewLoad>;
    return watchView(load, REFETCH_ON, ({ view: answer }) => {
      if (answer === undefined) return;
      setLoaded(answer);
      const shown = answer.view;
      setState((s) => {
        if (shown === null) return ending.current ? s : proposalGone(s);
        return s.at === "working" ? proposalShown(s, shown.proposal) : s;
      });
    });
  }, [project, choices, first]);
  const view = state.at === "proposal" ? (loaded?.view ?? null) : null;

  // The go-ahead's and the presets' first button takes the keyboard (a proposal leaves it in the terminal).
  const at = step ? state.at : null;
  useEffect(() => {
    if (at === "go_ahead" || at === "presets")
      box.current?.querySelector<HTMLElement>(".button[data-variant='primary']:not(:disabled)")?.focus();
  }, [at]);

  // ⌘L focuses the agent's terminal: the onboarding overlay holds the app's shortcuts back, so the step listens.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!focusesAgent(keymap.get(), e, selection.get().focusedTerminal !== null)) return;
      e.preventDefault();
      terminal.current?.focus();
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
    };
  }, []);

  const agent = flowAgent(state);
  const done = () => {
    onDone(agent);
  };
  const act = (sent: Promise<unknown>, after: () => void) => {
    setBusy(true);
    setError(null);
    sent.then(
      () => {
        setBusy(false);
        after();
      },
      (e: unknown) => {
        ending.current = false;
        setBusy(false);
        setError(asRequestError(e).message);
      },
    );
  };
  const end = (sent: () => Promise<unknown>) => {
    ending.current = true;
    act(sent(), done);
  };
  // Sign in…'s terminal closed: the tool is checked again, then the agent starts (or the card stays).
  const afterSignIn = () => {
    if (!step || state.at !== "cannot_start") return;
    const action = state.card.retry?.action;
    const tool = action?.kind === "sign_in" ? action.tool : null;
    act(
      request("agent_tools.recheck", { tool })
        .then(() => startFlowAgent(step, request))
        .then((started) => {
          setState(flowStarted(started));
        }),
      () => undefined,
    );
  };
  const agentTools = useAgentToolActions(ignorePage, setError, afterSignIn);

  if (!step) return error !== null ? <p className="onboarding-failure">{error}</p> : null;
  const start = () => {
    act(
      startFlowAgent(step, request).then((started) => {
        setState(flowStarted(started));
      }),
      () => undefined,
    );
  };
  const goAhead: GoAheadProps = {
    step,
    busy,
    onSkip: done,
    onPresets: () => {
      setState({ at: "presets", agent });
    },
    onLook: start,
  };
  const failed = {
    busy,
    onStop: () => {
      if (agent !== null) act(stopFlowAgent(project, agent, request).then(setState), () => undefined);
    },
    onSkip: done,
    onPresets: goAhead.onPresets,
    onRetry: (button: FlowAtWorkButton) => {
      const { action } = button;
      if (action.kind === "sign_in") agentTools.act(action.tool, { kind: "sign_in" });
      else act(retryFlowAgent(action, request), () => undefined);
    },
  };
  let body;
  switch (state.at) {
    case "go_ahead":
      body = <FlowGoAhead {...goAhead} />;
      break;
    case "presets":
      body = (
        <FlowPresets
          {...goAhead}
          onPick={(file) => {
            act(
              proposePreset(step, file, request).then((proposal) => {
                setChoices(NO_CHOICES);
                setState({ at: "proposal", agent, first: proposal });
              }),
              () => undefined,
            );
          }}
        />
      );
      break;
    case "working":
      body = <FlowWorking project={project} agent={state.agent} terminal={terminal} {...failed} />;
      break;
    case "cannot_start":
      body = <FlowAtWork view={state.card} {...failed} />;
      break;
    case "proposal":
      body = (
        <div className="flow-step-split">
          {state.agent !== null && (
            <FlowAgentTerminal project={project} agent={state.agent} terminal={terminal} />
          )}
          {view !== null && (
            <FlowProposal
              view={view}
              choices={choices}
              busy={busy}
              error={error}
              onChoices={setChoices}
              onError={setError}
              onUseDefault={() => {
                end(() => cancelPreview(view));
              }}
              onAsk={() => {
                terminal.current?.focus();
              }}
              onUse={() => {
                end(() => applyPreview(view));
              }}
            />
          )}
        </div>
      );
      break;
  }
  return (
    <div className="flow-step" ref={box}>
      {body}
      {agentTools.dialogs}
      {error !== null && state.at !== "proposal" && <p className="onboarding-failure">{error}</p>}
    </div>
  );
}
