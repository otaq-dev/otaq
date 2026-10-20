// Settings → Agents (T-0405; AGM-01, AGM-04, AGT-05's marks; d0108, d0077): the default model for new agents and the
// agents, each with its role tag, where it came from, its model as Settings shows it, its skills and its amber mark
// (`st-paused`: a model that can't run). Read-only until T-0410 draws adding, editing and the fallbacks. Everything
// comes from the view (`settings.agents`).
import { useEffect, useState } from "react";

import type { AgentListRow } from "../../generated/AgentListRow";
import type { AgentMarkView } from "../../generated/AgentMarkView";
import type { AgentsSection as AgentsSectionView } from "../../generated/AgentsSection";
import type { RequestError } from "../../generated/RequestError";
import { watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

/** The amber line of a mark, the model in mono. */
export function MarkLine({ mark }: { mark: AgentMarkView }) {
  return (
    <span className="settings-agent-mark-line">
      <Icon name="triangle-alert" size={12} />
      <span>
        {mark.before}
        {mark.model !== null && <code>{mark.model}</code>}
        {mark.after}
      </span>
    </span>
  );
}

function AgentRow({ row }: { row: AgentListRow }) {
  return (
    <div className="settings-agents-row" role="row">
      <span className="settings-agents-name" role="cell">
        <span>{row.name}</span>
        {row.role !== null && <span className="settings-agents-added">{row.role}</span>}
      </span>
      <span className="settings-agents-tool-model" role="cell">
        <span className="settings-agents-model" data-marked={row.mark ? true : undefined}>
          {row.model}
        </span>
      </span>
      <span className="settings-agents-use" role="cell">
        {row.skills.join(", ")}
      </span>
      <span className="settings-agents-use" role="cell">
        {row.came_from}
      </span>
      {row.mark && (
        <span className="settings-agents-mark">
          <MarkLine mark={row.mark} />
        </span>
      )}
    </div>
  );
}

/** The whole section as the view says. */
export function AgentsSectionBody({ section }: { section: AgentsSectionView }) {
  return (
    <section className="settings-section settings-agents" data-section="agents" aria-label={section.title}>
      <h3 className="settings-section-title">{section.title}</h3>
      <p className="settings-intro">{section.default_model}</p>
      <div className="settings-agents-table" role="table" aria-label={section.title} data-columns={4}>
        {section.rows.map((row) => (
          <AgentRow key={row.id} row={row} />
        ))}
      </div>
    </section>
  );
}

/** The global page's Agents section. It shows Otaq's copy of the model lists at once, then asks the tools for their
 * lists again (once per opening, d0086) and redraws with them. */
export function AgentsSection() {
  const [section, setSection] = useState<AgentsSectionView | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  useEffect(
    () =>
      watchView(
        () => request("settings.agents", { refresh: false }) as Promise<AgentsSectionView>,
        // A tool connected or hidden in Agent tools changes the default model and the models' words.
        ["agents_changed", "agent_tools_changed"],
        (state) => {
          if (state.view) setSection(state.view);
          setError(state.error ?? null);
        },
      ),
    [],
  );
  useEffect(() => {
    let live = true;
    request("settings.agents", { refresh: true }).then(
      (s) => {
        if (live) setSection(s as AgentsSectionView);
      },
      // The copy already shown stays; the next opening asks again.
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);
  return (
    <>
      {error && (
        <p className="settings-problem" role="alert">
          {error.message}
        </p>
      )}
      {section && <AgentsSectionBody section={section} />}
    </>
  );
}
