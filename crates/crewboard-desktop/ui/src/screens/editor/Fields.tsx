// The editor's form (UI-05, TKT-03, TKT-09, DEP-01, DEP-02, DEP-04; d0019, d0051, d0065, d0067; *08 · Ticket editor
// → Full form · filled, Validation · empty form, Depends on · picker open, Cycle prevention, Row · base branch per
// ticket, Row · epics and base branches*): Title, Priority, Type and Epic (when creating), What and why, Depends
// on with its picker, Start from and merge into, External ID. Text fields are the webview's own (selection, IME,
// undo). Used by the modal (new ticket, ⌘N) and the Ticket tab.
import { useEffect, useEffectEvent, useRef, useState } from "react";

import { Field, Segmented, Select, StatusPill, TextArea } from "../../components";
import type { EditorDepChip } from "../../generated/EditorDepChip";
import type { EditorForm } from "../../generated/EditorForm";
import type { EditorPickerRow } from "../../generated/EditorPickerRow";
import type { EditorRefusal } from "../../generated/EditorRefusal";
import type { EditorView } from "../../generated/EditorView";
import type { MenuItem } from "../../generated/MenuItem";
import { request } from "../../tauri";
import { Icon } from "../../theme/Icon";
import { highlighted, refusalFor, shownPriority, step } from "./logic";

const ignore = () => undefined;

export function RefusalBox({ refusal }: { refusal: EditorRefusal | null }) {
  if (refusal === null) return null;
  return (
    <div className="editor-refusal" role="alert">
      <Icon name={refusal.icon} size={14} />
      <div>
        <p className="editor-refusal-title">{refusal.title}</p>
        <p className="editor-refusal-message">{refusal.message}</p>
      </div>
    </div>
  );
}

interface DepsProps {
  view: EditorView;
  chips: readonly EditorDepChip[];
  refusal: EditorRefusal | null;
  onAdd: (row: EditorPickerRow) => void;
  onRemove: (key: string) => void;
}

/** Depends on: the chips with their live status, and the picker that opens as you type (same project only). */
function DepsField({ view, chips, refusal, onAdd, onRemove }: DepsProps) {
  const input = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [hover, setHover] = useState<number | null>(null);
  const [rows, setRows] = useState<readonly EditorPickerRow[]>([]);
  const chosen = chips.map((c) => c.key).join(" ");
  useEffect(() => {
    if (!open) return;
    let live = true;
    const params = {
      project: view.project,
      editing: view.key,
      chosen: chosen === "" ? [] : chosen.split(" "),
      query,
    };
    (request("editor.picker", params) as Promise<EditorPickerRow[]>).then((r) => {
      if (live) setRows(r);
    }, ignore);
    return () => {
      live = false;
    };
  }, [open, query, chosen, view.project, view.key]);
  const enabled = rows.map((r) => r.enabled);
  const lit = open ? highlighted(hover, enabled) : null;
  const pick = (row: EditorPickerRow) => {
    if (!row.enabled) return;
    onAdd(row);
    setQuery("");
    setOpen(false);
    setHover(null);
  };
  // Native, so the keys stay here before an enclosing dialog's ↵ / Esc see them.
  const onKey = useEffectEvent((e: KeyboardEvent) => {
    if (!open || e.isComposing) return;
    if (e.key === "Escape") {
      setOpen(false);
    } else if (e.key === "Enter" && !e.metaKey && !e.ctrlKey) {
      const row = lit === null ? undefined : rows[lit];
      if (row) pick(row);
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      setHover((h) => step(h, e.key === "ArrowDown" ? 1 : -1, enabled));
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  });
  useEffect(() => {
    const el = input.current;
    if (!el) return;
    const listener = (e: KeyboardEvent) => {
      onKey(e);
    };
    el.addEventListener("keydown", listener);
    return () => {
      el.removeEventListener("keydown", listener);
    };
  }, []);
  return (
    <div className="editor-field">
      <div className="editor-label">
        <span>{view.texts.deps}</span>
        <span className="editor-hint">{view.texts.deps_hint}</span>
      </div>
      <div className="editor-deps" data-invalid={refusal !== null || undefined}>
        {chips.map((chip) => (
          <span key={chip.key} className="editor-chip">
            <span className="editor-chip-key">{chip.key}</span>
            <StatusPill pill={chip.pill} />
            <button
              type="button"
              className="editor-chip-remove"
              aria-label={`${view.texts.deps}: ${chip.key} ×`}
              onClick={() => {
                onRemove(chip.key);
              }}
            >
              <Icon name="x" size={11} />
            </button>
          </span>
        ))}
        <input
          ref={input}
          className="editor-deps-input"
          value={query}
          placeholder={view.texts.deps_placeholder}
          aria-label={view.texts.deps}
          aria-expanded={open}
          role="combobox"
          aria-controls="editor-picker"
          spellCheck={false}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
            setHover(null);
          }}
          onBlur={() => {
            setOpen(false);
          }}
        />
        {open && rows.length > 0 && (
          <div className="editor-picker" id="editor-picker" role="listbox" aria-label={view.texts.deps}>
            {rows.map((row, i) => (
              <div
                key={row.key}
                role="option"
                tabIndex={-1}
                aria-selected={i === lit}
                aria-disabled={!row.enabled || undefined}
                className="editor-picker-row"
                data-lit={i === lit || undefined}
                onMouseEnter={() => {
                  setHover(i);
                }}
                // Picking must not blur the field first (that would close the list before the click lands).
                onMouseDown={(e) => {
                  e.preventDefault();
                  pick(row);
                }}
              >
                <span className="editor-picker-key">{row.key}</span>
                <span className="editor-picker-text">
                  <span className="editor-picker-title">{row.title}</span>
                  <span className="editor-picker-note">{row.note}</span>
                </span>
                {row.epic_progress !== null && (
                  <span className="editor-picker-epic">{row.epic_progress}</span>
                )}
                <StatusPill pill={row.pill} />
              </div>
            ))}
          </div>
        )}
      </div>
      <RefusalBox refusal={refusal} />
    </div>
  );
}

