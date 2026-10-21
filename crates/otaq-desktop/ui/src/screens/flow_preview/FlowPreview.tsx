// The flow preview (T-0420, FLW-04; *18 → Row · The preview: next to the Project agent* `s18-27`/`s18-28`, *a whole
// flow, a change* `s18-29`/`s18-30`, *warnings, problems, no change* `s18-31`/`s18-32`, *back to the previous flow, a
// rename* `s18-35`/`s18-36`): a sheet over the main area, beside the Project agent pane, which stays usable. Every
// word and state comes from `flow_preview.view`; this keeps the picks until Apply, the Ask for more changes line, and
// the request in flight.
import { type ReactNode, useCallback, useEffect, useId, useRef, useState } from "react";

import type { OverlayProps } from "../../app/registry";
import { Button, IconButton, Radio, Select, TextInput } from "../../components";
import type { FlowPreviewAgentRow } from "../../generated/FlowPreviewAgentRow";
import type { FlowPreviewChoices } from "../../generated/FlowPreviewChoices";
import type { FlowPreviewClash } from "../../generated/FlowPreviewClash";
import type { FlowPreviewClashPick } from "../../generated/FlowPreviewClashPick";
import type { FlowPreviewLoad } from "../../generated/FlowPreviewLoad";
import type { FlowPreviewProblem } from "../../generated/FlowPreviewProblem";
import type { FlowPreviewTable } from "../../generated/FlowPreviewTable";
import type { FlowPreviewView } from "../../generated/FlowPreviewView";
import type { IconName } from "../../generated/IconName";
import { asRequestError, useServiceView } from "../../service";
import { pickFolder, request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { optionItems } from "../settings/Agents";
import { ModelPicker } from "../settings/ModelPicker";
import {
  applyPreview,
  askForChanges,
  cancelPreview,
  NO_CHOICES,
  pickedTool,
  withClash,
  withFolder,
  withModel,
} from "./logic";

/** What the preview refetches on: its proposal, the agents and tools it names, the tickets a change waits for. */
export const REFETCH_ON = [
  "flow_changed",
  "agents_changed",
  "agent_tools_changed",
  "ticket_changed",
  "card_status_changed",
];

type Tone = "info" | "ok" | "amber" | "red";

function Note({ tone, icon, children }: { tone: Tone; icon: IconName; children: ReactNode }) {
  return (
    <div className="flow-preview-note" data-tone={tone}>
      <Icon name={icon} size={14} />
      <div className="flow-preview-note-text">{children}</div>
    </div>
  );
}

function StageTable({ table }: { table: FlowPreviewTable }) {
  return (
    <div className="flow-preview-table" role="table">
      <div className="flow-preview-table-head" role="row">
        {table.headings.map((h) => (
          <span key={h} role="columnheader">
            {h}
          </span>
        ))}
      </div>
      <div className="flow-preview-always" role="row">
        <Icon name="circle-dashed" size={13} />
        <span role="cell">{table.always}</span>
      </div>
      {table.rows.map((row) => (
        <div key={row.id} className="flow-preview-stage" role="row">
          <div className="flow-preview-stage-line">
            <span className="flow-preview-stage-name" role="cell">
              <Icon name={row.icon} size={14} />
              <span>
                <strong>{row.name}</strong>
                <small>{row.kind}</small>
              </span>
            </span>
            <span className="flow-preview-stage-agent" role="cell" data-muted={row.agent_muted || undefined}>
              <span>{row.agent}</span>
              {row.model !== null && <small>{row.model}</small>}
            </span>
            <span
              className="flow-preview-stage-options"
              role="cell"
              data-muted={row.options_muted || undefined}
            >
              {row.options}
            </span>
          </div>
          {row.why !== null && (
            <p className="flow-preview-why">
              <Icon name="sparkles" size={12} />
              <span>{row.why}</span>
            </p>
          )}
        </div>
      ))}
    </div>
  );
}

function AgentLine({ row }: { row: FlowPreviewAgentRow }) {
  const [open, setOpen] = useState(false);
  const details = row.instructions !== null || row.skills.length > 0 || row.why !== null;
  return (
    <div className="flow-preview-agent">
      <button
        type="button"
        className="flow-preview-agent-line"
        aria-expanded={open}
        disabled={!details}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        <Icon name={open ? "chevron-down" : "chevron-right"} size={13} />
        <span className="flow-preview-agent-name">{row.name}</span>
        <span className="flow-preview-agent-summary">{row.summary}</span>
        <span className="flow-preview-agent-tag">{row.tag}</span>
      </button>
      {open && (
        <div className="flow-preview-agent-details">
          {row.instructions !== null && <p className="flow-preview-agent-instructions">{row.instructions}</p>}
          {row.skills.length > 0 && (
            <p className="flow-preview-agent-skills">
              <span>{row.skills_label}</span>
              <span>{row.skills.join(", ")}</span>
            </p>
          )}
          {row.why !== null && (
            <p className="flow-preview-why">
              <Icon name="sparkles" size={12} />
              <span>{row.why}</span>
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function ClashChoices({
  clash,
  onPick,
}: {
  clash: FlowPreviewClash;
  onPick: (pick: FlowPreviewClashPick) => void;
}) {
  const name = useId();
  return (
    <Note tone="amber" icon="git-compare">
      <p>{clash.text}</p>
      <div className="flow-preview-clash" role="radiogroup" aria-label={clash.text}>
        {clash.options.map((o) => (
          <Radio
            key={o.id}
            name={name}
            checked={o.selected}
            label={o.label}
            onChange={(checked) => {
              if (checked) onPick(o.id as FlowPreviewClashPick);
            }}
          />
        ))}
      </div>
    </Note>
  );
}

function Problem({
  problem,
  choices,
  onChoices,
  onError,
}: {
  problem: FlowPreviewProblem;
  choices: FlowPreviewChoices;
  onChoices: (choices: FlowPreviewChoices) => void;
  onError: (message: string) => void;
}) {
  const [picking, setPicking] = useState(false);
  const fix = problem.fix;
  let action: ReactNode = null;
  let below: ReactNode = null;
  if (fix?.kind === "folder") {
    action = (
      <Button
        variant="secondary"
        compact
        label={fix.label}
        onClick={() => {
          pickFolder(fix.picker_title).then(
            (folder) => {
              if (folder !== null) onChoices(withFolder(choices, fix.skill, folder));
            },
            (error: unknown) => {
              onError(asRequestError(error).message);
            },
          );
        }}
      />
    );
    if (fix.picked !== null) below = <span className="flow-preview-picked">{fix.picked}</span>;
  } else if (fix?.kind === "model") {
    const model = fix.fix;
    action = (
      <Button
        variant="secondary"
        compact
        label={fix.label}
        onClick={() => {
          setPicking((p) => !p);
        }}
      />
    );
    if (picking || problem.fixed) {
      const tool = pickedTool(choices, model.agent);
      const chosen = model.tools.find((t) => t.selected);
      below = (
        <span className="flow-preview-model-fix">
          <Select
            label={model.tool_label}
            value={chosen?.label ?? model.tool_label}
            items={optionItems(model.tools)}
            onChange={(id) => {
              onChoices(withModel(choices, model.agent, id));
            }}
          />
          {model.picker !== null && tool !== null && (
            <ModelPicker
              label={model.model_label}
              picker={model.picker}
              onPick={(picked) => {
                onChoices(withModel(choices, model.agent, tool, picked));
              }}
              onRetry={(t) => {
                request("settings.agents_models_retry", { tool: t }).catch(() => undefined);
              }}
            />
          )}
        </span>
      );
    }
  }
  return (
    <div className="flow-preview-note flow-preview-problem" data-tone={problem.fixed ? "info" : "red"}>
      <Icon name={problem.fixed ? "circle-check" : "circle-x"} size={14} />
      <div className="flow-preview-note-text">
        <div className="flow-preview-problem-line">
          <span>{problem.text}</span>
          {action}
        </div>
        {below}
      </div>
    </div>
  );
}

/** The sheet's contents, drawn from the view (the tests render it). */
export function PreviewBody({
  view,
  choices,
  onChoices,
  onError,
}: {
  view: FlowPreviewView;
  choices: FlowPreviewChoices;
  onChoices: (choices: FlowPreviewChoices) => void;
  onError: (message: string) => void;
}) {
  return (
    <>
      {view.same !== null && (
        <Note tone="ok" icon="circle-check">
          {view.same}
        </Note>
      )}
      {view.problems.map((p) => (
        <Problem key={p.text} problem={p} choices={choices} onChoices={onChoices} onError={onError} />
      ))}
      {view.table !== null && <StageTable table={view.table} />}
      {view.changes.length > 0 && (
        <ul className="flow-preview-changes">
          {view.changes.map((c) => (
            <li key={c.text} className="flow-preview-change">
              <span className="flow-preview-change-line">
                <Icon name={c.icon} size={13} />
                <span>{c.text}</span>
              </span>
              {c.detail !== null && <small>{c.detail}</small>}
            </li>
          ))}
        </ul>
      )}
      {view.agents !== null && (
        <section className="flow-preview-agents" aria-label={view.agents.heading}>
          <h3>{view.agents.heading}</h3>
          {view.agents.rows.map((row) => (
            <AgentLine key={row.name} row={row} />
          ))}
          {view.agents.already !== null && <p className="flow-preview-already">{view.agents.already}</p>}
        </section>
      )}
      {view.clashes.map((clash) => (
        <ClashChoices
          key={clash.agent}
          clash={clash}
          onPick={(pick) => {
            onChoices(withClash(choices, clash.agent, pick));
          }}
        />
      ))}
      {view.warnings.map((w) => (
        <Note key={w.text} tone={w.level} icon="triangle-alert">
          {w.text}
        </Note>
      ))}
      {view.applies !== null && (
        <Note tone="info" icon="info">
          {view.applies}
        </Note>
      )}
    </>
  );
}

/** What the footer is doing: its buttons, the Ask for more changes line, or a request in flight. */
export type FooterMode = "buttons" | "asking" | "busy";

/** The footer: the agent's waiting hint, then Cancel · Ask for more changes · Apply (or Close), or the line to send. */
export function PreviewFooter({
  view,
  mode,
  text,
  onText,
  onCancel,
  onAsk,
  onSend,
  onBack,
  onApply,
}: {
  view: FlowPreviewView;
  mode: FooterMode;
  text: string;
  onText: (text: string) => void;
  onCancel: () => void;
  onAsk: () => void;
  onSend: () => void;
  onBack: () => void;
  onApply: () => void;
}) {
  const busy = mode === "busy";
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (mode === "asking") field.current?.focus();
  }, [mode]);
  if (mode === "asking" && view.ask !== null) {
    const ask = view.ask;
    return (
      <div className="flow-preview-foot flow-preview-asking">
        <TextInput
          ref={field}
          label={ask.label}
          value={text}
          placeholder={ask.placeholder}
          onChange={onText}
        />
        <Button variant="ghost" label={ask.back} onClick={onBack} />
        <Button label={ask.send} icon="send" disabled={text.trim() === ""} onClick={onSend} />
      </div>
    );
  }
  return (
    <div className="flow-preview-foot">
      <span className="flow-preview-waits">
        {view.agent_waits !== null && (
          <>
            <Icon name="terminal" size={12} />
            <span>{view.agent_waits}</span>
          </>
        )}
      </span>
      {view.cancel !== null && (
        <Button variant="ghost" label={view.cancel} disabled={busy} onClick={onCancel} />
      )}
      {view.ask !== null && (
        <Button variant="secondary" label={view.ask.label} disabled={busy} onClick={onAsk} />
      )}
      {view.apply !== null && (
        <span title={view.apply.off ?? undefined}>
          <Button
            label={view.apply.label}
            icon="check"
            disabled={busy || !view.apply.enabled}
            loading={busy}
            onClick={onApply}
          />
        </span>
      )}
      {view.close !== null && <Button label={view.close} disabled={busy} onClick={onCancel} />}
    </div>
  );
}

/** The `flow_preview` overlay: the project's open proposal, or nothing once it is gone (it then closes itself). */
export function FlowPreviewSheet({ params, close }: OverlayProps<"flow_preview">) {
  // Another project's proposal starts afresh: no picks, no line typed.
  return <Sheet key={params.project} project={params.project} focus={params.focus ?? false} close={close} />;
}

function Sheet({ project, focus, close }: { project: string; focus: boolean; close: () => void }) {
  const [choices, setChoices] = useState<FlowPreviewChoices>(NO_CHOICES);
  const [mode, setMode] = useState<FooterMode>("buttons");
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const titleId = useId();
  const sheet = useRef<HTMLElement>(null);
  const load = useCallback(
    () => request("flow_preview.view", { project, choices }) as Promise<FlowPreviewLoad>,
    [project, choices],
  );
  const state = useServiceView(load, REFETCH_ON);
  const view = state.view?.view ?? null;
  const gone = state.view !== undefined && state.view.view === null;
  useEffect(() => {
    if (gone) close();
  }, [gone, close]);
  // Opened by the user (Settings → Flow): the keyboard moves in. A proposal that arrives on its own doesn't take it
  // from the terminal the user may be typing in.
  const shown = view !== null;
  useEffect(() => {
    if (focus && shown) sheet.current?.focus();
  }, [focus, shown]);

  const answer = useCallback(
    (sent: Promise<unknown> | null, after: FooterMode = "buttons") => {
      if (sent === null) return;
      setMode("busy");
      setError(null);
      sent.then(
        () => {
          close();
        },
        (e: unknown) => {
          setMode(after);
          setError(asRequestError(e).message);
        },
      );
    },
    [close],
  );
  const cancel = useCallback(() => {
    if (view !== null) answer(cancelPreview(view));
  }, [view, answer]);
  // Esc: back from the Ask for more changes line, else Cancel.
  useEffect(() => {
    const box = sheet.current;
    if (!box) return;
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      e.preventDefault();
      e.stopPropagation();
      if (mode === "asking") setMode("buttons");
      else if (mode === "buttons") cancel();
    };
    box.addEventListener("keydown", onKey);
    return () => {
      box.removeEventListener("keydown", onKey);
    };
  }, [mode, cancel, shown]);
  if (view === null) return null;
  return (
    <div className="flow-preview-scrim" role="presentation">
      <section
        ref={sheet}
        className="flow-preview"
        role="dialog"
        aria-modal="false"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        <header className="flow-preview-head">
          <div className="flow-preview-titles">
            <h2 id={titleId}>{view.title}</h2>
            <p>{view.subtitle}</p>
          </div>
          <IconButton icon="x" label={view.close_label} disabled={mode === "busy"} onClick={cancel} />
        </header>
        <div className="flow-preview-body">
          <PreviewBody view={view} choices={choices} onChoices={setChoices} onError={setError} />
          {error !== null && (
            <p className="flow-preview-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <PreviewFooter
          view={view}
          mode={mode}
          text={text}
          onText={setText}
          onCancel={cancel}
          onAsk={() => {
            setMode("asking");
          }}
          onBack={() => {
            setMode("buttons");
          }}
          onSend={() => {
            answer(askForChanges(view, text), "asking");
          }}
          onApply={() => {
            answer(applyPreview(view));
          }}
        />
      </section>
    </div>
  );
}
