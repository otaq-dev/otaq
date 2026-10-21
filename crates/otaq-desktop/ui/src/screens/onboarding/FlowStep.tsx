// The Flow step (ASK-04…ASK-06, d0108; *14 → Row · Flow step: sources* `s14-12`/`s14-13`, *the agent at work*
// `s14-14`/`s14-15`, *the proposal* `s14-16`/`s14-17`): after Add your first project, the go-ahead (Ask an agent: a row
// per connected tool, nothing asked on its own), the presets, the agents at work once a tool was asked (the tabs over
// their terminals and the card, `FlowAtWork.tsx`), and a proposal in the step with Use the default · Use this flow.
// The project stays in Flow setup until its flow is settled, so the step stays too, and quitting brings it back. What
// it says is Rust's (`onboarding.flow`, `onboarding_page.rs`, `flow_at_work.rs`; two or more proposals side by side
// in `FlowCompare` from `flow_preview.compare`, a tool not ready beside them as its `AgentRow`; `flow_preview.view` with
// `onboarding`); the state machine is `logic.ts`'s. The go-ahead, its rows, the presets and the proposal are their own
// components, for the Add project dialog and the Flow setup page.
import { useCallback, useEffect, useRef, useState } from "react";

import { Button, IconButton, ToolAvatar } from "../../components";
import type { FlowCompareParams } from "../../generated/FlowCompareParams";
import type { FlowCompareView } from "../../generated/FlowCompareView";
import type { FlowPreviewChoices } from "../../generated/FlowPreviewChoices";
import type { FlowPreviewLoad } from "../../generated/FlowPreviewLoad";
import type { FlowPreviewParams } from "../../generated/FlowPreviewParams";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { OnboardingProjectParams } from "../../generated/OnboardingProjectParams";
import type { PresetCard } from "../../generated/PresetCard";
import { keymap } from "../../app/keyboard";
import { selection } from "../../app/selection";
import { asRequestError, watchView } from "../../service";
import { request } from "../../tauri";
import type { TerminalHandle } from "../../terminal/Terminal";
import { Icon } from "../../theme/Icon";
import { PreviewBody, PreviewFooter, REFETCH_ON } from "../flow_preview/FlowPreview";
import { applyPreview, NO_CHOICES, previewHeld } from "../flow_preview/logic";
import { FlowCompare } from "../flow_preview/FlowCompare";
import { AgentRow, AgentsAtWork, FlowTerminals, useAtWorkActions, useFlowSetupView } from "./FlowAtWork";
import {
  askTool,
  chooseDefault,
  firstProposals,
  type FlowStepState,
  flowStepChanged,
  focusesAgent,
  proposalGone,
  proposalShown,
  proposePreset,
  proposingAgent,
  stopAll,
  watchedTab,
} from "./logic";

export interface AskRowsProps {
  step: OnboardingFlowStep;
  busy: boolean;
  onAsk: (tool: string) => void;
}

/** Ask an agent (`s14-12`): one row per connected tool, its Ask; the only tool's Ask is the primary button. */
export function AskRows({ step, busy, onAsk }: AskRowsProps) {
  return (
    <div className="flow-step-ask">
      <h2 className="flow-step-heading">{step.heading}</h2>
      {step.rows.map((row) => (
        <div key={row.tool} className="flow-step-ask-row">
          <ToolAvatar avatar={row.avatar} />
          <span className="flow-step-ask-name">{row.name}</span>
          <span className="flow-step-ask-detail">{row.detail}</span>
          <span className="flow-step-spacer" />
          <Button
            variant={row.primary ? "primary" : "secondary"}
            label={row.ask}
            disabled={busy}
            onClick={() => {
              onAsk(row.tool);
            }}
          />
        </div>
      ))}
    </div>
  );
}

export interface GoAheadProps extends AskRowsProps {
  onSkip: () => void;
  onPresets: () => void;
}