function BaseSelect({
  view,
  form,
  onChange,
}: {
  view: EditorView;
  form: EditorForm;
  onChange: (f: EditorForm) => void;
}) {
  const [items, setItems] = useState<readonly MenuItem[]>([]);
  const editable = view.base.kind === "editable";
  useEffect(() => {
    if (!editable) return;
    let live = true;
    (request("editor.branches", { project: view.project }) as Promise<MenuItem[]>).then((list) => {
      if (live) setItems(list);
    }, ignore);
    return () => {
      live = false;
    };
  }, [editable, view.project]);
  if (view.base.kind !== "editable") {
    return (
      <Select
        label={view.texts.base}
        value={view.base.kind === "locked" ? `${form.base_branch ?? ""} · ${view.base.text}` : view.base.text}
        icon="git-branch"
        mono
        trailing="lock"
        disabled
        items={[]}
        onChange={ignore}
      />
    );
  }
  const shown = items.find((i) => i.id === form.base_branch)?.label ?? form.base_branch ?? "";
  return (
    <Select
      label={view.texts.base}
      value={shown}
      icon="git-branch"
      mono
      trailing="chevrons-up-down"
      items={items.map((i) => ({ ...i, checked: !i.header && i.id === form.base_branch }))}
      onChange={(id) => {
        onChange({ ...form, base_branch: id });
      }}
    />
  );
}

export interface FieldsProps {
  view: EditorView;
  form: EditorForm;
  onChange: (form: EditorForm) => void;
  titleMissing: boolean;
  refusal: EditorRefusal | null;
  /** The dependency chips: an existing ticket's live links, or a new ticket's picks. */
  chips: readonly EditorDepChip[];
  onAddDep: (row: EditorPickerRow) => void;
  onRemoveDep: (key: string) => void;
  autoFocusTitle?: boolean;
}

