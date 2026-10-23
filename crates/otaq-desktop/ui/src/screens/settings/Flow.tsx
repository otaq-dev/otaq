// Settings → Flow, a project's section (T-0422; FLW-06…FLW-11; *15 → Row · The project's Flow section*, *Row · Use a
// preset*): who changed the flow and when, the waiting change with Cancel change, the read-only stage table, and the
// actions. Everything it says comes from Rust (`settings.flow`, `settings.flow_presets`, `settings.flow_copy`); what
// the buttons do is in `flow.ts`.
import { useCallback, useEffect, useRef, useState } from "react";

import { AnchoredMenu, Button, Dialog, IconButton } from "../../components";
import type { FlowCopyMenu } from "../../generated/FlowCopyMenu";
import type { FlowSettings } from "../../generated/FlowSettings";
import type { FlowSettingsStage } from "../../generated/FlowSettingsStage";
import type { MenuItem } from "../../generated/MenuItem";
import type { PresetCard } from "../../generated/PresetCard";
import type { PresetDialog as PresetDialogView } from "../../generated/PresetDialog";
import type { RequestError } from "../../generated/RequestError";
import { asRequestError, watchView } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { changeFlow, menuChoice, proposeFrom } from "./flowActions";

/** The service events after which the section is fetched again: the flow, its waiting change, the agents. */
const REFETCH_ON = ["flow_changed", "agents_changed", "projects_changed", "ticket_changed"];

function StageRow({ row }: { row: FlowSettingsStage }) {
  return (
    <div className="settings-flow-row" role="row">
      <span className="settings-flow-stage" role="cell">
        <span className="settings-flow-icon">
          <Icon name={row.icon} size={13} />
        </span>
        <span className="settings-flow-cell">
          <span className="settings-flow-name">{row.name}</span>
          <span className="settings-flow-kind">{row.kind}</span>
        </span>
      </span>
      <span className="settings-flow-cell" role="cell">
        <span>{row.agent}</span>
        {row.model !== null && <span className="settings-flow-model">{row.model}</span>}
      </span>
      <span role="cell">{row.options}</span>
    </div>
  );
}

/** One preset: its name and how it delivers, its line, the small diagram of its columns, its file. */
function Preset({ card, onPick }: { card: PresetCard; onPick: () => void }) {
  return (
    <button type="button" className="settings-preset" onClick={onPick}>
      <span className="settings-preset-head">
        <span className="settings-preset-name">{card.name}</span>
        <span className="settings-preset-delivers">{card.delivers}</span>
      </span>
      <span className="settings-preset-line">{card.line}</span>
      <span className="settings-preset-columns">
        {card.columns.map((column, i) => (
          <span key={`${column.label}-${String(i)}`} className="settings-preset-column">
            <Icon name={column.icon} size={12} />
            <span>{column.label}</span>
          </span>
        ))}
      </span>
      <span className="settings-preset-file">
        <Icon name="file-text" size={12} />
        {card.file}
      </span>
    </button>
  );
}

/** Use a preset… (*Row · Use a preset*): picking one opens the flow editor on it. */
export function PresetDialog({
  dialog,
  onPick,
  onCancel,
}: {
  dialog: PresetDialogView;
  onPick: (file: string) => void;
  onCancel: () => void;
}) {
  return (
    <Dialog
      title={dialog.title}
      width={720}
      onCancel={onCancel}
      footer={<Button variant="ghost" label={dialog.cancel} onClick={onCancel} />}
    >
      <div className="settings-presets">
        {dialog.presets.map((card) => (
          <Preset
            key={card.file}
            card={card}
            onPick={() => {
              onPick(card.file);
            }}
          />
        ))}
      </div>
    </Dialog>
  );
}

/** The copy menu's items: one per other project. */
function copyItems(menu: FlowCopyMenu): MenuItem[] {
  return [{ ...item("", menu.title), header: true }, ...menu.projects.map((p) => item(p.key, p.name))];
}

function item(id: string, label: string): MenuItem {
  return {
    id,
    label,
    icon: null,
    shortcut: null,
    danger: false,
    disabled: false,
    hint: null,
    separated: false,
    checked: false,
    header: false,
  };
}

