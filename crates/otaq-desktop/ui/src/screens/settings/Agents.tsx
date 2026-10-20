// Settings → Agents (T-0410; AGM-02, AGM-03, AGM-04, AGT-05's marks; d0108, d0078, d0086, d0092; *15 → Agents
// (update AG §8)* `s15-58` / `s15-59`, the model picker `s15-30`, the mark `s15-32`): the default model for new agents, the fallback per tool and the list (NAME · MODEL · SKILLS · USED IN, each row's ⋯
// Duplicate / Delete…); and one agent's page in the same place (← name, its Model with the tool's effort, its Role, its
// Instructions in the Markdown editor, its Skills with Import skill folder… and Re-import, Used in). Everything comes
// from the views (`settings.agents`, `settings.agent` and the answers of the calls below); this keeps which page is
// open, the open menu and dialog, and the text being typed until it is saved.
import { type ReactNode, useCallback, useContext, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import {
  AnchoredMenu,
  Button,
  ConfirmDialog,
  IconButton,
  Segmented,
  Select,
  TextInput,
} from "../../components";
import type { AgentAddOutcome } from "../../generated/AgentAddOutcome";
import type { AgentChange } from "../../generated/AgentChange";
import type { AgentDeleteDialog } from "../../generated/AgentDeleteDialog";
import type { AgentDeleteOutcome } from "../../generated/AgentDeleteOutcome";
import type { AgentInstructionsField } from "../../generated/AgentInstructionsField";
import type { AgentListRow } from "../../generated/AgentListRow";
import type { AgentMarkView } from "../../generated/AgentMarkView";
import type { AgentOption } from "../../generated/AgentOption";
import type { AgentPage } from "../../generated/AgentPage";
import type { AgentPageParams } from "../../generated/AgentPageParams";
import type { AgentSkillDialog } from "../../generated/AgentSkillDialog";
import type { AgentSkillRow } from "../../generated/AgentSkillRow";
import type { AgentsSection as AgentsSectionView } from "../../generated/AgentsSection";
import type { MenuItem } from "../../generated/MenuItem";
import type { RequestError } from "../../generated/RequestError";
import {
  MarkdownSurface,
  MarkdownToolbar,
  openingMode,
  rememberMode,
  SourceArea,
  SourceToggle,
  useFocusOnLeave,
  useMarkdownEditor,
  useSourceMode,
} from "../../markdown";
import { asRequestError, watchView } from "../../service";
import { pickFolder, request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { useSelection } from "../../app/selection";
import { SettingsContext } from "./context";
import { proposeFrom } from "./flowActions";
import { ModelDropdown, type ModelPick, ModelPicker } from "./ModelPicker";

/** How long typed instructions wait before they are saved. */
export const INSTRUCTIONS_SAVE_MS = 800;

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

/** A small select's choices (the tool, the effort) as menu rows, the current one checked. */
export function optionItems(options: readonly AgentOption[]): MenuItem[] {
  return options.map((o) => ({
    id: o.id,
    label: o.label,
    icon: null,
    shortcut: null,
    danger: false,
    disabled: false,
    hint: null,
    separated: false,
    checked: o.selected,
    header: false,
  }));
}

/** The shipped skills Add skill offers, as menu rows. */
export function skillItems(names: readonly string[]): MenuItem[] {
  return names.map((name) => ({
    id: name,
    label: name,
    icon: null,
    shortcut: null,
    danger: false,
    disabled: false,
    hint: null,
    separated: false,
    checked: false,
    header: false,
  }));
}

/** What a picked option sends: its id, or none for the empty id (the default model, the tool's effort). */
export function optionValue(id: string): string | null {
  return id === "" ? null : id;
}

/** A row's ⋯ button and its menu (Duplicate, Delete…). */
function MoreMenu({
  label,
  items,
  onChoose,
}: {
  label: string;
  items: readonly MenuItem[];
  onChoose: (id: string) => void;
}) {
  const more = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <IconButton
        ref={more}
        icon="ellipsis"
        label={label}
        onClick={() => {
          setOpen((o) => !o);
        }}
      />
      {open && (
        <AnchoredMenu
          anchor={more}
          align="end"
          label={label}
          items={items}
          onChoose={(id) => {
            setOpen(false);
            onChoose(id);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") more.current?.focus();
          }}
        />
      )}
    </>
  );
}

/** What the page asks the section to do. */
export interface AgentsActions {
  onOpen: (params: AgentPageParams) => void;
  onMenu: (id: string, action: string) => void;
  onDefaultModel: (pick: ModelPick) => void;
  onFallback: (tool: string, pick: ModelPick) => void;
}

function Row({ row, actions }: { row: AgentListRow; actions: AgentsActions }) {
  const open = () => {
    actions.onOpen({ id: row.id, duplicate: null, draft: null, change: null });
  };
  return (
    <div className="settings-agents-row" role="row">
      <span className="settings-agents-name" role="cell">
        <button type="button" className="settings-agents-open" onClick={open}>
          <span className="settings-agents-title">
            <span>{row.name}</span>
            {row.role !== null && (
              <span className="settings-agents-role">
                <Icon name="circle-check" size={11} />
                {row.role}
              </span>
            )}
          </span>
          <span className="settings-agents-came">{row.came_from}</span>
        </button>
      </span>
      <span className="settings-agents-model" role="cell" data-marked={row.mark ? true : undefined}>
        {row.model}
      </span>
      <span className="settings-agents-skills" role="cell">
        {row.skills.map((skill) => (
          <code key={skill} className="settings-agents-skill">
            {skill}
          </code>
        ))}
      </span>
      <span className="settings-agents-use" role="cell">
        {row.used_in}
      </span>
      <span className="settings-agents-more" role="cell">
        <MoreMenu
          label={row.menu_label}
          items={row.menu}
          onChoose={(action) => {
            actions.onMenu(row.id, action);
          }}
        />
      </span>
      {row.mark && (
        <span className="settings-agents-mark">
          <MarkLine mark={row.mark} />
          {row.mark_link !== null && (
            <>
              <span className="settings-agents-dot">·</span>
              <button type="button" className="settings-link" onClick={open}>
                {row.mark_link}
              </button>
            </>
          )}
        </span>
      )}
    </div>
  );
}

/** The Agents page as the view says (`s15-58`): default model, fallbacks, the list. */
export function AgentsSectionBody({
  section,
  actions,
}: {
  section: AgentsSectionView;
  actions: AgentsActions;
}) {
  return (
    <section className="settings-section settings-agents" data-section="agents" aria-label={section.title}>
      <p className="settings-intro">{section.intro}</p>
      <div className="settings-agents-settings">
        <div className="settings-agents-setting">
          <span className="settings-row-text">
            <span className="settings-row-label">{section.default_model.label}</span>
            <span className="settings-row-description">{section.default_model.description}</span>
          </span>
          <span className="settings-agents-setting-control">
            <ModelDropdown
              label={section.default_model.label}
              dropdown={section.default_model.dropdown}
              onPick={actions.onDefaultModel}
            />
          </span>
        </div>
        {section.fallbacks && (
          <div className="settings-agents-setting">
            <span className="settings-row-text">
              <span className="settings-row-label">{section.fallbacks.label}</span>
              <span className="settings-row-description">{section.fallbacks.description}</span>
            </span>
            <span className="settings-agents-fallbacks">
              {section.fallbacks.lines.map((line) => (
                <span key={line.tool} className="settings-agents-fallback-line">
                  <span>{line.label}</span>
                  <ModelDropdown
                    label={line.label}
                    dropdown={line.dropdown}
                    onPick={(pick) => {
                      actions.onFallback(line.tool, pick);
                    }}
                  />
                </span>
              ))}
            </span>
          </div>
        )}
      </div>
      <div className="settings-agents-table" role="table" aria-label={section.title}>
        <div className="settings-agents-row settings-agents-head" role="row">
          {section.columns.map((column) => (
            <span key={column} role="columnheader">
              {column}
            </span>
          ))}
        </div>
        {section.rows.map((row) => (
          <Row key={row.id} row={row} actions={actions} />
        ))}
      </div>
    </section>
  );
}

/** What one agent's page asks for. */
export interface PageActions {
  onChange: (change: AgentChange) => void;
  onRole: (role: string) => void;
  onAdd: (name: string, instructions: string) => void;
  onCancel: () => void;
  onImport: () => void;
  onReimport: (skill: string) => void;
  onRetry: (tool: string) => void;
}

/** One skill: its name in mono, where an imported one came from, its warnings, Re-import (asks first) and Remove. */
function SkillLine({ row, actions }: { row: AgentSkillRow; actions: PageActions }) {
  const [asking, setAsking] = useState<AgentSkillDialog | null>(null);
  return (
    <div className="settings-agent-skill">
      <Icon name="folder" size={14} />
      <span className="settings-agent-skill-text">
        <span className="settings-agent-skill-line">
          <code className="settings-agent-skill-name">{row.name}</code>
          {row.source !== null && <span className="settings-agent-skill-source">{row.source}</span>}
        </span>
        {row.warnings.map((w) => (
          <span key={w} className="settings-agent-skill-warning">
            <Icon name="triangle-alert" size={12} />
            <span>{w}</span>
          </span>
        ))}
      </span>
      {row.reimport !== null && (
        <Button
          variant="secondary"
          compact
          icon="refresh-cw"
          label={row.reimport_label}
          onClick={() => {
            setAsking(row.reimport);
          }}
        />
      )}
      <IconButton
        icon="x"
        label={`${row.remove_label} ${row.name}`}
        onClick={() => {
          actions.onChange({ field: "remove_skill", value: row.key });
        }}
      />
      {asking && (
        <ConfirmDialog
          title={asking.title}
          body={asking.text}
          confirmLabel={asking.confirm}
          cancelLabel={asking.cancel}
          onConfirm={() => {
            setAsking(null);
            if (row.reimport_id !== null) actions.onReimport(row.reimport_id);
          }}
          onCancel={() => {
            setAsking(null);
          }}
        />
      )}
    </div>
  );
}

/** One agent's page (`s15-58` "One agent", "Giving a role to another agent"). Name and instructions are typed here and
 * saved when the name field is left, and a moment after the instructions stop changing; everything else is saved at
 * once (a draft keeps them until Add agent). */
export function AgentPageBody({ page, actions }: { page: AgentPage; actions: PageActions }) {
  const [name, setName] = useState(page.name.value);
  const [instructions, setInstructions] = useState(page.instructions.value);
  // A fresh page with another name (saved, or a refused one sent back) replaces what is typed.
  const [shownName, setShownName] = useState(page.name.value);
  if (shownName !== page.name.value) {
    setShownName(page.name.value);
    setName(page.name.value);
  }
  const pending = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saved = page.id !== null;
  useEffect(
    () => () => {
      if (pending.current !== null) clearTimeout(pending.current);
    },
    [],
  );
  const model = page.model;
  const role = page.role;
  const roleIndex = role
    ? Math.max(
        0,
        role.options.findIndex((o) => o.selected),
      )
    : 0;
  return (
    <section className="settings-section settings-agent-page" data-section="agent" aria-label={page.title}>
      {page.mark && (
        <div className="settings-agent-mark">
          <MarkLine mark={page.mark} />
          {page.mark.message !== null && (
            <span className="settings-agent-mark-message">“{page.mark.message}”</span>
          )}
        </div>
      )}
      <label className="settings-agent-label" htmlFor="agent-name">
        {page.name.label}
      </label>
      <TextInput
        id="agent-name"
        value={name}
        invalid={page.name.problem !== null}
        onChange={setName}
        onBlur={() => {
          if (saved && name !== page.name.value) actions.onChange({ field: "name", value: name });
        }}
      />
      {page.name.problem !== null && <span className="settings-agent-problem">{page.name.problem}</span>}

      <span className="settings-agent-label">{model.label}</span>
      <span className="settings-agent-model">
        <Select
          label={model.tool_label}
          value={model.tool_value}
          items={optionItems(model.tools)}
          onChange={(id) => {
            actions.onChange({ field: "tool", value: optionValue(id) });
          }}
        />
        {model.picker && (
          <ModelPicker
            label={model.label}
            picker={model.picker}
            onPick={(picked) => {
              actions.onChange({ field: "model", value: picked });
            }}
            onRetry={actions.onRetry}
          />
        )}
        {model.default_now !== null && (
          <span className="settings-agent-default-now">{model.default_now}</span>
        )}
        {model.effort && (
          <span className="settings-agent-effort">
            <span>{model.effort.label}</span>
            <Select
              label={model.effort.label}
              value={model.effort.value}
              items={optionItems(model.effort.options)}
              onChange={(id) => {
                actions.onChange({ field: "effort", value: optionValue(id) });
              }}
            />
          </span>
        )}
      </span>
      {model.problem !== null ? (
        <span className="settings-agent-problem">{model.problem}</span>
      ) : (
        model.picker?.status === null &&
        model.picker.value.no_longer_listed !== null && (
          <span className="settings-agent-hint">{model.picker.hint}</span>
        )
      )}

      {role && (
        <>
          <span className="settings-agent-label">{role.label}</span>
          <Segmented
            label={role.label}
            options={role.options.map((o) => o.label)}
            selected={roleIndex}
            onSelect={(i) => {
              const picked = role.options[i];
              if (picked && !picked.selected) actions.onRole(picked.id);
            }}
          />
          <span className="settings-agent-hint">{role.hint}</span>
          {role.problem !== null && <span className="settings-agent-problem">{role.problem}</span>}
          {role.moved !== null && (
            <span className="settings-agent-moved" role="status">
              <Icon name="circle-check" size={14} />
              <span>{role.moved}</span>
            </span>
          )}
        </>
      )}

      <span className="settings-agent-label">{page.instructions.label}</span>
      <Instructions
        field={page.instructions}
        value={instructions}
        onChange={(text) => {
          setInstructions(text);
          if (!saved) return;
          if (pending.current !== null) clearTimeout(pending.current);
          pending.current = setTimeout(() => {
            pending.current = null;
            actions.onChange({ field: "instructions", value: text });
          }, INSTRUCTIONS_SAVE_MS);
        }}
      />

      <span className="settings-agent-label">{page.skills.label}</span>
      <div className="settings-agent-skills">
        {page.skills.rows.map((row) => (
          <SkillLine key={row.key} row={row} actions={actions} />
        ))}
      </div>
      {page.skills.problem !== null && <span className="settings-agent-problem">{page.skills.problem}</span>}
      <span className="settings-agent-skill-buttons">
        {page.skills.import_label !== null && (
          <Button
            variant="secondary"
            icon="folder-input"
            label={page.skills.import_label}
            onClick={actions.onImport}
          />
        )}
        {page.skills.add_options.length > 0 && (
          <AddSkill
            label={page.skills.add_label}
            options={page.skills.add_options}
            onAdd={(skill) => {
              actions.onChange({ field: "add_skill", value: skill });
            }}
          />
        )}
      </span>

      {page.used_in && (
        <>
          <span className="settings-agent-label">{page.used_in.label}</span>
          <span className="settings-agent-used">
            {page.used_in.items.map((item) => (
              <code key={item} className="settings-agent-place">
                {item}
              </code>
            ))}
            {page.used_in.empty !== null && <span>{page.used_in.empty}</span>}
          </span>
        </>
      )}
      {page.note !== null && (
        <p className="settings-agent-note">
          <Icon name="info" size={14} />
          <span>{page.note}</span>
        </p>
      )}
      {page.problem !== null && <span className="settings-agent-problem">{page.problem}</span>}
      {page.add && (
        <span className="settings-agent-add">
          <Button variant="ghost" label={page.add.cancel_label} onClick={actions.onCancel} />
          <Button
            variant="primary"
            label={page.add.add_label}
            onClick={() => {
              actions.onAdd(name, instructions);
            }}
          />
        </span>
      )}
    </section>
  );
}

/** Instructions in the Markdown editor (d0078), with its toolbar and its Markdown source switch (D9), which opens in the
 * mode last chosen and is remembered for every editor. */
function Instructions({
  field,
  value,
  onChange,
}: {
  field: AgentInstructionsField;
  value: string;
  onChange: (text: string) => void;
}) {
  const mode = useSourceMode(value, openingMode(false));
  const editor = useMarkdownEditor({
    value: mode.editorValue,
    onChange,
    placeholder: field.placeholder,
    label: field.label,
  });
  useFocusOnLeave(editor, mode);
  const toggle = (on: boolean) => {
    mode.toggle();
    rememberMode(on);
    request("editor.source_mode", { on }).catch(() => undefined);
  };
  return (
    <MarkdownSurface
      editor={editor}
      className="settings-agent-instructions"
      toolbar={
        <MarkdownToolbar
          disabled={mode.on}
          expand={<></>}
          sourceToggle={<SourceToggle on={mode.on} label={field.markdown_label} onChange={toggle} />}
        />
      }
      {...(mode.on
        ? {
            source: (
              <SourceArea value={value} label={field.label} takeFocus={mode.switched} onChange={onChange} />
            ),
          }
        : {})}
    />
  );
}

/** Add skill: a button that opens the shipped skills the agent doesn't have. */
function AddSkill({
  label,
  options,
  onAdd,
}: {
  label: string;
  options: readonly string[];
  onAdd: (skill: string) => void;
}) {
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        ref={button}
        variant="secondary"
        icon="plus"
        label={label}
        onClick={() => {
          setOpen((o) => !o);
        }}
      />
      {open && (
        <AnchoredMenu
          anchor={button}
          align="start"
          label={label}
          items={skillItems(options)}
          onChoose={(id) => {
            setOpen(false);
            onAdd(id);
          }}
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

/** One agent's heading in the Settings header: ← name, its role tag, where it came from. */
function AgentHeading({ page, onBack }: { page: AgentPage; onBack: () => void }) {
  return (
    <span className="settings-agent-heading">
      <span className="settings-agent-heading-line">
        <IconButton icon="arrow-left" label={page.back_label} onClick={onBack} />
        <h2>{page.title}</h2>
        {page.role_tag !== null && (
          <span className="settings-agents-role">
            <Icon name="circle-check" size={11} />
            {page.role_tag}
          </span>
        )}
      </span>
      {page.came_from !== null && <span className="settings-agents-came">{page.came_from}</span>}
    </span>
  );
}

type Asking = { kind: "delete"; dialog: AgentDeleteDialog } | null;

/** The global page's Agents topic: its page, or one agent's. It shows Otaq's copy of the model lists at once, then
 * asks the tools for their lists again (once per opening, d0086) and redraws with them. */
export function AgentsSection() {
  // The project on screen: the shipped agents file's preview is its (FLW-10).
  const screenProject = useSelection().project;
  const settings = useContext(SettingsContext);
  // `undefined` outside Settings (the gallery, tests): the header's buttons are drawn above the page instead.
  const headerSlot = settings?.headerSlot;
  const setHeading = settings?.setHeading;
  const [section, setSection] = useState<AgentsSectionView | null>(null);
  const [page, setPage] = useState<AgentPage | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  const [asking, setAsking] = useState<Asking>(null);
  const failed = (e: unknown) => {
    setError(asRequestError(e));
  };

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

  // Opened on one agent (Open agent… on a card): its page, once.
  const openedOn = useRef(false);
  const startAgent = settings?.agent;
  useEffect(() => {
    if (startAgent === undefined || openedOn.current) return;
    openedOn.current = true;
    request("settings.agent", { id: startAgent, duplicate: null, draft: null, change: null }).then(
      (fresh) => {
        setPage(fresh as AgentPage);
      },
      (e: unknown) => {
        setError(asRequestError(e));
      },
    );
  }, [startAgent]);

  const back = useCallback(() => {
    setPage(null);
    setError(null);
  }, []);
  // A nav click goes back to the list.
  const visit = settings?.visit;
  const [seenVisit, setSeenVisit] = useState(visit);
  if (seenVisit !== visit) {
    setSeenVisit(visit);
    setPage(null);
  }
  // The header shows the open agent, and the topic's label again without one.
  useEffect(() => {
    setHeading?.(page ? <AgentHeading page={page} onBack={back} /> : null);
  }, [page, setHeading, back]);
  useEffect(() => () => setHeading?.(null), [setHeading]);

  const show = (p: Promise<unknown>) => {
    p.then((fresh) => {
      setPage(fresh as AgentPage);
      setError(null);
    }, failed);
  };
  const open = (params: AgentPageParams) => {
    show(request("settings.agent", params));
  };
  const listed = (p: Promise<unknown>) => {
    p.then((fresh) => {
      setSection(fresh as AgentsSectionView);
      setError(null);
    }, failed);
  };
  const askDelete = (id: string) => {
    request("settings.agent_delete_dialog", { id }).then((d) => {
      setAsking({ kind: "delete", dialog: d as AgentDeleteDialog });
    }, failed);
  };
  const menu = (id: string, action: string) => {
    if (action === "duplicate") open({ id: null, duplicate: id, draft: null, change: null });
    else if (action === "delete") askDelete(id);
  };

  const actions: AgentsActions = {
    onOpen: open,
    onMenu: menu,
    onDefaultModel: (pick) => {
      if (pick) listed(request("settings.agents_default_model", pick));
    },
    onFallback: (tool, fallback) => {
      listed(request("settings.agents_fallback", { tool, fallback }));
    },
  };

  const pageActions = (p: AgentPage): PageActions => ({
    onChange: (change) => {
      show(
        request("settings.agent", {
          id: p.id,
          duplicate: null,
          draft: p.id === null ? p.draft : null,
          change,
        }),
      );
    },
    onRole: (role) => {
      if (p.id !== null) show(request("settings.agent_role", { id: p.id, role }));
    },
    onAdd: (name, instructions) => {
      request("settings.agent_add", { draft: { ...p.draft, name, instructions } }).then((outcome) => {
        setPage((outcome as AgentAddOutcome).page);
        setError(null);
      }, failed);
    },
    onCancel: back,
    onImport: () => {
      const id = p.id;
      if (id === null) return;
      void pickFolder(p.skills.picker_title).then((folder) => {
        if (folder !== null) show(request("settings.agent_import", { id, folder }));
      }, failed);
    },
    onReimport: (skill) => {
      if (p.id !== null) show(request("settings.agent_reimport", { id: p.id, skill }));
    },
    onRetry: (tool) => {
      request("settings.agents_models_retry", { tool }).then(() => {
        open({ id: p.id, duplicate: null, draft: p.id === null ? p.draft : null, change: null });
      }, failed);
    },
  });

  let header: ReactNode = null;
  if (page) {
    if (page.menu.length > 0 && page.id !== null) {
      const id = page.id;
      header = (
        <MoreMenu
          label={page.menu_label}
          items={page.menu}
          onChoose={(a) => {
            menu(id, a);
          }}
        />
      );
    }
  } else if (section) {
    header = (
      <>
        <Button
          variant="secondary"
          icon="plus"
          label={section.new_agent_label}
          onClick={() => {
            open({ id: null, duplicate: null, draft: null, change: null });
          }}
        />
        {/* Apply the shipped agents file again (FLW-10): the preview, for the project on screen. */}
        <MoreMenu
          label={section.more_label}
          items={section.more.map((m) =>
            m.id === "agents_file" && screenProject === null ? { ...m, disabled: true } : m,
          )}
          onChoose={(id) => {
            if (id === "agents_file" && screenProject !== null)
              proposeFrom(screenProject, { from: "agents_file" }).catch(failed);
          }}
        />
      </>
    );
  }

  return (
    <>
      {headerSlot && header ? createPortal(header, headerSlot) : null}
      {headerSlot === undefined && header && <div className="settings-agents-header">{header}</div>}
      {error && (
        <p className="settings-problem" role="alert">
          {error.message}
        </p>
      )}
      {page ? (
        <AgentPageBody key={page.id ?? "draft"} page={page} actions={pageActions(page)} />
      ) : (
        section && <AgentsSectionBody section={section} actions={actions} />
      )}
      {asking?.kind === "delete" &&
        (asking.dialog.kind === "refused" ? (
          <ConfirmDialog
            title={asking.dialog.title}
            body={asking.dialog.text}
            confirmLabel={null}
            cancelLabel={asking.dialog.ok_label}
            hint="close"
            onConfirm={() => {
              setAsking(null);
            }}
            onCancel={() => {
              setAsking(null);
            }}
          />
        ) : (
          <ConfirmDialog
            title={asking.dialog.title}
            body={asking.dialog.text}
            confirmLabel={asking.dialog.delete_label}
            cancelLabel={asking.dialog.cancel_label}
            destructive
            onConfirm={() => {
              if (asking.dialog.kind !== "confirm") return;
              const id = asking.dialog.id;
              setAsking(null);
              request("settings.agent_delete", { id }).then((o) => {
                const outcome = o as AgentDeleteOutcome;
                if (outcome.outcome === "deleted") {
                  setSection(outcome.section);
                  setPage(null);
                } else setAsking({ kind: "delete", dialog: outcome.dialog });
              }, failed);
            }}
            onCancel={() => {
              setAsking(null);
            }}
          />
        ))}
    </>
  );
}
