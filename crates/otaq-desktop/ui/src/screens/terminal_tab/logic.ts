// The Terminal tab's own decisions, as plain functions the tests drive without a terminal: which side of the Agent /
// Validator switch it shows, which body, which strip (state label and hint) fits the terminal's phase and focus,
// whether the panel is narrow, and whether a terminal focus request is for it. Every text comes from
// `terminal_tab.view` (Rust).
import type { TerminalFocusRequest } from "../../app/selection";
import type { TerminalPhase } from "../../generated/TerminalPhase";
import type { TerminalStripTone } from "../../generated/TerminalStripTone";
import type { TerminalTabSide } from "../../generated/TerminalTabSide";
import type { TerminalTabStrip } from "../../generated/TerminalTabStrip";
import type { TerminalTabView } from "../../generated/TerminalTabView";

/** What fills the tab under its header strip. */
export type TabBody = "terminal" | "recovery" | "no_session";

/** The side the user picked, while the tab lands on the same side and no newer focus request came. */
export interface SideChoice {
  ticket: string;
  landsOn: TerminalTabSide;
  /** The newest focus request for the ticket when it was picked (0: none). */
  focus: number;
  side: TerminalTabSide;
}

/**
 * The side shown: the validator only once a validation ran; the user's pick until the ticket or the side the tab
 * lands on changes (a round starts, the validator asks for you) or a focus request comes (⌘J, a notification
 * click), then the side that request asked for (`asked`: the panel's "Open terminal · Validator") or else the
 * side Rust names.
 */
export function shownSide(
  view: TerminalTabView,
  choice: SideChoice | null,
  focus = 0,
  asked?: TerminalTabSide,
): TerminalTabSide {
  if (view.validator === null) return "agent";
  const holds =
    choice !== null &&
    choice.ticket === view.key &&
    choice.landsOn === view.lands_on &&
    choice.focus === focus;
  return holds ? choice.side : (asked ?? view.lands_on);
}

/** The session the terminal attaches to on that side. */
export function sessionOf(view: TerminalTabView, side: TerminalTabSide): string | null {
  return side === "validator" && view.validator !== null ? view.validator.session : view.session;
}

export function bodyOf(view: TerminalTabView, side: TerminalTabSide = "agent"): TabBody {
  if (side === "validator" && view.validator !== null) return "terminal";
  if (view.recovery !== null) return "recovery";
  return view.session === null ? "no_session" : "terminal";
}

/** The terminal's phase as the strip shows it: a ticket without a session is `no_session` whatever it was told. */
export function shownPhase(
  view: TerminalTabView,
  phase: TerminalPhase,
  side: TerminalTabSide = "agent",
): TerminalPhase {
  return bodyOf(view, side) === "terminal" ? phase : "no_session";
}

/** The state label and hint for this phase and focus. */
export function stripFor(
  view: TerminalTabView,
  phase: TerminalPhase,
  focused: boolean,
  side: TerminalTabSide = "agent",
): TerminalTabStrip | undefined {
  const shown = shownPhase(view, phase, side);
  const strips = side === "validator" && view.validator !== null ? view.validator.strips : view.strips;
  return strips.find((s) => s.phase === shown && s.focused === focused);
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
