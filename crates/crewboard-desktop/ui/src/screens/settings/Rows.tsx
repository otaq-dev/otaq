// A Settings section and its rows (*15 · Settings*): label and description on the left (320 px), the control on the
// right, a line under each row. Every text and every control's state comes from the Rust row (`SettingsRow`); the
// controls keep only what is being typed. Text saves on blur and on ↵ (a multi-line field: blur and ⌘↵ / Ctrl+↵).
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from "react";

import {
  AnchoredMenu,
  Button,
  IconButton,
  pickTheme,
  Segmented,
  TextArea,
  TextInput,
  ThemePicker,
  Toggle,
  useThemePicker,
} from "../../components";
import type { MenuItem } from "../../generated/MenuItem";
import type { SettingsControl } from "../../generated/SettingsControl";
import type { SettingsProvider } from "../../generated/SettingsProvider";
import type { SettingsRow } from "../../generated/SettingsRow";
import type { SettingsSection } from "../../generated/SettingsSection";
import type { SettingsTools } from "../../generated/SettingsTools";
import { asRequestError } from "../../service";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { fieldId, sectionOf, useSettings } from "./context";

type Of<K extends SettingsControl["kind"]> = Extract<SettingsControl, { kind: K }>;

/** A text field that saves when it loses focus or on ↵, only when it changed. */
function TextControl({ field, control }: { field: string; control: Of<"text"> }) {
  const { edit } = useSettings();
  const [draft, setDraft] = useState(control.value);
  const [base, setBase] = useState(control.value);
  // A fresh value from the service (another window, a reload) replaces the draft.
  if (base !== control.value) {
    setBase(control.value);
    setDraft(control.value);
  }
  const save = () => {
    if (draft !== control.value) edit(field, draft);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    const submit = control.multiline ? e.key === "Enter" && (e.metaKey || e.ctrlKey) : e.key === "Enter";
    if (submit) {
      e.preventDefault();
      save();
    }
  };
  const common = {
    id: fieldId(field),
    value: draft,
    onChange: setDraft,
    onBlur: save,
    placeholder: control.placeholder,
    mono: control.mono,
    invalid: control.problem !== null,
  };
  return (
    <div className="settings-field" onKeyDown={onKeyDown} role="presentation">
      {control.multiline ? <TextArea {...common} rows={3} /> : <TextInput {...common} />}
      {control.problem !== null && <span className="settings-problem">{control.problem}</span>}
    </div>
  );
}

/** − value + (*15 → Agent limits*, terminal font size); the value is clamped by the Rust edit. */
function Stepper({ field, label, control }: { field: string; label: string; control: Of<"stepper"> }) {
  const { edit } = useSettings();
  const step = (by: number) => {
    edit(field, Math.min(control.max, Math.max(control.min, control.value + by)));
  };
  return (
    <span className="settings-stepper" role="group" aria-label={label}>
      <button
        type="button"
        aria-label="−"
        disabled={control.disabled || control.value <= control.min}
        onClick={() => {
          step(-1);
        }}
      >
        <Icon name="minus" size={13} />
      </button>
      <span className="settings-stepper-value">
        {control.value}
        {control.suffix}
      </span>
      <button
        type="button"
        aria-label="+"
        disabled={control.disabled || control.value >= control.max}
        onClick={() => {
          step(1);
        }}
      >
        <Icon name="plus" size={13} />
      </button>
    </span>
  );
}

function Skills({ control }: { control: Of<"skills"> }) {
  return (
    <div className="settings-skills">
      {control.rows.map((row) => (
        <div key={row.field} className="settings-skill">
          <label className="settings-skill-label" htmlFor={fieldId(row.field)}>
            {row.label}
          </label>
          <TextControl
            field={row.field}
            control={{
              kind: "text",
              value: row.value,
              placeholder: row.placeholder,
              mono: true,
              multiline: false,
              problem: row.problem,
            }}
          />
        </div>
      ))}
    </div>
  );
}

