// A Project agent's tab around its terminal (T-0243, the UI part of T-0204; UI-09, PAG-04, PAG-05, PAG-06, TKT-08;
// d0068, d0060; *18 · Project agents & terminals → Row · states, Row · lifecycle*): the Agent header (name with
// Rename, the state pill, tokens, what it may do, "named by you", Resume once it ended) and, beside the terminal,
// Created and changed with a link per ticket and Delete on the ones it created (the 0060 confirmation,
// `delete_ticket`). Also Close mid-turn's confirmation and Recently ended's list. Every text is Rust's
// (`project_agent_page.rs`). Project agents left the drawer for the pane (d0087, `pane.*`); T-0330 moves these parts
// to `screens/pane/`.
import { useRef, useState } from "react";

import { openOverlay } from "../../app/overlays";
import { openTicket } from "../../app/selection";
import { Button, ConfirmDialog, Popover, StatusPill } from "../../components";
import type { AgentCloseDialog } from "../../generated/AgentCloseDialog";
import type { AgentHeader } from "../../generated/AgentHeader";
import type { CreatedView } from "../../generated/CreatedView";
import type { RecentAgents } from "../../generated/RecentAgents";
import { Icon } from "../../theme/Icon";

export interface HeaderBarProps {
  header: AgentHeader;
  /** The name is being edited (Rename, or a double-click on it). */
  editing: boolean;
  onEdit: (editing: boolean) => void;
  onRename: (name: string) => void;
  onResume: () => void;
}

/** The Agent header above the terminal. */
export function HeaderBar({ header, editing, onEdit, onRename, onResume }: HeaderBarProps) {
  const [draft, setDraft] = useState(header.name);
  return (
    <div className="agent-header">
      {editing ? (
        <input
          className="agent-name-input"
          aria-label={header.rename}
          value={draft}
          // eslint-disable-next-line jsx-a11y/no-autofocus -- Rename puts the cursor in the name, as the frame shows
          autoFocus
          onChange={(e) => {
            setDraft(e.target.value);
          }}
          onBlur={() => {
            onEdit(false);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              onRename(draft);
              onEdit(false);
            } else if (e.key === "Escape") {
              e.stopPropagation();
              onEdit(false);
            }
          }}
        />
      ) : (
        <>
          <button
            type="button"
            className="agent-name"
            data-unnamed={header.unnamed || undefined}
            onDoubleClick={() => {
              setDraft(header.name);
              onEdit(true);
            }}
          >
            {header.name}
          </button>
          <button
            type="button"
            className="agent-rename"
            onClick={() => {
              setDraft(header.name);
              onEdit(true);
            }}
          >
            <Icon name="pencil" size={11} />
            <span>{header.rename}</span>
          </button>
        </>
      )}
      <StatusPill pill={header.pill} />
      <span className="agent-tokens">
        <Icon name="coins" size={11} />
        <span>{header.tokens}</span>
      </span>
      <span className="agent-scope">{header.scope}</span>
      {header.named_by_you !== null && <span className="agent-named">{header.named_by_you}</span>}
      <span className="drawer-spacer" />
      {header.resume !== null && (
        <Button
          compact
          variant={header.resume.primary ? "primary" : "secondary"}
          icon="play"
          label={header.resume.label}
          onClick={onResume}
        />
      )}
    </div>
  );
}

/** Created and changed beside the terminal. */
export function CreatedList({ created }: { created: CreatedView }) {
  return (
    <aside className="agent-created" aria-label={created.title}>
      <div className="agent-created-head">
        <span>{created.title}</span>
        {created.count !== null && <span>{created.count}</span>}
      </div>
      {created.rows.length === 0 ? (
        <p className="agent-created-empty">{created.empty}</p>
      ) : (
        <>
          {created.summary !== null && <p className="agent-created-summary">{created.summary}</p>}
          <ul className="agent-created-rows">
            {created.rows.map((row) => (
              <li key={`${row.key}-${String(row.edited)}`} data-edited={row.edited || undefined}>
                <Icon name={row.edited ? "pencil" : "square-plus"} size={12} />
                <button
                  type="button"
                  className="agent-created-key"
                  onClick={() => {
                    openTicket(row.key);
                  }}
                >
                  {row.key}
                </button>
                <span className="agent-created-text">{row.text}</span>
                {row.delete !== null && (
                  <button
                    type="button"
                    className="agent-created-delete"
                    onClick={() => {
                      openOverlay("delete_ticket", { ticket: row.key });
                    }}
                  >
                    <Icon name="trash-2" size={11} />
                    <span>{row.delete}</span>
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </aside>
  );
}

/** *Close an agent mid-turn · confirm*: the attention line, what stays, and Stop and close (Cancel the default). */
export function CloseAgentDialog({
  dialog,
  onConfirm,
  onCancel,
}: {
  dialog: AgentCloseDialog;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <ConfirmDialog
      title={dialog.title}
      width={480}
      destructive
      confirmLabel={dialog.confirm}
      confirmIcon="circle-stop"
      body={
        <>
          <p className="agent-close-turn">
            <Icon name="activity" size={13} />
            <span>{dialog.mid_turn}</span>
          </p>
          <p className="dialog-text">{dialog.text}</p>
        </>
      }
      onConfirm={onConfirm}
      onCancel={onCancel}
    />
  );
}

/** The Recently ended list (*Recently ended · list*), with entries or empty. */
export function RecentList({ recent, onResume }: { recent: RecentAgents; onResume: (id: string) => void }) {
  return (
    <div className="agent-recent">
      <span className="agent-recent-title">{recent.title}</span>
      {recent.rows.length === 0 ? (
        <p className="agent-recent-empty">{recent.empty}</p>
      ) : (
        recent.rows.map((row) => (
          <div key={row.id} className="agent-recent-row">
            <Icon name="bot" size={14} />
            <span className="agent-recent-text">
              <span className="agent-recent-name">{row.name}</span>
              <span className="agent-recent-when">{row.when}</span>
            </span>
            <Button
              compact
              variant="secondary"
              icon="play"
              label={row.resume}
              onClick={() => {
                onResume(row.id);
              }}
            />
          </div>
        ))
      )}
    </div>
  );
}

/** The header's Recently ended button and its popover. */
export function RecentButton({
  label,
  recent,
  onResume,
}: {
  label: string;
  recent: RecentAgents;
  onResume: (id: string) => void;
}) {
  const anchor = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        ref={anchor}
        type="button"
        className="drawer-recent"
        aria-haspopup="dialog"
        aria-expanded={open}
        data-open={open || undefined}
        onClick={() => {
          setOpen(!open);
        }}
      >
        <Icon name="history" size={13} />
        <span>{label}</span>
      </button>
      {open && (
        <Popover
          label={label}
          anchor={anchor}
          align="end"
          onClose={() => {
            setOpen(false);
          }}
        >
          <RecentList
            recent={recent}
            onResume={(id) => {
              setOpen(false);
              onResume(id);
            }}
          />
        </Popover>
      )}
    </>
  );
}
