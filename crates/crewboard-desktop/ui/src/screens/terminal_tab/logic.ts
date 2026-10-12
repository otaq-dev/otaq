// The Terminal tab's own decisions, as plain functions the tests drive without a terminal: which body it shows, which
// strip (state label and hint) fits the terminal's phase and focus, whether the panel is narrow, and whether a
// terminal focus request is for it. Every text comes from `terminal_tab.view` (Rust).
import type { TerminalFocusRequest } from "../../app/selection";
import type { TerminalPhase } from "../../generated/TerminalPhase";
import type { TerminalStripTone } from "../../generated/TerminalStripTone";
import type { TerminalTabStrip } from "../../generated/TerminalTabStrip";
import type { TerminalTabView } from "../../generated/TerminalTabView";

/** What fills the tab under its header strip. */
export type TabBody = "terminal" | "recovery" | "no_session";

export function bodyOf(view: TerminalTabView): TabBody {
  if (view.recovery !== null) return "recovery";
  return view.session === null ? "no_session" : "terminal";
}

/** The terminal's phase as the strip shows it: a ticket without a session is `no_session` whatever it was told. */
export function shownPhase(view: TerminalTabView, phase: TerminalPhase): TerminalPhase {
  return bodyOf(view) === "terminal" ? phase : "no_session";
}

/** The state label and hint for this phase and focus. */
export function stripFor(
  view: TerminalTabView,
  phase: TerminalPhase,
  focused: boolean,
): TerminalTabStrip | undefined {
  const shown = shownPhase(view, phase);
  return view.strips.find((s) => s.phase === shown && s.focused === focused);
}

/** The CSS colour of a state label. */
export function toneColor(tone: TerminalStripTone): string {
  switch (tone) {
    case "working":
      return "var(--st-working)";
    case "attention":
      return "var(--st-attn)";
    case "secondary":
      return "var(--text-secondary)";
    case "muted":
      return "var(--text-muted)";
    case "error":
      return "var(--st-error)";
  }
}

/** *Narrow panel · 480 px*: the strip drops the path and the port and shortens the attach command. */
export function isNarrow(width: number, view: TerminalTabView): boolean {
  return width > 0 && width < view.narrow_below;
}

/** A focus request this tab answers: for its ticket and newer than the last one answered. */
export function answers(request: TerminalFocusRequest | null, ticket: string, answered: number): boolean {
  return request !== null && request.ticket === ticket && request.seq > answered;
}

/** "Scrolled up 1,320 lines": the hint while the terminal is scrolled up (a count only the page knows). */
export function scrolledLabel(lines: number): string {
  return lines === 1 ? "Scrolled up 1 line" : `Scrolled up ${lines.toLocaleString("en-US")} lines`;
}
