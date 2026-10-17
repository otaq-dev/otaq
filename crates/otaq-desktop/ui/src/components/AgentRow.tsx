// The agent rows (AGT-04, VAL-14; d0086, d0077; *08 · Ticket editor → Row · Agents field*, *05 · Ticket side panel →
// Column 1 · locked rows, Row · Developer and Validator rows · every state*, *08 → Epic panel · lead running*): one
// Developer, Validator or Epic lead row as `agent_rows.rs` words it, and the "Agents" block around the rows. A row is
// its label, the named agent (bold) with its model, who set it (neutral "set by you", violet "picked by …"), Let agents
// pick again or Change, and the picker: a list under the value, "Project default (…)" first, the current one checked;
// ↑/↓ move, ↵ picks, Esc closes. A row without a picker is read-only (one named agent, or locked: the lock and its
// tooltip). The form look is the editor's 28 px field; the panel look is the panel's compact line. With several tools
// connected (M23, SHL-08) the value and the picker's items lead with the tool's avatar; a row a fallback moved says so
// in amber ("moved from Strong: Claude Code limit", AGT-08) and a tool that quietly ran another model says so,
// neutral ("asked for gpt-5.5, ran on auto", AGT-05). Never turquoise (d0077).
import { type KeyboardEvent, useEffect, useId, useRef, useState } from "react";

import type { AgentBlock as AgentBlockView } from "../generated/AgentBlock";
import type { AgentPickerItem } from "../generated/AgentPickerItem";
import type { AgentRole } from "../generated/AgentRole";
import type { AgentRowPicked } from "../generated/AgentRowPicked";
import type { AgentRowSetBy } from "../generated/AgentRowSetBy";
import type { AgentRowView } from "../generated/AgentRowView";
import { Icon } from "../theme/Icon";
import { useFloatingStyle } from "./Floating";
import { ToolAvatar } from "./ToolAvatar";
import { useDismiss } from "./Popover";
import { Tooltip } from "./Tooltip";
import "./AgentRow.css";

export type AgentRowLook = "form" | "panel";

/**
 * The row as shown while a pick made in the ⌘N form waits for the ticket to be created: the picked item's name and
 * model, *set by you* and Let agents pick again (`AgentBlock.picked`); "Project default (…)" or no pick shows the
 * row as the view gave it.
 */
export function shownRow(
  row: AgentRowView,
  pick: string | null,
  picked: AgentRowPicked | null,
): AgentRowView {
  const item = pick === null ? undefined : row.picker?.find((i) => i.agent === pick);
  if (!item || !row.picker || !picked) return row;
  return {
    ...row,
    value: item.label,
    detail: item.detail,
    set_by: picked.set_by,
    let_agents_pick: picked.let_agents_pick,
    picker: row.picker.map((i) => ({ ...i, selected: i.agent === pick })),
  };
}

/** The item the picker opens on: the current one. */
function current(items: readonly AgentPickerItem[]): number {
  return Math.max(
    items.findIndex((i) => i.selected),
    0,
  );
}

