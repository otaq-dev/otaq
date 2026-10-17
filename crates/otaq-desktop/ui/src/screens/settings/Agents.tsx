// Settings → Agents (T-0343; AGT-01…AGT-03, AGT-05's mark; d0086, d0077; *15 · Settings* `bVdGw`, `FYvzM`, `cnRA5`,
// `XlyOa`, `Z4gQQQ` in their one-tool form): the Named agents table (NAME · MODEL · USE FOR, no tool column), each
// row's ⋯ menu (Edit…, Duplicate, Delete…), the amber mark (`st-paused`), the fresh-install note while one entry
// exists, Add agent, and the four role defaults; and the project's Agents block (`mQijC`), collapsed at "Uses app
// defaults · Change" until the project overrides a role. Everything comes from T-0340's views (`settings.agents`,
// `settings.project_agents` and the answers of the calls below); this keeps the open menu and dialog.
import { useCallback, useEffect, useRef, useState } from "react";

import { closeOverlay } from "../../app/overlays";
import { AnchoredMenu, Button, IconButton } from "../../components";
import type { AgentDeleteDialog } from "../../generated/AgentDeleteDialog";
import type { AgentDeleteOutcome } from "../../generated/AgentDeleteOutcome";
import type { AgentDialog as AgentDialogView } from "../../generated/AgentDialog";
import type { AgentDraft } from "../../generated/AgentDraft";
import type { AgentMenuAction } from "../../generated/AgentMenuAction";
import type { AgentMenuItem } from "../../generated/AgentMenuItem";
import type { AgentRole } from "../../generated/AgentRole";
import type { AgentSaveOutcome } from "../../generated/AgentSaveOutcome";
import type { AgentsSection as AgentsSectionView } from "../../generated/AgentsSection";
import type { AgentTableRow } from "../../generated/AgentTableRow";
import type { MenuItem } from "../../generated/MenuItem";
import type { ProjectAgentsBlock } from "../../generated/ProjectAgentsBlock";
import type { RequestError } from "../../generated/RequestError";
import { asRequestError, watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { openKey } from "../sections/Report";
import { AgentDeleteDialogView, AgentDialog, MarkLine } from "./AgentDialog";
import { AgentPickerField } from "./ModelPicker";

/** A row's ⋯ items as menu rows; the last agent's Delete… is disabled with its reason under it. */
export function menuItems(items: readonly AgentMenuItem[]): MenuItem[] {
  return items.map((item) => ({
    id: item.action,
    label: item.label,
    icon: item.icon,
    shortcut: null,
    danger: item.danger,
    disabled: !item.enabled,
    hint: item.tooltip,
    separated: item.separated,
    checked: false,
    header: false,
  }));
}

/** What the section asks the page to do. */
export interface AgentsActions {
  onAdd: () => void;
  onMenu: (row: AgentTableRow, action: AgentMenuAction) => void;
  onDefault: (role: AgentRole, agent: string) => void;
}

function TableRow({ row, onMenu }: { row: AgentTableRow; onMenu: AgentsActions["onMenu"] }) {
  const more = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <div className="settings-agents-row" role="row" data-open={open || undefined}>
      <span className="settings-agents-name" role="cell">
        <span>{row.name}</span>
        {row.mark && (
          <Icon
            name="triangle-alert"
            size={12}
            label={row.mark.before + (row.mark.model ?? "") + row.mark.after}
          />
        )}
      </span>
      <span
        className="settings-agents-model"
        role="cell"
        data-mono={row.model_mono || undefined}
        data-marked={row.mark ? true : undefined}
      >
        {row.model}
      </span>
      <span className="settings-agents-use" role="cell">
        {row.use_for}
      </span>
      <span className="settings-agents-more" role="cell">
        <IconButton
          ref={more}
          icon="ellipsis"
          label={row.menu_label}
          onClick={() => {
            setOpen((o) => !o);
          }}
        />
      </span>
      {row.mark && (
        <span className="settings-agents-mark">
          <MarkLine mark={row.mark} />
          <span className="settings-agents-dot">·</span>
          <button
            type="button"
            className="settings-link"
            onClick={() => {
              onMenu(row, "edit");
            }}
          >
            {row.mark.edit_label}
          </button>
        </span>
      )}
      {open && (
        <AnchoredMenu
          anchor={more}
          align="end"
          label={row.menu_label}
          items={menuItems(row.menu)}
          width={180}
          onChoose={(id) => {
            onMenu(row, id as AgentMenuAction);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") more.current?.focus();
          }}
        />
      )}
    </div>
  );
}

