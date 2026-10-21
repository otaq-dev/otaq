// The proposals compared (T-0435, ASK-07; *14 → Row · Flow step: the proposal* `s14-16`/`s14-17`, *two proposals to
// compare* and *one proposal ready, the other stopped*): in Flow setup, two or more proposals side by side, each with
// every why line and Ask for changes · Use this flow, a tool still working or stopped beside them (its agents-at-work
// row, drawn by the page), then the closing line and one Use the default. Every word comes from
// `flow_preview.compare` (`flow_preview.rs`'s `compare`); this keeps the request in flight and its error.
import { type ReactNode, useState } from "react";

import { Button } from "../../components";
import type { FlowCompareProposal } from "../../generated/FlowCompareProposal";
import type { FlowCompareView } from "../../generated/FlowCompareView";
import type { FlowPreviewChoices } from "../../generated/FlowPreviewChoices";
import { asRequestError } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { PreviewBody, PreviewFooter } from "./FlowPreview";
import { type Ask, applyProposal } from "./logic";

export interface FlowCompareProps {
  /** `flow_preview.compare`'s answer while `compare` is true. */
  view: FlowCompareView;
  /** A proposal's picks changed (a skill folder or a model for a problem): the page asks the view again with them. */
  onChoices: (proposal: number, choices: FlowPreviewChoices) => void;
  /** Ask for changes on a flow agent's proposal: the page focuses that agent's terminal tab. */
  onAskForChanges: (agent: string) => void;
  /** Use this flow was applied (`flow_preview.apply` with that proposal's number): Flow setup is settled. */
  onUsed: (proposal: number) => void;
  /** Use the default under the proposals. */
  onUseDefault: () => void;
  /** A tool still working or stopped (its id, `opencode`): its agents-at-work row, shown beside the proposals. */
  renderOther: (tool: string) => ReactNode;
  /** Use the default (or another answer) is on its way: every button waits. */
  busy?: boolean;
  ask?: Ask;
}

function Proposal({
  proposal,
  busy,
  onChoices,
  onError,
  onAsk,
  onUse,
}: {
  proposal: FlowCompareProposal;
  busy: boolean;
  onChoices: (choices: FlowPreviewChoices) => void;
  onError: (message: string) => void;
  onAsk: () => void;
  onUse: () => void;
}) {
  const { view } = proposal;
  const ignore = () => undefined;
  return (
    <section
      className="flow-preview flow-compare-proposal"
      aria-label={`${view.title} · ${view.subtitle}`}
      data-tool={proposal.tool ?? undefined}
    >
      <header className="flow-preview-head">
        <div className="flow-preview-titles">
          <h2>{view.title}</h2>
          <p>{view.subtitle}</p>
        </div>
      </header>
      <div className="flow-preview-body">
        <PreviewBody view={view} choices={view.choices} onChoices={onChoices} onError={onError} />
      </div>
      <PreviewFooter
        view={view}
        mode={busy ? "busy" : "buttons"}
        text=""
        onText={ignore}
        onCancel={ignore}
        onAsk={onAsk}
        onSend={ignore}
        onBack={ignore}
        onApply={onUse}
      />
    </section>
  );
}

/** The proposals side by side, the rows of the tools not ready beside them, and one Use the default under them. */
export function FlowCompare({
  view,
  onChoices,
  onAskForChanges,
  onUsed,
  onUseDefault,
  renderOther,
  busy = false,
  ask = request,
}: FlowCompareProps) {
  const [using, setUsing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const waiting = busy || using;
  const use = (proposal: FlowCompareProposal) => {
    setUsing(true);
    setError(null);
    applyProposal(proposal, ask).then(
      () => {
        setUsing(false);
        onUsed(proposal.view.proposal);
      },
      (e: unknown) => {
        setUsing(false);
        setError(asRequestError(e).message);
      },
    );
  };
  return (
    <section className="flow-compare">
      <div className="flow-compare-row">
        {view.proposals.map((p) => (
          <Proposal
            key={p.view.proposal}
            proposal={p}
            busy={waiting}
            onChoices={(choices) => {
              onChoices(p.view.proposal, choices);
            }}
            onError={setError}
            onAsk={() => {
              if (p.agent !== null) onAskForChanges(p.agent);
            }}
            onUse={() => {
              use(p);
            }}
          />
        ))}
        {view.others.map((tool) => (
          <div key={tool} className="flow-compare-other" data-tool={tool}>
            {renderOther(tool)}
          </div>
        ))}
      </div>
      {error !== null && (
        <p className="flow-preview-error" role="alert">
          {error}
        </p>
      )}
      <div className="flow-compare-foot">
        {view.closes_others !== null && (
          <>
            <Icon name="info" size={13} />
            <span>{view.closes_others}</span>
          </>
        )}
        <span className="flow-compare-spacer" />
        {view.use_default !== null && (
          <Button variant="ghost" label={view.use_default} disabled={waiting} onClick={onUseDefault} />
        )}
      </div>
    </section>
  );
}