export function EditorFields({
  view,
  form,
  onChange,
  titleMissing,
  refusal,
  chips,
  onAddDep,
  onRemoveDep,
  autoFocusTitle = false,
}: FieldsProps) {
  const t = view.texts;
  const priority = shownPriority(view, form);
  const epicChild = form.epic !== null;
  const epicLabel = view.epic_menu.find((i) => i.id === (form.epic ?? ""))?.label ?? form.epic ?? t.epic_none;
  return (
    <div className="editor-fields">
      <Field label={t.title} error={titleMissing && form.title.trim() === "" ? t.title_required : null}>
        {(id) => (
          <input
            id={id}
            className="text-input editor-title"
            value={form.title}
            placeholder={t.title_placeholder}
            aria-invalid={(titleMissing && form.title.trim() === "") || undefined}
            // The form opens to be typed in: the title gets the keyboard.
            // eslint-disable-next-line jsx-a11y/no-autofocus -- opened by the user's own ⌘N, Edit or ⇧↵
            autoFocus={autoFocusTitle}
            onChange={(e) => {
              onChange({ ...form, title: e.target.value });
            }}
          />
        )}
      </Field>
      <div className="editor-field">
        <div className="editor-label">
          <span>{t.priority}</span>
          <span className="editor-hint">{view.follows ?? t.priority_hint}</span>
        </div>
        <Segmented
          label={t.priority}
          options={view.priorities.map((p) => `${p.view.short} ${p.view.word}`)}
          selected={Math.max(
            view.priorities.findIndex((p) => p.number === priority),
            0,
          )}
          onSelect={(i) => {
            const picked = view.priorities[i];
            if (picked) onChange({ ...form, priority: picked.number });
          }}
        />
      </div>
      {view.create && (
        <div className="editor-row">
          <div className="editor-field">
            <div className="editor-label">
              <span>{t.kind}</span>
            </div>
            <Segmented
              label={t.kind}
              options={[t.kind_ticket, t.kind_epic]}
              selected={form.epic_kind ? 1 : 0}
              onSelect={(i) => {
                onChange({ ...form, epic_kind: i === 1, epic: i === 1 ? null : form.epic });
              }}
            />
          </div>
          {!form.epic_kind && view.epic_menu.length > 1 && (
            <div className="editor-field editor-grow">
              <div className="editor-label">
                <span>{t.epic}</span>
                <span className="editor-hint">{t.epic_hint}</span>
              </div>
              <Select
                label={t.epic}
                value={epicLabel}
                icon="layers"
                items={view.epic_menu.map((i) => ({ ...i, checked: i.id === (form.epic ?? "") }))}
                onChange={(id) => {
                  onChange({ ...form, epic: id === "" ? null : id });
                }}
              />
            </div>
          )}
        </div>
      )}
      <div className="editor-field">
        <div className="editor-label">
          <span>{t.body}</span>
          <span className="editor-hint">{t.body_hint}</span>
        </div>
        <TextArea
          label={t.body}
          value={form.body}
          placeholder={t.body_placeholder}
          rows={8}
          onChange={(body) => {
            onChange({ ...form, body });
          }}
        />
      </div>
      {view.deps_shown && !form.epic_kind && (
        <DepsField
          view={view}
          chips={chips}
          refusal={refusalFor(refusal, "deps")}
          onAdd={onAddDep}
          onRemove={onRemoveDep}
        />
      )}
      {!(view.create && epicChild) && (
        <div className="editor-field">
          <div className="editor-label">
            <span>{t.base}</span>
            <span className="editor-hint">{t.base_hint}</span>
          </div>
          <BaseSelect view={view} form={form} onChange={onChange} />
          <RefusalBox refusal={refusalFor(refusal, "base")} />
        </div>
      )}
      {view.external_id_shown && !epicChild && (
        <div className="editor-field">
          <div className="editor-label">
            <span>{t.external_id}</span>
            <span className="editor-hint">{t.external_id_hint}</span>
          </div>
          <input
            className="text-input"
            data-mono
            value={form.external_id}
            placeholder={t.external_id_placeholder}
            aria-label={t.external_id}
            spellCheck={false}
            onChange={(e) => {
              onChange({ ...form, external_id: e.target.value });
            }}
          />
        </div>
      )}
      <RefusalBox refusal={refusalFor(refusal, "form")} />
    </div>
  );
}