/** The section as the view says. */
export function FlowSectionBody({
  section,
  onCancelChange,
  onChange,
  onPreset,
  onCopy,
  onMenu,
}: {
  section: FlowSettings;
  onCancelChange: () => void;
  onChange: () => void;
  onPreset: () => void;
  onCopy: (project: string) => void;
  onMenu: (id: string) => void;
}) {
  const more = useRef<HTMLButtonElement>(null);
  const copyButton = useRef<HTMLButtonElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [copy, setCopy] = useState<FlowCopyMenu | null>(null);
  return (
    <section className="settings-section settings-flow" data-section="flow" aria-label={section.title}>
      <div className="settings-flow-title">
        <span className="settings-row-label">{section.title}</span>
        <span className="settings-flow-changed">{section.changed}</span>
      </div>
      <p className="settings-row-description">{section.intro}</p>
      {section.waiting !== null && (
        <div className="settings-flow-waiting" role="status">
          <Icon name="triangle-alert" size={14} />
          <span className="settings-flow-waiting-text">{section.waiting.text}</span>
          <Button compact variant="secondary" label={section.waiting.cancel_label} onClick={onCancelChange} />
        </div>
      )}
      {section.problem !== null ? (
        <p className="settings-problem" role="alert">
          {section.problem}
        </p>
      ) : (
        <div className="settings-flow-table" role="table" aria-label={section.title}>
          <div className="settings-flow-row settings-flow-head" role="row">
            {section.columns.map((column) => (
              <span key={column} role="columnheader">
                {column}
              </span>
            ))}
          </div>
          <div className="settings-flow-ends" role="row">
            <Icon name="circle-dashed" size={12} />
            <span role="cell">{section.ends}</span>
          </div>
          {section.rows.map((row) => (
            <StageRow key={`${row.name}-${row.kind}`} row={row} />
          ))}
        </div>
      )}
      <div className="settings-flow-actions">
        <Button
          label={section.change.label}
          icon={section.change.icon ?? undefined}
          disabled={section.change.disabled}
          onClick={onChange}
        />
        <Button variant="secondary" label={section.preset.label} onClick={onPreset} />
        <span className="settings-flow-copy" title={section.copy.hint ?? undefined}>
          <Button
            ref={copyButton}
            variant="secondary"
            label={section.copy.label}
            disabled={section.copy.disabled}
            onClick={() => {
              request("settings.flow_copy", { project: section.project }).then(
                (menu) => {
                  setCopy(menu as FlowCopyMenu);
                },
                () => undefined,
              );
            }}
          />
        </span>
        <IconButton
          ref={more}
          icon="ellipsis"
          label={section.menu_label}
          onClick={() => {
            setMenuOpen((o) => !o);
          }}
        />
        {copy !== null && (
          <AnchoredMenu
            anchor={copyButton}
            align="start"
            label={section.copy.label}
            items={copyItems(copy)}
            onChoose={(key) => {
              setCopy(null);
              onCopy(key);
            }}
            onClose={(how) => {
              setCopy(null);
              if (how === "key") copyButton.current?.focus();
            }}
          />
        )}
        {menuOpen && (
          <AnchoredMenu
            anchor={more}
            align="start"
            label={section.menu_label}
            items={section.menu}
            onChoose={(id) => {
              setMenuOpen(false);
              onMenu(id);
            }}
            onClose={(how) => {
              setMenuOpen(false);
              if (how === "key") more.current?.focus();
            }}
          />
        )}
      </div>
    </section>
  );
}

/** The registered section: loads `settings.flow` for its project and follows its changes. */
export function FlowSection({ project }: { project: string }) {
  const [section, setSection] = useState<FlowSettings | null>(null);
  const [error, setError] = useState<RequestError | null>(null);
  const [presets, setPresets] = useState<PresetDialogView | null>(null);
  const load = useCallback(() => request("settings.flow", { project }) as Promise<FlowSettings>, [project]);
  useEffect(
    () =>
      watchView(load, REFETCH_ON, (state) => {
        if (state.view) setSection(state.view);
        setError(state.error ?? null);
      }),
    [load],
  );
  const failed = (e: unknown) => {
    setError(asRequestError(e));
  };
  if (section === null) return error ? <p className="settings-problem">{error.message}</p> : null;
  return (
    <>
      {error && (
        <p className="settings-problem" role="alert">
          {error.message}
        </p>
      )}
      <FlowSectionBody
        section={section}
        onCancelChange={() => {
          request("settings.flow_cancel", { project }).then((fresh) => {
            setSection(fresh as FlowSettings);
            setError(null);
          }, failed);
        }}
        onChange={() => {
          changeFlow(section);
        }}
        onPreset={() => {
          request("settings.flow_presets", { project }).then((d) => {
            setPresets(d as PresetDialogView);
          }, failed);
        }}
        onCopy={(from) => {
          proposeFrom(project, { from: "project", project: from }).catch(failed);
        }}
        onMenu={(id) => {
          menuChoice(section, id).then(() => {
            setError(null);
          }, failed);
        }}
      />
      {presets !== null && (
        <PresetDialog
          dialog={presets}
          onCancel={() => {
            setPresets(null);
          }}
          onPick={(file) => {
            setPresets(null);
            proposeFrom(project, { from: "preset", file }).catch(failed);
          }}
        />
      )}
    </>
  );
}
