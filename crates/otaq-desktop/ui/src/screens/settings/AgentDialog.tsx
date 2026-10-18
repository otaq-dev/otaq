// The dialogs of Settings → Agents (T-0343, T-0366; AGT-02, AGT-05, AGT-08, d0086): Add agent and Edit agent · <name>
// (*15 → Add agent · model picker open* `k8WyHN`, *Edit agent · the mark in the dialog* `WFHlh`); with one tool no
// Fallback field and the tool read-only, with several (`ObF25`) a Tool picker of the connected tools (a change asks
// the dialog again with `tool`: the model goes back to the tool default and a fallback on that tool goes) and a
// Fallback field of agents on other tools; and Delete…: refused while anything uses the entry (`O7UTA`, Show tickets
// lists every key and each opens its panel), else a plain confirmation (`Wjp2r`). Every word, check and enabled state
// comes from `named_agents.rs`; this keeps the text being typed and sends it on each change (`onDraft`) for the live
// checks.
import { useEffect, useRef, useState } from "react";

import { Button, ConfirmDialog, Dialog, Field, TextInput, ToolAvatar, Tooltip } from "../../components";
import type { AgentDeleteDialog } from "../../generated/AgentDeleteDialog";
import type { AgentDialog as AgentDialogView } from "../../generated/AgentDialog";
import type { AgentDraft } from "../../generated/AgentDraft";
import type { AgentMarkView } from "../../generated/AgentMarkView";
import { Icon } from "../../theme/Icon";
import { AgentPickerField, ModelPicker, ToolPickerField } from "./ModelPicker";

/** The amber line of a marked row, its model in mono. */
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

/** The same line at the top of Edit agent, with the tool's own message quoted under it. */
function MarkBox({ mark }: { mark: AgentMarkView }) {
  return (
    <div className="settings-agent-mark-box" role="note">
      <span className="settings-agent-mark-title">
        <Icon name="triangle-alert" size={13} />
        <span>
          {mark.before}
          {mark.model ?? ""}
          {mark.after}
        </span>
      </span>
      {mark.message !== null && <q className="settings-agent-mark-quote">{mark.message}</q>}
    </div>
  );
}

export interface AgentDialogProps {
  dialog: AgentDialogView;
  /** Each change of the fields: the page asks for the dialog again with it (`settings.agent_dialog`). */
  onDraft: (draft: AgentDraft) => void;
  /** The Tool picker changed to `tool`: the page asks for the dialog again with it (`settings.agent_dialog`'s
   * `tool`), the model already cleared in `draft`. */
  onTool: (tool: string, draft: AgentDraft) => void;
  /** The model list's Retry for `tool` (`settings.agent_models_retry`), with the fields as they are. */
  onRetry: (tool: string, draft: AgentDraft) => void;
  onSave: (draft: AgentDraft) => void;
  onCancel: () => void;
  /** Edit's Delete…: asks first (`settings.agent_delete_dialog`). */
  onDelete: (id: string) => void;
}

