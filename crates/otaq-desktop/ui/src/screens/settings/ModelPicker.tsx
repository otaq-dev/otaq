// The pickers of Settings → Agents (T-0343; AGT-02, AGT-03, d0086): the model field of the Add / Edit agent dialog
// (*15 → Add agent · model picker open* `k8WyHN` / `L1mzD`), a list under the field with only what the tool's own
// list has, nothing typed; and the agent pickers of the role defaults and the project's block, a dropdown of named
// agents (`mQijC` in its one-tool form), read-only with one agent (no chevron). Every row comes from the Rust view
// (`model_picker.rs`, `named_agents.rs`); this keeps only whether the list is open and which row is highlighted. The
// keys are the menus' (`menuKey`): ↑/↓ move, Home/End, ↵ or Space picks, Esc or Tab closes.
import { type KeyboardEvent, type RefObject, useEffect, useId, useRef, useState } from "react";

import { useFloatingStyle } from "../../components";
import { menuKey } from "../../components/logic";
import type { AgentOption } from "../../generated/AgentOption";
import type { AgentPicker } from "../../generated/AgentPicker";
import type { IconName } from "../../generated/IconName";
import type { MenuItem } from "../../generated/MenuItem";
import type { ModelPickerView } from "../../generated/ModelPickerView";
import { Icon } from "../../theme/Icon";

/** One line of a list: a choice, a group's heading, or a muted note in place of a group's choices. */
export interface Choice {
  kind: "option" | "heading" | "note";
  id: string;
  label: string;
  /** The muted part: "now claude-opus-5-5", "Opus · always the newest". */
  detail: string | null;
  /** Drawn in the mono font (an exact model id). */
  mono: boolean;
  detailMono: boolean;
  checked: boolean;
  icon: IconName | null;
}

/** The id the model picker uses for the tool default (its value is none: Otaq passes no model). */
export const TOOL_DEFAULT_ID = "";

/** The model picker's lines, in the view's order: Tool default, ALWAYS THE NEWEST, EXACT MODELS. */
export function modelChoices(picker: ModelPickerView): Choice[] {
  return picker.groups.flatMap((group): Choice[] => [
    ...(group.heading === null
      ? []
      : [
          {
            kind: "heading" as const,
            id: `h-${group.heading}`,
            label: group.heading,
            detail: null,
            mono: false,
            detailMono: false,
            checked: false,
            icon: null,
          },
        ]),
    ...group.items.map((item) => ({
      kind: "option" as const,
      id: item.value ?? TOOL_DEFAULT_ID,
      label: item.label,
      detail: item.detail,
      mono: item.mono,
      // "now claude-opus-5-5" names an exact model.
      detailMono: item.value !== null,
      checked: item.selected,
      icon: null,
    })),
    ...(group.empty === null
      ? []
      : [
          {
            kind: "note" as const,
            id: `n-${group.empty}`,
            label: group.empty,
            detail: null,
            mono: false,
            detailMono: false,
            checked: false,
            icon: null,
          },
        ]),
  ]);
}

/** What an agent option sends back: its entry, or none for "Use app default". */
export const USE_APP_DEFAULT_ID = "";

/** An agent picker's lines: the name, its model muted; "Use app default (…)" with its icon. */
export function agentChoices(picker: AgentPicker): Choice[] {
  return picker.options.map((option) => ({
    kind: "option",
    id: option.agent ?? USE_APP_DEFAULT_ID,
    label: option.name,
    detail: option.model,
    mono: false,
    detailMono: false,
    checked: option.selected,
    icon: option.agent === null ? "rotate-ccw" : null,
  }));
}

/** The lines as menu rows, for the menus' key rules (headings and notes are never chosen). */
function asMenu(choices: readonly Choice[]): MenuItem[] {
  return choices.map((c) => ({
    id: c.id,
    label: c.label,
    icon: null,
    shortcut: null,
    danger: false,
    disabled: false,
    hint: null,
    separated: false,
    checked: c.checked,
    header: c.kind !== "option",
  }));
}

export type ListClose = "key" | "outside";

