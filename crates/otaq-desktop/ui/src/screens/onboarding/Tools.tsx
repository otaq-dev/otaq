// The tools step (*2 · Tool checklist · everything installed*, *claude missing · blocking*, *gh and az missing ·
// non-blocking*; d0021, d0011): one row per tool with its version or why it is missing, the install command to copy,
// the permission-mode warning with its docs link, Re-check, and Continue (off while a required tool is missing, with
// the reason beside it), and under the list the starter agent's line (*14* `ODRG2`, M22). All of it is
// `OnboardingTools`'.
import { Button, IconButton } from "../../components";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { ToolRowView } from "../../generated/ToolRowView";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";

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
}

export function ToolsList({ tools, checking, warning, onDismiss, onRecheck, onContinue }: ToolsListProps) {
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