/** The whole section as the view says, with the page's calls. */
export function AgentsSectionBody({
  section,
  onAdd,
  onMenu,
  onDefault,
}: { section: AgentsSectionView } & AgentsActions) {
  return (
    <section className="settings-section settings-agents" data-section="agents" aria-label={section.title}>
      <h3 className="settings-section-title">{section.title}</h3>
      <p className="settings-intro">{section.intro}</p>
      <h4 className="settings-agents-heading">{section.table_title}</h4>
      <div className="settings-agents-table" role="table" aria-label={section.table_title}>
        <div className="settings-agents-row settings-agents-head" role="row">
          {section.columns.map((column) => (
            <span key={column} role="columnheader">
              {column}
            </span>
          ))}
          <span role="columnheader" />
        </div>
        {section.rows.map((row) => (
          <TableRow key={row.id} row={row} onMenu={onMenu} />
        ))}
      </div>
      <span className="settings-agents-add">
        <Button variant="secondary" icon="plus" label={section.add_label} onClick={onAdd} />
      </span>
      {section.fresh_note && (
        <div className="settings-agents-note" role="note">
          <Icon name="lightbulb" size={14} />
          <span className="settings-agents-note-text">
            <strong>{section.fresh_note.title}</strong>
            <span>{section.fresh_note.text}</span>
          </span>
          <Button variant="secondary" compact label={section.fresh_note.add_label} onClick={onAdd} />
        </div>
      )}
      <h4 className="settings-agents-heading" data-ruled>
        {section.defaults_title}
      </h4>
      {section.defaults.map((row) => (
        <div key={row.role} className="settings-row settings-agents-default">
          <div className="settings-row-text">
            <span className="settings-row-label">{row.label}</span>
            <span className="settings-row-description">{row.description}</span>
          </div>
          <div className="settings-agents-picker">
            <AgentPickerField
              label={row.label}
              picker={row.picker}
              onPick={(agent) => {
                if (agent !== null) onDefault(row.role, agent);
              }}
            />
          </div>
        </div>
      ))}
      <p className="settings-agents-defaults-note">{section.defaults_note}</p>
    </section>
  );
}

type Open =
  | { kind: "edit"; dialog: AgentDialogView; key: number }
  | { kind: "delete"; dialog: AgentDeleteDialog }
  | null;

/** A key of Show tickets opens its ticket's panel; Settings closes so it can be seen. */
function openTicket(key: string): void {
  closeOverlay("settings");
  openKey(key);
}

/** The dialogs of the section, and the calls behind them; `onSection` takes a fresh section from a save. */
export function useAgentDialogs(
  onSection: (section: AgentsSectionView) => void,
  onError: (e: RequestError) => void,
) {
  const [open, setOpen] = useState<Open>(null);
  // The newest live check wins: an older answer arriving late is dropped.
  const asked = useRef(0);
  const opened = useRef(0);
  const fail = useCallback(
    (e: unknown) => {
      onError(asRequestError(e));
    },
    [onError],
  );
  const showDialog = useCallback(
    (params: { id?: string; duplicate?: string }) => {
      request("settings.agent_dialog", params).then((d) => {
        opened.current += 1;
        setOpen({ kind: "edit", dialog: d as AgentDialogView, key: opened.current });
      }, fail);
    },
    [fail],
  );
  const askDelete = useCallback(
    (id: string) => {
      request("settings.agent_delete_dialog", { id }).then((d) => {
        setOpen({ kind: "delete", dialog: d as AgentDeleteDialog });
      }, fail);
    },
    [fail],
  );
  const onMenu = useCallback(
    (row: AgentTableRow, action: AgentMenuAction) => {
      switch (action) {
        case "edit":
          showDialog({ id: row.id });
          return;
        case "duplicate":
          showDialog({ duplicate: row.id });
          return;
        case "delete":
          askDelete(row.id);
          return;
      }
    },
    [showDialog, askDelete],
  );
  const close = () => {
    setOpen(null);
  };
  let dialog = null;
  if (open?.kind === "edit") {
    const id = open.dialog.id;
    dialog = (
      <AgentDialog
        key={open.key}
        dialog={open.dialog}
        onDraft={(draft: AgentDraft) => {
          const mine = ++asked.current;
          request("settings.agent_dialog", { id, draft }).then((d) => {
            if (mine !== asked.current) return;
            setOpen((o) =>
              o?.kind === "edit" && o.key === open.key ? { ...o, dialog: d as AgentDialogView } : o,
            );
          }, fail);
        }}
        onSave={(draft) => {
          asked.current += 1;
          request("settings.agent_save", { id, draft }).then((r) => {
            const outcome = r as AgentSaveOutcome;
            if (outcome.outcome === "saved") {
              onSection(outcome.section);
              setOpen(null);
            } else {
              setOpen((o) => (o?.kind === "edit" ? { ...o, dialog: outcome.dialog } : o));
            }
          }, fail);
        }}
        onCancel={close}
        onDelete={askDelete}
      />
    );
  } else if (open?.kind === "delete") {
    dialog = (
      <AgentDeleteDialogView
        dialog={open.dialog}
        onClose={close}
        onOpenTicket={openTicket}
        onDelete={(id) => {
          request("settings.agent_delete", { id }).then((r) => {
            const outcome = r as AgentDeleteOutcome;
            if (outcome.outcome === "deleted") {
              onSection(outcome.section);
              setOpen(null);
            } else {
              setOpen({ kind: "delete", dialog: outcome.dialog });
            }
          }, fail);
        }}
      />
    );
  }
  return {
    dialog,
    onAdd: () => {
      showDialog({});
    },
    onMenu,
  };
}