/** The go-ahead (`s14-12`): the title, the intro, the line, Ask an agent, the provider and tokens line, its buttons. */
export function FlowGoAhead(props: GoAheadProps) {
  const { step, busy, onSkip, onPresets } = props;
  return (
    <section className="onboarding-card flow-step-card">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{step.title}</h1>
        <p className="onboarding-text">{step.intro}</p>
      </div>
      <div className="flow-step-line">
        <p>{step.line}</p>
      </div>
      <AskRows {...props} />
      <p className="flow-step-provider">
        <Icon name="info" size={12} />
        <span>{step.provider}</span>
      </p>
      <div className="flow-step-buttons">
        <Button variant="ghost" label={step.skip} disabled={busy} onClick={onSkip} />
        <span className="flow-step-spacer" />
        <Button variant="secondary" label={step.use_preset} disabled={busy} onClick={onPresets} />
      </div>
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

export interface PresetsProps {
  step: OnboardingFlowStep;
  busy: boolean;
  onPick: (file: string) => void;
  onSkip: () => void;
  /** Back to the agents (a tool is connected). */
  onAgents: () => void;
  /** With no tool connected: Back to Tools. */
  onBackToTools: () => void;
}

/**
 * The presets (`s14-12 → No agent tool connected`): Use a preset instead, or the step itself with no tool connected
 * (then with the line and Back to Tools). A pick proposes it.
 */
export function FlowPresets({ step, busy, onPick, onSkip, onAgents, onBackToTools }: PresetsProps) {
  return (
    <section className="onboarding-card flow-step-card">
      <div className="flow-step-titles">
        <h1 className="onboarding-title">{step.title}</h1>
        <p className="onboarding-text">{step.presets.text}</p>
      </div>
      {step.presets.note !== null && (
        <p className="flow-step-note">
          <span>{step.presets.note}</span>
          {step.back_to_tools !== null && (
            <button type="button" className="flow-step-link" disabled={busy} onClick={onBackToTools}>
              {step.back_to_tools}
            </button>
          )}
        </p>
      )}
      <h2 className="flow-step-heading">{step.presets.heading}</h2>
      <div className="flow-step-presets">
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
      <div className="flow-step-buttons">
        <Button variant="ghost" label={step.skip} disabled={busy} onClick={onSkip} />
        <span className="flow-step-spacer" />
        {step.ask_an_agent !== null && (
          <Button variant="secondary" label={step.ask_an_agent} disabled={busy} onClick={onAgents} />
        )}
      </div>
    </section>
  );
}

/**
 * The proposal in the step (`s14-16`/`s14-17`, right): FLW-04's preview with the onboarding buttons. Ask for changes
 * hands the keyboard to the shown agent's terminal; the preview updates when it proposes again.
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

/**
 * The Flow step for the project in Flow setup. `onDone` ends onboarding on the project's board (with the chosen
 * proposal's flow agent open in its pane) once the flow is settled: Skip, Use this flow or Use the default.
 * `onBackToTools` goes to the Tools step (no tool connected).
 */
export function FlowStep({
  project,
  onDone,
  onBackToTools,
}: {
  project: string;
  onDone: (agent: string | null) => void;
  onBackToTools: () => void;
}) {
  const load = useCallback(() => {
    const params: OnboardingProjectParams = { project };
    return request("onboarding.flow", params) as Promise<OnboardingFlowStep>;
  }, [project]);
  const step = useFlowSetupView(load);
  // The user's place; the go-ahead stands for "where it opens" (`flowStepChanged`).
  const [place, setState] = useState<FlowStepState>({ at: "go_ahead" });
  const state = step ? flowStepChanged(place, step) : undefined;
  const [choices, setChoices] = useState<FlowPreviewChoices>(NO_CHOICES);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [picked, setPicked] = useState<string | null>(null);
  const terminal = useRef<TerminalHandle>(null);
  const box = useRef<HTMLDivElement>(null);
  // Use this flow or Use the default is on its way: the proposal closing is the step ending, not a way back.
  const ending = useRef(false);

  // The usual preview doesn't open over the step: it shows the project's proposals itself.
  useEffect(() => {
    previewHeld.set(project);
    return () => {
      previewHeld.set(null);
    };
  }, [project]);

  // The project's open proposal: an agent's arrives while it works; a later one replaces it in place. An answer to an
  // older ask is dropped (the watch restarts when `first` or the picks change).
  const first = place.at === "proposal" ? place.first : null;
  const [loaded, setLoaded] = useState<FlowPreviewLoad>();
  useEffect(() => {
    const params: FlowPreviewParams = { project, choices, onboarding: { first } };
    const load = () => request("flow_preview.view", params) as Promise<FlowPreviewLoad>;
    return watchView(load, REFETCH_ON, ({ view: answer }) => {
      if (answer === undefined) return;
      setLoaded(answer);
      const shown = answer.view;
      setState((s) => {
        if (shown !== null) return proposalShown(s, shown.proposal);
        return ending.current ? s : proposalGone(s);
      });
    });
  }, [project, choices, first]);
  const view = state?.at === "proposal" ? (loaded?.view ?? null) : null;

  // Two or more proposals (ASK-07): compared side by side, each with its own picks; `firsts` keeps the proposal shown
  // first per flow agent, so a later one reads "updated after your answer".
  const comparing = place.at === "proposal";
  const [picks, setPicks] = useState<Record<number, FlowPreviewChoices>>({});
  const [firsts, setFirsts] = useState<Record<string, number>>({});
  const [compare, setCompare] = useState<FlowCompareView>();
  useEffect(() => {
    if (!comparing) return;
    const params: FlowCompareParams = { project, choices: picks, first: firsts };
    const load = () => request("flow_preview.compare", params) as Promise<FlowCompareView>;
    return watchView(load, REFETCH_ON, ({ view: answer }) => {
      if (answer === undefined) return;
      setCompare(answer);
      const seen = firstProposals(firsts, answer);
      if (seen !== firsts) setFirsts(seen);
    });
  }, [project, comparing, picks, firsts]);
  const compared = comparing && compare?.compare === true ? compare : null;

  // The go-ahead's and the presets' first button takes the keyboard (the agents at work leave it in the terminal).
  const at = step ? (state?.at ?? null) : null;
  useEffect(() => {
    if (at === "go_ahead" || at === "presets")
      box.current?.querySelector<HTMLElement>(".button[data-variant='primary']:not(:disabled)")?.focus();
  }, [at]);

  // ⌘L focuses the shown agent's terminal: the onboarding overlay holds the app's shortcuts back, so the step listens.
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

  const send = useCallback((sent: Promise<unknown>, after: () => void = () => undefined) => {
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
  }, []);
  const rows = useAtWorkActions(project, send, setError);

  if (!step || !state) return error !== null ? <p className="onboarding-failure">{error}</p> : null;
  const atWork = step.at_work;
  const watched = watchedTab(atWork, picked);
  const end = (sent: () => Promise<unknown>, agent: string | null) => {
    ending.current = true;
    send(sent(), () => {
      onDone(agent);
    });
  };
  const skip = () => {
    end(() => chooseDefault(project, request), null);
  };
  const ask = (tool: string) => {
    send(askTool(project, tool, request));
  };
  const terminals = atWork.tabs.length > 0 && (
    <FlowTerminals
      project={project}
      view={atWork}
      watched={watched}
      onWatch={setPicked}
      terminal={terminal}
    />
  );
  let body;
  switch (state.at) {
    case "go_ahead":
      body = (
        <FlowGoAhead
          step={step}
          busy={busy}
          onAsk={ask}
          onSkip={skip}
          onPresets={() => {
            setState({ at: "presets" });
          }}
        />
      );
      break;
    case "presets":
      body = (
        <FlowPresets
          step={step}
          busy={busy}
          onSkip={skip}
          onAgents={() => {
            setState({ at: "go_ahead" });
          }}
          onBackToTools={onBackToTools}
          onPick={(file) => {
            send(
              proposePreset(step, file, request).then((proposal) => {
                setChoices(NO_CHOICES);
                setState({ at: "proposal", first: proposal });
              }),
            );
          }}
        />
      );
      break;
    case "at_work":
      body = (
        <div className="flow-step-split">
          {terminals}
          <AgentsAtWork
            view={atWork}
            watched={watched}
            busy={busy}
            onWatch={setPicked}
            onAct={rows.act}
            onStopAll={() => {
              send(stopAll(project, atWork, request).then(setState));
            }}
            onSkip={skip}
          />
        </div>
      );
      break;
    case "proposal":
      body = (
        <div className="flow-step-split">
          {terminals}
          {compared !== null && (
            <FlowCompare
              view={compared}
              busy={busy}
              onChoices={(proposal, picked) => {
                setPicks((p) => ({ ...p, [proposal]: picked }));
              }}
              onAskForChanges={(agent) => {
                setPicked(agent);
                terminal.current?.focus();
              }}
              onUsed={(proposal) => {
                ending.current = true;
                onDone(proposingAgent(atWork, proposal));
              }}
              onUseDefault={skip}
              renderOther={(tool) => {
                const row = atWork.rows.find((r) => r.tool === tool);
                return (
                  row && (
                    <AgentRow
                      row={row}
                      watch={atWork.watch}
                      watching={atWork.watching}
                      shown={row.agent !== null && row.agent === watched}
                      busy={busy}
                      onWatch={setPicked}
                      onAct={rows.act}
                    />
                  )
                );
              }}
            />
          )}
          {compared === null && view !== null && (
            <FlowProposal
              view={view}
              choices={choices}
              busy={busy}
              error={error}
              onChoices={setChoices}
              onError={setError}
              onUseDefault={skip}
              onAsk={() => {
                // Ask for changes goes to the proposal's own agent: its terminal shows and takes the keyboard.
                const agent = proposingAgent(atWork, view.proposal);
                if (agent !== null) setPicked(agent);
                terminal.current?.focus();
              }}
              onUse={() => {
                end(() => applyPreview(view), proposingAgent(atWork, view.proposal));
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
      {rows.dialogs}
      {error !== null && state.at !== "proposal" && <p className="onboarding-failure">{error}</p>}
    </div>
  );
}