function EnvFiles({ control }: { control: Of<"env_files"> }) {
  const { edit } = useSettings();
  const [adding, setAdding] = useState("");
  return (
    <div className="settings-env">
      {control.rows.map((row, i) => (
        <div key={`${String(i)}-${row.value}`} className="settings-env-row">
          <div className="settings-env-line">
            <TextControl
              field={`env_files.${String(i)}`}
              control={{
                kind: "text",
                value: row.value,
                placeholder: "",
                mono: true,
                multiline: false,
                problem: row.problem,
              }}
            />
            <IconButton
              icon="x"
              label={control.remove_label}
              onClick={() => {
                edit("env_files.remove", i);
              }}
            />
          </div>
        </div>
      ))}
      <div
        role="presentation"
        onKeyDown={(e) => {
          if (e.key !== "Enter" || adding.trim() === "") return;
          e.preventDefault();
          edit("env_files.add", adding);
          setAdding("");
        }}
      >
        <TextInput
          value={adding}
          onChange={setAdding}
          placeholder={control.add_placeholder}
          label={control.add_placeholder}
          mono
        />
      </div>
    </div>
  );
}

/** The base branch dropdown: the repository's branches, asked for when it opens. */
function Branch({ control }: { control: Of<"branch"> }) {
  const { edit, project } = useSettings();
  const trigger = useRef<HTMLButtonElement>(null);
  const [items, setItems] = useState<readonly MenuItem[] | null>(null);
  const open = () => {
    if (project === null) return;
    void request("settings.branches", { project }).then(
      (list) => {
        setItems(list as MenuItem[]);
      },
      () => {
        setItems(null);
      },
    );
  };
  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="settings-branch"
        aria-haspopup="menu"
        aria-expanded={items !== null}
        onClick={open}
      >
        <Icon name="git-branch" size={13} />
        <span>{control.value}</span>
        <Icon name="chevron-down" size={13} />
      </button>
      {items && (
        <AnchoredMenu
          anchor={trigger}
          label={control.value}
          items={items}
          list
          width={280}
          onChoose={(id) => {
            edit("base_branch", id);
          }}
          onClose={(how) => {
            setItems(null);
            if (how === "key") trigger.current?.focus();
          }}
        />
      )}
    </>
  );
}

function Themes() {
  const picker = useThemePicker();
  if (picker.error) return <p className="settings-problem">{picker.error.message}</p>;
  if (!picker.view) return null;
  return (
    <ThemePicker
      sections={picker.view}
      inRows
      onPick={(key) => {
        void pickTheme(key);
      }}
    />
  );
}

