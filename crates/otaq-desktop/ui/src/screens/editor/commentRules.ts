// The comment box's rules while typing (CMT-11, d0109; *05 → Row · Writing, editing and deleting*): a paste over the
// limit becomes the report chip, never text; typing past it shows "2,140 / 2,000" and turns the button off; with only
// a chip the comment still needs its own line. The limit comes with the view (`CommentLimits`, from core's constant);
// the service checks it again. Typed text is never turned into a report.
import type { CommentLimits } from "../../generated/CommentLimits";

/** A paste kept as the comment's report: its text and its size in UTF-8 bytes. */
export interface Pasted {
  text: string;
  bytes: number;
}

/** What the box holds: the typed text and at most one pasted report. */
export interface BoxValue {
  text: string;
  pasted: Pasted | null;
}

export const EMPTY_BOX: BoxValue = { text: "", pasted: null };

/** Unicode characters (code points), as the service counts them. */
export function chars(text: string): number {
  return Array.from(text).length;
}

/** What a paste does: over the limit it becomes the report chip; otherwise it goes into the text as usual. */
export function pasteAction(pasted: string, limits: CommentLimits): "chip" | "text" {
  return chars(pasted) > limits.chars ? "chip" : "text";
}

/** The box after a paste that becomes the chip: it replaces an earlier one (one report per comment). */
export function withPasted(value: BoxValue, text: string): BoxValue {
  return { ...value, pasted: { text, bytes: new TextEncoder().encode(text).length } };
}

/** "2,140". */
function grouped(n: number): string {
  return n.toLocaleString("en-US");
}

export interface BoxState {
  /** The text's length as the service counts it (trimmed). */
  count: number;
  /** Past the limit: the red box. */
  over: boolean;
  /** "2,140 / 2,000" while over the limit. */
  counter: string | null;
  /** Only a chip, no line of text: "Say what the next agent should look for". */
  needsLine: boolean;
  /** The button may send it. */
  canSubmit: boolean;
}

/** The box's state; `optional` is a dialog's box that may stay empty (no text and no chip). */
export function boxState(value: BoxValue, limits: CommentLimits, optional = false): BoxState {
  const count = chars(value.text.trim());
  const over = count > limits.chars;
  const empty = count === 0;
  const needsLine = value.pasted !== null && empty;
  return {
    count,
    over,
    counter: over ? `${grouped(count)} / ${grouped(limits.chars)}` : null,
    needsLine,
    canSubmit: !over && !needsLine && (!empty || optional),
  };
}
