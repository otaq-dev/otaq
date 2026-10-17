// The dialogs of Settings → Agents (T-0343; AGT-02, AGT-05, d0086): Add agent and Edit agent · <name> (*15 → Add
// agent · model picker open* `k8WyHN`, *Edit agent · the mark in the dialog* `WFHlh`), in their one-tool form: no
// Fallback field, the tool read-only; and Delete…: refused while anything uses the entry (`O7UTA`, Show tickets lists
// every key and each opens its panel), else a plain confirmation (`Wjp2r`). Every word, check and enabled state comes
// from `named_agents.rs`; this keeps the text being typed and sends it on each change (`onDraft`) for the live checks.
import { useRef, useState } from "react";

import { Button, ConfirmDialog, Dialog, Field, TextInput, Tooltip } from "../../components";
import type { AgentDeleteDialog } from "../../generated/AgentDeleteDialog";
import type { AgentDialog as AgentDialogView } from "../../generated/AgentDialog";
import type { AgentDraft } from "../../generated/AgentDraft";
import type { AgentMarkView } from "../../generated/AgentMarkView";
import { Icon } from "../../theme/Icon";
import { ModelPicker } from "./ModelPicker";

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
  onSave: (draft: AgentDraft) => void;
  onCancel: () => void;
  /** Edit's Delete…: asks first (`settings.agent_delete_dialog`). */
  onDelete: (id: string) => void;
}

/** Add agent or Edit agent. Esc and Cancel close it without saving; ↵ saves when Save is enabled. */
export function AgentDialog({ dialog, onDraft, onSave, onCancel, onDelete }: AgentDialogProps) {
  const name = useRef<HTMLInputElement>(null);
  const [draft, setDraft] = useState<AgentDraft>(dialog.draft);
  const change = (next: AgentDraft) => {
    setDraft(next);
    onDraft(next);
  };
  const save = () => {
    if (dialog.save_enabled) onSave(draft);
  };
  const remove = dialog.delete;
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
            <span id={id} className="select settings-agent-tool" role="note">
              <span className="select-value">{dialog.tool}</span>
              <span className="settings-model-detail">{dialog.tool_note}</span>
            </span>
          )}
        </Field>
        <Field label={dialog.model_label} hint={dialog.picker.hint} error={dialog.model_problem}>
          {(id) => (
            <ModelPicker
              id={id}
              label={dialog.model_label}
              picker={dialog.picker}
              onPick={(model) => {
                change({ ...draft, model });
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
