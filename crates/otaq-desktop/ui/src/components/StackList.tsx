// The Stack list (DEP-05, d0006; *12 · Stack & dependencies → Stack list · detail*): the chain as an indented list
// with statuses; the current ticket is marked and a click selects a row's ticket. The rows and their texts come from
// Rust (`stack.rs`, T-0222); this is the shell they are drawn in.
import type { ReactNode } from "react";

import type { PillView } from "../generated/PillView";
import { Icon } from "../theme/Icon";
import { StatusPill } from "./Pill";

export interface StackListProps {
  /** "Stack". */
  title: string;
  /** Shown when there are no rows ("SHOP-12 has no dependencies…"). */
  empty?: string | null;
  children?: ReactNode;
}

export function StackList({ title, empty, children }: StackListProps) {
  return (
    <div className="stack-list">
      <div className="stack-list-head">
        <Icon name="link-2" size={12} />
        <span>{title}</span>
      </div>
      {empty !== null && empty !== undefined ? <div className="stack-list-empty">{empty}</div> : children}
    </div>
  );
}

export interface StackRowProps {
  /** How deep in the chain: 14 px of indent each, with a `└` from depth 1. */
  depth: number;
  ticketKey: string;
  title: string;
  /** A Queued ticket's waiting text, the card's. */
  waits?: string | null;
  pill: PillView;
  current: boolean;
  onSelect: () => void;
}

export function StackRow({ depth, ticketKey, title, waits, pill, current, onSelect }: StackRowProps) {
  return (
    <button
      type="button"
      className="stack-row"
      aria-current={current || undefined}
      style={{ paddingLeft: 6 + 14 * depth }}
      onClick={onSelect}
    >
      {depth > 0 && <span className="stack-row-branch">└</span>}
      <span className="stack-row-key">{ticketKey}</span>
      <span className="stack-row-text">
        <span className="stack-row-title">{title}</span>
        {waits !== null && waits !== undefined && <span className="stack-row-waits">{waits}</span>}
      </span>
      <StatusPill pill={pill} />
    </button>
  );
}