function Picker({
  label,
  items,
  anchor,
  onPick,
  onClose,
}: {
  label: string;
  items: readonly AgentPickerItem[];
  anchor: React.RefObject<HTMLElement | null>;
  onPick: (agent: string | null) => void;
  onClose: (refocus: boolean) => void;
}) {
  const id = useId();
  const list = useRef<HTMLDivElement>(null);
  const style = useFloatingStyle(anchor, list);
  const [lit, setLit] = useState(() => current(items));
  useDismiss(list, anchor, () => {
    onClose(true);
  });
  useEffect(() => {
    list.current?.focus();
  }, []);
  const pick = (item: AgentPickerItem | undefined) => {
    if (!item) return;
    onClose(true);
    if (!item.selected) onPick(item.agent);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      const delta = e.key === "ArrowDown" ? 1 : -1;
      setLit((at) => Math.min(Math.max(at + delta, 0), items.length - 1));
    } else if (e.key === "Home" || e.key === "End") {
      setLit(e.key === "Home" ? 0 : items.length - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      pick(items[lit]);
    } else if (e.key === "Tab") {
      onClose(false);
      return;
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  };
  const option = (n: number) => `${id}-${String(n)}`;
  return (
    <div
      ref={list}
      className="agent-picker"
      role="listbox"
      aria-label={label}
      tabIndex={-1}
      aria-activedescendant={option(lit)}
      style={style}
      onKeyDown={onKeyDown}
    >
      {items.map((item, i) => (
        <div
          key={item.agent ?? ""}
          id={option(i)}
          role="option"
          tabIndex={-1}
          aria-selected={item.selected}
          className="agent-picker-item"
          data-lit={i === lit || undefined}
          onMouseMove={() => {
            if (i !== lit) setLit(i);
          }}
          // Picks on the press, keeping focus in the list (the keys are the list's own: ↑ ↓ ↵ Esc).
          onMouseDown={(e) => {
            e.preventDefault();
            pick(item);
          }}
        >
          {item.agent === null && <Icon name="rotate-ccw" size={13} />}
          {item.avatar !== null && <ToolAvatar avatar={item.avatar} />}
          <span className="agent-picker-name" data-default={item.agent === null || undefined}>
            {item.label}
          </span>
          <span className="agent-picker-detail">{item.detail}</span>
          <span className="agent-row-spacer" />
          {item.selected && <Icon name="check" size={13} label="current" />}
        </div>
      ))}
    </div>
  );
}

function Who({ by }: { by: AgentRowSetBy }) {
  return (
    <span className="agent-row-who" data-agent={by.agent_made || undefined}>
      {by.icon !== null && <Icon name={by.icon} size={12} />}
      <span>{by.text}</span>
    </span>
  );
}

export interface AgentRowProps {
  row: AgentRowView;
  look: AgentRowLook;
  /** Writes the row: the picked named agent, or `null` for "Project default (…)" and Let agents pick again. */
  onPick: (agent: string | null) => void;
  /** Opens the picker with the keyboard in it (the panel's `focus_agent_row`: Change agent / Change validator). */
  focus?: boolean;
}

export function AgentRow({ row, look, onPick, focus = false }: AgentRowProps) {
  const value = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const items = row.picker;
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the request arrives with the view, once per open
    if (focus && items !== null) setOpen(true);
  }, [focus, items]);
  const toggle = () => {
    setOpen((o) => !o);
  };
  const name = (
    <>
      {row.avatar !== null && <ToolAvatar avatar={row.avatar} />}
      <span className="agent-row-name">{row.value}</span>
      {row.detail !== null && <span className="agent-row-detail">{row.detail}</span>}
    </>
  );
  // The right-hand link: Let agents pick again on a row set by you; in the panel, else Change (the editor's field has
  // only its chevron). An ended lead's "Change for the next lead" sits on its own line instead.
  const change = look === "panel" && row.next_lead === null && items !== null ? row.change : null;
  return (
    <div className="agent-row" data-look={look} data-role={row.role}>
      <div className="agent-row-line">
        <span className="agent-row-label">{row.label}</span>
        {items === null ? (
          <span className="agent-row-value" data-locked={row.lock !== null || undefined}>
            {name}
            {row.lock !== null && (
              <Tooltip text={row.lock}>
                {/* Focusable, so the keyboard reaches the tooltip too. */}
                {/* eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- the lock explains itself on focus */}
                <span className="agent-row-lock" tabIndex={0} role="img" aria-label={row.lock}>
                  <Icon name="lock" size={11} />
                </span>
              </Tooltip>
            )}
          </span>
        ) : (
          <button
            ref={value}
            type="button"
            className="agent-row-value"
            data-editable
            data-open={open || undefined}
            aria-label={`${row.label}: ${row.value}`}
            aria-haspopup="listbox"
            aria-expanded={open}
            // While open, a press here must not close the list first (it would close, then this click reopen it).
            onMouseDown={(e) => {
              if (open) e.preventDefault();
            }}
            onClick={toggle}
          >
            {name}
            <span className="agent-row-spacer" />
            <Icon name="chevron-down" size={11} />
          </button>
        )}
        {row.set_by !== null && <Who by={row.set_by} />}
        <span className="agent-row-spacer" />
        {row.let_agents_pick !== null ? (
          <button
            type="button"
            className="agent-row-link"
            onClick={() => {
              onPick(null);
            }}
          >
            {row.let_agents_pick}
          </button>
        ) : (
          change !== null && (
            <button type="button" className="agent-row-link" onClick={toggle}>
              {change}
            </button>
          )
        )}
      </div>
      {row.moved !== null && (
        <div className="agent-row-note" data-moved>
          <Icon name="arrow-right" size={12} />
          <span>{row.moved}</span>
        </div>
      )}
      {row.swap !== null && (
        <div className="agent-row-note" data-swap>
          <Icon name="refresh-cw" size={12} />
          <span>{row.swap}</span>
        </div>
      )}
      {row.moved_on !== null && (
        <div className="agent-row-note">
          <Icon name="arrow-up-right" size={12} />
          <span>{row.moved_on}</span>
        </div>
      )}
      {row.next_lead !== null && items !== null && (
        <div className="agent-row-note">
          <button type="button" className="agent-row-link" onClick={toggle}>
            {row.next_lead}
          </button>
        </div>
      )}
      {open && items !== null && (
        <Picker
          label={row.label}
          items={items}
          anchor={value}
          onPick={onPick}
          onClose={(refocus) => {
            setOpen(false);
            if (refocus) value.current?.focus();
          }}
        />
      )}
    </div>
  );
}