/** Add agent or Edit agent. Esc and Cancel close it without saving; ↵ saves when Save is enabled. */
export function AgentDialog({
  dialog,
  onDraft,
  onTool,
  onRetry,
  onSave,
  onCancel,
  onDelete,
}: AgentDialogProps) {
  const name = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<AgentDraft>(dialog.draft);
  // After a Tool change, the answer's fallback wins (one on the new tool went); the fields keep their own text.
  const toolChanged = useRef(false);
  const answered = dialog.draft;
  useEffect(() => {
    if (!toolChanged.current) return;
    toolChanged.current = false;
    setDraft((d) => ({ ...d, tool: answered.tool, model: answered.model, fallback: answered.fallback }));
  }, [answered]);
  const change = (next: AgentDraft) => {
    setDraft(next);
    onDraft(next);
  };
  const changeTool = (tool: string) => {
    const next = { ...draft, tool, model: null };
    setDraft(next);
    toolChanged.current = true;
    onTool(tool, next);
  };
  const save = () => {
    if (dialog.save_enabled) onSave(draft);
  };
  const remove = dialog.delete;
  const fallback = dialog.fallback;
  const deleteButton = remove && dialog.id !== null && (
    <Button
      variant="secondary"
      label={remove.label}
      disabled={!remove.enabled}
      onClick={() => {
        if (dialog.id !== null) onDelete(dialog.id);
      }}
    />
  );
  return (
    <Dialog
      title={dialog.title}
      width={480}
      onConfirm={save}
      onCancel={onCancel}
      initialFocus={name}
      hints={
        deleteButton && (
          <span className="settings-agent-dialog-delete">
            {remove.tooltip !== null ? <Tooltip text={remove.tooltip}>{deleteButton}</Tooltip> : deleteButton}
          </span>
        )
      }
      footer={
        <>
          <Button variant="secondary" label={dialog.cancel_label} onClick={onCancel} />
          <Button label={dialog.save_label} disabled={!dialog.save_enabled} onClick={save} />
        </>
      }
    >
      <div className="settings-agent-dialog">
        {dialog.mark && <MarkBox mark={dialog.mark} />}
        <Field label={dialog.name_label} error={dialog.name_problem}>
          {(id) => (
            <TextInput
              ref={name}
              id={id}
              value={draft.name}
              invalid={dialog.name_problem !== null}
              onChange={(text) => {
                change({ ...draft, name: text });
              }}
            />
          )}
        </Field>
        <Field label={dialog.tool_label}>
          {(id) => (
            <>
              {dialog.tool_options.length > 0 ? (
                <ToolPickerField
                  id={id}
                  label={dialog.tool_label}
                  tool={dialog.tool}
                  avatar={dialog.tool_avatar}
                  options={dialog.tool_options}
                  onPick={changeTool}
                />
              ) : (
                <span id={id} className="select settings-agent-tool" data-read-only role="note">
                  <span className="select-value settings-agent-value">
                    {dialog.tool_avatar && <ToolAvatar avatar={dialog.tool_avatar} />}
                    <span>{dialog.tool}</span>
                  </span>
                  {dialog.tool_note !== null && (
                    <span className="settings-model-detail">{dialog.tool_note}</span>
                  )}
                </span>
              )}
              {dialog.tool_problem !== null && (
                <span className="settings-agent-mark-line settings-agent-tool-problem" role="note">
                  <Icon name="triangle-alert" size={12} />
                  <span>{dialog.tool_problem}</span>
                </span>
              )}
            </>
          )}
        </Field>
        <Field
          label={dialog.model_label}
          hint={dialog.picker.status?.note ?? dialog.picker.hint}
          error={dialog.model_problem}
        >
          {(id) => (
            <ModelPicker
              id={id}
              label={dialog.model_label}
              picker={dialog.picker}
              onPick={(model) => {
                change({ ...draft, model });
              }}
              onRetry={(tool) => {
                onRetry(tool, draft);
              }}
            />
          )}
        </Field>
        <Field label={dialog.use_for_label} hint={dialog.use_for_hint}>
          {(id) => (
            <TextInput
              id={id}
              value={draft.use_for}
              onChange={(text) => {
                change({ ...draft, use_for: text });
              }}
            />
          )}
        </Field>
        {fallback && (
          <Field label={fallback.label} hint={fallback.hint} error={fallback.problem}>
            {(id) => (
              <AgentPickerField
                id={id}
                label={fallback.label}
                picker={fallback.picker}
                none
                onPick={(fallback) => {
                  change({ ...draft, fallback });
                }}
              />
            )}
          </Field>
        )}
      </div>
    </Dialog>
  );
}

/** Delete…: the refused dialog (OK, Show tickets) or the plain confirmation (Cancel, Delete). */
export function AgentDeleteDialogView({
  dialog,
  onClose,
  onDelete,
  onOpenTicket,
}: {
  dialog: AgentDeleteDialog;
  onClose: () => void;
  onDelete: (id: string) => void;
  /** A key of Show tickets' list: opens its panel. */
  onOpenTicket: (key: string) => void;
}) {
  const [all, setAll] = useState(false);
  if (dialog.kind === "confirm")
    return (
      <ConfirmDialog
        title={dialog.title}
        destructive
        confirmLabel={dialog.delete_label}
        width={440}
        onConfirm={() => {
          onDelete(dialog.id);
        }}
        onCancel={onClose}
        body={
          <>
            <p className="dialog-text settings-agent-delete-text">{dialog.text}</p>
            <p className="settings-agent-delete-note">
              <Icon name="history" size={13} />
              <span>{dialog.note}</span>
            </p>
          </>
        }
      />
    );
  const showTickets = dialog.show_tickets_label;
  return (
    <Dialog
      title={dialog.title}
      width={440}
      onConfirm={onClose}
      onCancel={onClose}
      footer={
        <>
          {showTickets !== null && !all && (
            <Button
              variant="secondary"
              label={showTickets}
              onClick={() => {
                setAll(true);
              }}
            />
          )}
          <Button label={dialog.ok_label} onClick={onClose} />
        </>
      }
    >
      <p className="dialog-text settings-agent-delete-text">{dialog.text}</p>
      <div className="settings-agent-uses">
        {dialog.lines.map((line) => (
          <span key={line} className="settings-agent-use">
            <Icon name="corner-down-right" size={12} />
            <span>{line}</span>
          </span>
        ))}
      </div>
      {all && (
        <ul className="settings-agent-tickets" aria-label={showTickets ?? undefined}>
          {dialog.tickets.map((key) => (
            <li key={key}>
              <button
                type="button"
                className="settings-link settings-agent-ticket"
                onClick={() => {
                  onOpenTicket(key);
                }}
              >
                {key}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Dialog>
  );
}