/** The open list: takes focus, starts on the picked line, and gives back the picked id. */
export function ChoiceList({
  label,
  choices,
  className,
  onChoose,
  onClose,
}: {
  label: string;
  choices: readonly Choice[];
  className: string;
  onChoose: (id: string) => void;
  onClose: (how: ListClose) => void;
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const items = asMenu(choices);
  const [highlighted, setHighlighted] = useState<number | null>(() => {
    const at = choices.findIndex((c) => c.checked);
    return at < 0 ? null : at;
  });
  useEffect(() => {
    root.current?.focus();
  }, []);
  const onKeyDown = (e: KeyboardEvent) => {
    const result = menuKey(items, highlighted, e.key);
    switch (result.kind) {
      case "highlight":
        setHighlighted(result.index);
        break;
      case "choose":
        onChoose(result.id);
        onClose("key");
        break;
      case "close":
        onClose("key");
        break;
      case "none":
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  };
  const row = (n: number) => `${id}-${String(n)}`;
  return (
    <div
      ref={root}
      className={className}
      role="listbox"
      aria-label={label}
      tabIndex={-1}
      aria-activedescendant={highlighted === null ? undefined : row(highlighted)}
      onKeyDown={onKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) onClose("outside");
      }}
    >
      {choices.map((c, i) => {
        if (c.kind === "heading")
          return (
            <div key={c.id} className="menu-header" role="presentation">
              {c.label}
            </div>
          );
        if (c.kind === "note")
          return (
            <div key={c.id} className="settings-choice-note" role="presentation">
              {c.label}
            </div>
          );
        return (
          <button
            key={c.id}
            id={row(i)}
            type="button"
            tabIndex={-1}
            role="option"
            aria-selected={c.checked}
            className="menu-item settings-choice"
            data-highlighted={i === highlighted || undefined}
            // Keeps focus on the list, so the press isn't lost to its blur.
            onMouseDown={(e) => {
              e.preventDefault();
            }}
            onMouseMove={() => {
              if (i !== highlighted) setHighlighted(i);
            }}
            onClick={() => {
              onChoose(c.id);
              onClose("key");
            }}
          >
            <span className="menu-item-line">
              {c.icon !== null && <Icon name={c.icon} size={13} />}
              <span className="settings-choice-label" data-mono={c.mono || undefined}>
                {c.label}
              </span>
              {c.detail !== null && (
                <span className="settings-choice-detail" data-mono={c.detailMono || undefined}>
                  {c.detail}
                </span>
              )}
              {c.checked && <Icon name="check" size={13} label="current" />}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** The keys that open a closed list from its field. */
function opensList(key: string): boolean {
  return key === "ArrowDown" || key === "ArrowUp";
}

/** The dialog's Model field: a button with the picked model and what it runs today, and the list under it. Nothing
 * in it takes text. */
export function ModelPicker({
  id,
  label,
  picker,
  onPick,
}: {
  id?: string;
  label: string;
  picker: ModelPickerView;
  onPick: (model: string | null) => void;
}) {
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const value = picker.value;
  return (
    <div className="settings-model-picker">
      <button
        ref={button}
        id={id}
        type="button"
        className="select settings-model-field"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        data-open={open || undefined}
        onMouseDown={(e) => {
          if (open) e.preventDefault();
        }}
        onClick={() => {
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => {
          if (!opensList(e.key)) return;
          e.preventDefault();
          setOpen(true);
        }}
      >
        <span className="select-value" data-mono={value.mono || undefined}>
          <span>{value.label}</span>
        </span>
        {!open && value.detail !== null && <span className="settings-model-detail">{value.detail}</span>}
        <Icon name="chevron-down" size={12} />
      </button>
      {open && (
        <ChoiceList
          label={label}
          choices={modelChoices(picker)}
          className="settings-model-list"
          onChoose={(picked) => {
            onPick(picked === TOOL_DEFAULT_ID ? null : picked);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") button.current?.focus();
          }}
        />
      )}
    </div>
  );
}

/** The closed picker's text: the name, and its model muted. */
function OptionValue({ option }: { option: AgentOption }) {
  return (
    <span className="select-value settings-agent-value">
      {option.agent === null && option.name !== "" && <Icon name="rotate-ccw" size={13} />}
      <span className="settings-agent-name">{option.name}</span>
      {option.model !== null && <span className="settings-agent-model">{option.model}</span>}
    </span>
  );
}

/** The open dropdown, under its field and as wide as it. */
function FloatingList({
  anchor,
  width,
  ...list
}: {
  anchor: RefObject<HTMLElement | null>;
  width: number | undefined;
  label: string;
  choices: readonly Choice[];
  onChoose: (id: string) => void;
  onClose: (how: ListClose) => void;
}) {
  const floating = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, floating);
  return (
    <div ref={floating} className="floating" style={{ ...style, width }}>
      <ChoiceList {...list} className="menu" />
    </div>
  );
}

/** A role's agent: the picked name and its model; with one named agent only that, read-only. `agent` is the entry
 * picked, `null` for "Use app default". */
export function AgentPickerField({
  label,
  picker,
  onPick,
}: {
  label: string;
  picker: AgentPicker;
  onPick: (agent: string | null) => void;
}) {
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [width, setWidth] = useState<number>();
  if (picker.read_only)
    return (
      <span className="select settings-agent-picker" data-read-only aria-label={label} role="note">
        <OptionValue option={picker.value} />
      </span>
    );
  return (
    <>
      <button
        ref={button}
        type="button"
        className="select settings-agent-picker"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        data-open={open || undefined}
        onMouseDown={(e) => {
          if (open) e.preventDefault();
        }}
        onClick={() => {
          setWidth(button.current?.offsetWidth);
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => {
          if (!opensList(e.key)) return;
          e.preventDefault();
          setWidth(button.current?.offsetWidth);
          setOpen(true);
        }}
      >
        <OptionValue option={picker.value} />
        <Icon name="chevron-down" size={12} />
      </button>
      {open && (
        <FloatingList
          anchor={button}
          width={width}
          label={label}
          choices={agentChoices(picker)}
          onChoose={(picked) => {
            onPick(picked === USE_APP_DEFAULT_ID ? null : picked);
          }}
          onClose={(how) => {
            setOpen(false);
            if (how === "key") button.current?.focus();
          }}
        />
      )}
    </>
  );
}