export interface AgentBlockProps {
  block: AgentBlockView;
  look: AgentRowLook;
  onPick: (role: AgentRole, agent: string | null) => void;
  /** The row whose picker opens with focus (`focus_agent_row`). */
  focus?: AgentRole | null;
  /** The ⌘N form's picks, not saved yet: shown with `block.picked`. */
  picks?: Readonly<Partial<Record<AgentRole, string>>>;
  /** A refusal under the rows (a row that locked meanwhile). */
  refusal?: string | null;
}

/** The "Agents" block: its label and hint, the rows, the note under them, an epic panel's children. */
export function AgentBlock({ block, look, onPick, focus = null, picks, refusal = null }: AgentBlockProps) {
  return (
    <div className="agent-block" data-look={look}>
      <div className="agent-block-label">
        <span>{block.heading}</span>
        <span className="agent-block-hint">{block.hint}</span>
      </div>
      {block.rows.map((row) => (
        <AgentRow
          key={row.role}
          row={shownRow(row, picks?.[row.role] ?? null, block.picked)}
          look={look}
          focus={focus === row.role}
          onPick={(agent) => {
            onPick(row.role, agent);
          }}
        />
      ))}
      {block.children.length > 0 && (
        <div className="agent-children">
          <span className="agent-row-label">{block.children_heading}</span>
          <div className="agent-children-list">
            {block.children.map((child) => (
              <div key={child.key} className="agent-child">
                <span className="agent-child-number">{child.number}</span>
                <span className="agent-child-name">{child.value}</span>
                {child.set_by !== null && (
                  <span className="agent-child-who" data-agent={child.set_by.agent_made || undefined}>
                    {child.set_by.text}
                  </span>
                )}
                {child.note !== null && <span className="agent-child-who">{child.note}</span>}
              </div>
            ))}
          </div>
        </div>
      )}
      {block.children_note !== null && <p className="agent-block-note">{block.children_note}</p>}
      {block.note !== null && (
        <p className="agent-block-note">
          {block.note_icon !== null && <Icon name={block.note_icon} size={12} />}
          <span>{block.note}</span>
        </p>
      )}
      {refusal !== null && (
        <p className="agent-block-note" data-error role="alert">
          {refusal}
        </p>
      )}
    </div>
  );
}
