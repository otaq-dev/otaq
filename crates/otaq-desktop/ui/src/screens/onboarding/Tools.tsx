// The tools step (*2 · Tool checklist · everything installed*, *gh and az missing · non-blocking*; d0021, d0011, d0086;
// *14* `y2FCwP` / `vnBdi`, `z8OVF` / `rZRuR`): the agent tools first ("AGENT TOOLS · AT LEAST ONE", how many are
// connected, the blocking line while none is, and the same rows as Settings → Agent tools in their compact form:
// Sign in…, Show, the install command, "Otaq can see", the tool's own warning, the … menu without Disconnect…), then
// git, gh and az with their version or why they are missing and the command to copy; the old Claude Code warning
// when the service gives one, Re-check, and Continue (off while no agent tool is connected, with the reason beside
// it), and under the list the starter agent's line (*14* `ODRG2`, M22). All of it is `OnboardingTools`'.
import { Button, IconButton, ToolAvatar } from "../../components";
import type { AgentToolRow } from "../../generated/AgentToolRow";
import type { OnboardingAgentTools } from "../../generated/OnboardingAgentTools";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { ToolRowView } from "../../generated/ToolRowView";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import {
  type AgentToolAction,
  CanSee,
  CommandBox,
  DetailLine,
  MARK_ICONS,
  markOf,
  RowMenu,
  ToolWarning,
} from "../settings/AgentTools";

function AgentRow({
  row,
  menuLabel,
  onAction,
}: {
  row: AgentToolRow;
  menuLabel: string;
  onAction: (action: AgentToolAction) => void;
}) {
  const mark = markOf(row);
  return (
    <li className="tool-row agent-tool-step-row" data-mark={mark}>
      <div className="tool-row-head">
        <span className="tool-row-icon">
          <Icon name={MARK_ICONS[mark]} size={16} />
        </span>
        <ToolAvatar avatar={row.avatar} />
        <span className="tool-row-name agent-tool-step-name">{row.name}</span>
        <span className="tool-row-status">
          <DetailLine row={row} onAction={onAction} />
        </span>
        {row.sign_in !== null && (
          <Button
            label={row.sign_in}
            variant="secondary"
            icon="square-terminal"
            compact
            onClick={() => {
              onAction({ kind: "sign_in" });
            }}
          />
        )}
        <RowMenu row={row} label={menuLabel} onAction={onAction} />
      </div>
      {row.install !== null && <CommandBox command={row.install} label={false} />}
      {row.update !== null && <CommandBox command={row.update} />}
      <CanSee row={row} />
      <ToolWarning row={row} onAction={onAction} />
    </li>
  );
}

/** The agent tools block: heading and count, the blocking line, the rows. */
export function AgentToolsBlock({
  block,
  onAction,
}: {
  block: OnboardingAgentTools;
  onAction: (tool: string, action: AgentToolAction) => void;
}) {
  return (
    <>
      {block.blocked !== null && (
        <div className="agent-tools-blocked" role="alert">
          <Icon name="circle-x" size={15} />
          <span>{block.blocked}</span>
        </div>
      )}
      <div className="tool-heading">
        <span>{block.heading}</span>
        <span className="tool-heading-count">{block.count}</span>
      </div>
      {block.rows.length > 0 && (
        <ul className="tool-rows">
          {block.rows.map((row) => (
            <AgentRow
              key={row.tool}
              row={row}
              menuLabel={block.menu_label}
              onAction={(action) => {
                onAction(row.tool, action);
              }}
            />
          ))}
        </ul>
      )}
    </>
  );
}

function Row({ row, copy }: { row: ToolRowView; copy: string }) {
  return (
    <li className="tool-row" data-look={row.look}>
      <div className="tool-row-head">
        <span className="tool-row-icon">
          <Icon name={row.icon} size={16} />
        </span>
        <span className="tool-row-name">{row.name}</span>
        <span className="tool-row-badge">{row.badge}</span>
        <span className="tool-row-status">{row.status}</span>
      </div>
      {row.explanation !== null && <p className="tool-row-explanation">{row.explanation}</p>}
      {row.command !== null && (
        <div className="tool-row-command">
          <code>{row.command}</code>
          <IconButton
            icon="copy"
            label={copy}
            onClick={() => {
              if (row.command !== null) navigator.clipboard.writeText(row.command).catch(() => undefined);
            }}
          />
        </div>
      )}
    </li>
  );
}

export interface ToolsListProps {
  tools: OnboardingTools;
  checking: boolean;
  /** The permission-mode warning shows (and wasn't closed). */
  warning: boolean;
  onDismiss: () => void;
  onRecheck: () => void;
  onContinue: () => void;
  /** A click on an agent tool's row (Sign in…, Show, its … menu). */
  onAgentAction: (tool: string, action: AgentToolAction) => void;
}

export function ToolsList({
  tools,
  checking,
  warning,
  onDismiss,
  onRecheck,
  onContinue,
  onAgentAction,
}: ToolsListProps) {
  return (
    <>
      <h1 className="onboarding-title">{tools.title}</h1>
      <p className="onboarding-text">{tools.text}</p>
      {warning && tools.warning && (
        <div className="tool-warning" role="note">
          <Icon name="shield-alert" size={15} />
          <span className="tool-warning-text">
            <span className="tool-warning-title">{tools.warning.title}</span>
            <span>
              {tools.warning.before}
              <button
                type="button"
                className="tool-warning-link"
                onClick={() => {
                  request("notices.open_docs").catch(() => undefined);
                }}
              >
                {tools.warning.link}
              </button>
              {tools.warning.after}
            </span>
          </span>
          <IconButton icon="x" label={tools.warning.close} onClick={onDismiss} />
        </div>
      )}
      {tools.waiting === null && <AgentToolsBlock block={tools.agent_tools} onAction={onAgentAction} />}
      {tools.waiting === null && <div className="tool-heading">{tools.other_heading}</div>}
      {tools.rows.length > 0 ? (
        <ul className="tool-rows">
          {tools.rows.map((row) => (
            <Row key={row.name} row={row} copy={tools.copy} />
          ))}
        </ul>
      ) : (
        <p className="tool-rows tool-rows-waiting">{tools.waiting}</p>
      )}
      {tools.agents_line !== null && <p className="tool-agents-line">{tools.agents_line}</p>}
      {tools.error !== null && <p className="onboarding-failure">{tools.error}</p>}
      <div className="onboarding-foot">
        <Button
          label={tools.recheck}
          variant="secondary"
          icon="rotate-ccw"
          loading={checking}
          onClick={onRecheck}
        />
        <span className="onboarding-spacer" />
        {tools.continue_reason !== null && <span className="tool-reason">{tools.continue_reason}</span>}
        <Button
          label={tools.continue_label}
          icon="arrow-right"
          disabled={!tools.continue_enabled || checking}
          onClick={onContinue}
        />
      </div>
    </>
  );
}