/** The PR provider, detected when the page opens (it reads the remote and asks the CLI for its login). */
function useProvider(project: string | null): SettingsProvider | null {
  const [provider, setProvider] = useState<SettingsProvider | null>(null);
  useEffect(() => {
    if (project === null) return;
    let live = true;
    void request("settings.provider", { project }).then(
      (p) => {
        if (live) setProvider(p as SettingsProvider);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [project]);
  return provider;
}

function ProviderRow({ row }: { row: SettingsRow }) {
  const { project } = useSettings();
  const provider = useProvider(project);
  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <span className="settings-row-label">{row.label}</span>
        {provider && <span className="settings-row-description">{provider.description}</span>}
      </div>
      <div className="settings-row-control">
        {provider && (
          <span className="settings-provider" data-look={provider.look}>
            <Icon name={provider.icon} size={13} />
            <span>{provider.status}</span>
            {provider.link !== null && (
              <button
                type="button"
                className="settings-link"
                onClick={() => {
                  document.getElementById(fieldId("workflow_skills.open_pr"))?.focus();
                }}
              >
                {provider.link}
              </button>
            )}
          </span>
        )}
      </div>
    </div>
  );
}

function Tools({ control }: { control: Of<"tools"> }) {
  const [tools, setTools] = useState<SettingsTools | null>(null);
  const [checking, setChecking] = useState(true);
  const run = useCallback(() => {
    void request("settings.tools").then(
      (t) => {
        setTools(t as SettingsTools);
        setChecking(false);
      },
      (e: unknown) => {
        setTools({ rows: [], error: asRequestError(e).message });
        setChecking(false);
      },
    );
  }, []);
  useEffect(run, [run]);
  const check = () => {
    setChecking(true);
    run();
  };
  return (
    <div className="settings-tools">
      {tools?.error != null && <p className="settings-problem">{tools.error}</p>}
      {tools?.rows.map((row) => (
        <div key={row.name} className="settings-tool" data-look={row.look}>
          <Icon name={row.icon} />
          <span className="settings-tool-name">{row.name}</span>
          <span className="settings-tool-need">{row.need}</span>
          <span className="settings-tool-status">{row.status}</span>
          {row.command !== null && <code className="settings-tool-command">{row.command}</code>}
        </div>
      ))}
      <span className="settings-tools-recheck">
        <Button
          variant="secondary"
          icon="rotate-ccw"
          label={control.recheck_label}
          loading={checking}
          onClick={check}
        />
      </span>
    </div>
  );
}

function Control({ row }: { row: SettingsRow }) {
  const { edit, act } = useSettings();
  const c = row.control;
  switch (c.kind) {
    case "text":
      return <TextControl field={row.field} control={c} />;
    case "toggle":
      return (
        <Toggle
          on={c.on}
          label={row.label}
          showLabel={false}
          onChange={(on) => {
            edit(row.field, on);
          }}
        />
      );
    case "stepper":
      return <Stepper field={row.field} label={row.label} control={c} />;
    case "segmented":
      return (
        <Segmented
          label={row.label}
          options={c.choices.map((choice) => choice.label)}
          selected={Math.max(
            0,
            c.choices.findIndex((choice) => choice.id === c.value),
          )}
          onSelect={(i) => {
            const choice = c.choices[i];
            if (choice) edit(row.field, choice.id);
          }}
        />
      );
    case "note":
      return <span className="settings-note">{c.text}</span>;
    case "value":
      return (
        <span className="settings-value" data-mono={c.mono || undefined} data-boxed={c.boxed || undefined}>
          {c.text}
        </span>
      );
    case "path":
      return (
        <span className="settings-path">
          <span className="settings-value" data-mono data-boxed>
            {c.text}
          </span>
          <Button
            variant="secondary"
            icon="folder-open"
            label={c.reveal_label}
            onClick={() => {
              void request("settings.reveal", { target: c.reveal });
            }}
          />
        </span>
      );
    case "button":
      return (
        <span>
          <Button
            variant={c.destructive ? "destructive" : "secondary"}
            icon={c.icon}
            label={c.label}
            onClick={() => {
              act(c.action);
            }}
          />
        </span>
      );
    case "skills":
      return <Skills control={c} />;
    case "env_files":
      return <EnvFiles control={c} />;
    case "branch":
      return <Branch control={c} />;
    case "themes":
      return null;
    case "provider":
      return null;
    case "tools":
      return <Tools control={c} />;
  }
}

function Row({ row }: { row: SettingsRow }) {
  if (row.control.kind === "provider") return <ProviderRow row={row} />;
  if (row.control.kind === "tools") return <Control row={row} />;
  // *Theme*: the row's text, then the picker across the whole width.
  if (row.control.kind === "themes")
    return (
      <>
        <div className="settings-row" data-open>
          <div className="settings-row-text">
            <span className="settings-row-label">{row.label}</span>
            <span className="settings-row-description">{row.description}</span>
          </div>
        </div>
        <Themes />
      </>
    );
  return (
    <div className="settings-row" data-dimmed={row.dimmed || undefined}>
      <div className="settings-row-text">
        <span className="settings-row-label">{row.label}</span>
        {row.description !== "" && <span className="settings-row-description">{row.description}</span>}
      </div>
      <div className="settings-row-control">
        <Control row={row} />
      </div>
    </div>
  );
}

export function Section({ section }: { section: SettingsSection<string> }) {
  return (
    <section className="settings-section" data-section={section.id} aria-label={section.title ?? section.id}>
      {section.title !== null && <h3 className="settings-section-title">{section.title}</h3>}
      {section.intro !== null && <p className="settings-intro">{section.intro}</p>}
      {section.rows.map((row) => (
        <Row key={`${row.field}-${row.label}`} row={row} />
      ))}
      {section.note !== null && (
        <p className="settings-right-now">
          <Icon name="info" size={13} />
          <span>{section.note}</span>
        </p>
      )}
    </section>
  );
}

/** The registered component of one section id: draws that section of the open page, if it has one. */
export function sectionComponent(id: string) {
  function SettingsSectionView() {
    const { page } = useSettings();
    const section = sectionOf(page, id);
    return section ? <Section section={section} /> : null;
  }
  SettingsSectionView.displayName = `SettingsSection(${id})`;
  return SettingsSectionView;
}