/** The global page's Agents section. It shows Otaq's copy of the model list at once, then asks the tool for its
 * list again (once per opening, d0086) and redraws with it. */
export function AgentsSection() {
  const [section, setSection] = useState<AgentsSectionView | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  useEffect(
    () =>
      watchView(
        () => request("settings.agents", { refresh: false }) as Promise<AgentsSectionView>,
        ["agents_changed"],
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
  const { dialog, onAdd, onMenu } = useAgentDialogs(setSection, setError);
  const onDefault = (role: AgentRole, agent: string) => {
    request("settings.agent_default", { role, agent }).then(
      (s) => {
        setSection(s as AgentsSectionView);
      },
      (e: unknown) => {
        setError(asRequestError(e));
      },
    );
  };
  return (
    <>
      {error && (
        <p className="settings-problem" role="alert">
          {error.message}
        </p>
      )}
      {section && <AgentsSectionBody section={section} onAdd={onAdd} onMenu={onMenu} onDefault={onDefault} />}
      {dialog}
    </>
  );
}

/** The project's block as the view says: collapsed until Change (or an override) opens it. */
export function ProjectAgentsBody({
  block,
  onPick,
}: {
  block: ProjectAgentsBlock;
  onPick: (role: AgentRole, agent: string | null) => void;
}) {
  const [changing, setChanging] = useState(false);
  const expanded = block.expanded || changing;
  return (
    <section className="settings-section settings-agents" data-section="agents" aria-label={block.title}>
      <h3 className="settings-section-title">{block.title}</h3>
      {!expanded ? (
        <div className="settings-row settings-agents-default">
          <div className="settings-row-text">
            <span className="settings-row-label">{block.collapsed_title}</span>
          </div>
          <span className="settings-agents-collapsed">
            <span>{block.collapsed_text}</span>
            <button
              type="button"
              className="settings-link"
              onClick={() => {
                setChanging(true);
              }}
            >
              {block.change_label}
            </button>
          </span>
        </div>
      ) : (
        block.rows.map((row) => (
          <div key={row.role} className="settings-row settings-agents-default">
            <div className="settings-row-text">
              <span className="settings-row-label">{row.label}</span>
            </div>
            <div className="settings-agents-picker">
              <AgentPickerField
                label={row.label}
                picker={row.picker}
                onPick={(agent) => {
                  onPick(row.role, agent);
                }}
              />
            </div>
          </div>
        ))
      )}
    </section>
  );
}

/** A project page's Agents section (AGT-03). */
export function ProjectAgents({ project }: { project: string }) {
  const [block, setBlock] = useState<ProjectAgentsBlock | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  useEffect(
    () =>
      watchView(
        () => request("settings.project_agents", { project }) as Promise<ProjectAgentsBlock>,
        ["agents_changed", "projects_changed"],
        (state) => {
          if (state.view) setBlock(state.view);
          setError(state.error ?? null);
        },
      ),
    [project],
  );
  const onPick = (role: AgentRole, agent: string | null) => {
    request("settings.project_agent_default", { project, role, agent }).then(
      (b) => {
        setBlock(b as ProjectAgentsBlock);
      },
      (e: unknown) => {
        setError(asRequestError(e));
      },
    );
  };
  return (
    <>
      {error && (
        <p className="settings-problem" role="alert">
          {error.message}
        </p>
      )}
      {block && <ProjectAgentsBody key={block.project} block={block} onPick={onPick} />}
    </>
  );
}
