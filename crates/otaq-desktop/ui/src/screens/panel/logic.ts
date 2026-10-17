// The panel's interaction state and where each button goes (T-0222). What the panel says and which buttons it has
// are Rust's (`src/api/panel.rs`); this file keeps only the moment of interaction: the dragged width (kept per window
// size class in the frame's `shellLayout`, with the expanded view), the picked answer, and which buttons open a tab or
// a dialog on the page instead of calling the service.
import type { ClassDrags } from "../../app/layout";
import type { AgentRole } from "../../generated/AgentRole";
import type { CardActionKind } from "../../generated/CardActionKind";
import type { PanelAction } from "../../generated/PanelAction";
import type { PanelAnswerRow } from "../../generated/PanelAnswerRow";
import type { PanelSizes } from "../../generated/PanelSizes";
import type { PanelTab } from "../../generated/PanelTab";
import type { PanelTabMark } from "../../generated/PanelTabMark";
import type { SizeClass } from "../../generated/SizeClass";
import type { TabIndicator } from "../../components";

/**
 * The width a drag of the left edge gives: from the pointer to the window's right edge, within the sizes Rust sent
 * up front (`panel::clamp_width`: at least 480 px, and the main area keeps 240 px on the panel's left).
 */
export function draggedWidth(
  pointerX: number,
  windowWidth: number,
  mainWidth: number,
  sizes: PanelSizes,
): number {
  return clampWidth(windowWidth - pointerX, mainWidth, sizes);
}

export function clampWidth(width: number, mainWidth: number, sizes: PanelSizes): number {
  const max = Math.max(mainWidth - sizes.main_keeps, sizes.min_width);
  return Math.min(Math.max(width, sizes.min_width), max);
}

/** The width drawn now in a window of `cls`: the one dragged for that class, else its default (`panel::width`). */
export function shownWidth(
  dragged: ClassDrags,
  cls: SizeClass,
  mainWidth: number,
  sizes: PanelSizes,
): number {
  return clampWidth(dragged[cls] ?? sizes.default_widths[cls], mainWidth, sizes);
}

/** Narrow panels show the tabs as icons (`ICON_TABS_BELOW`); the expanded panel never does. */
export function iconTabs(width: number, expanded: boolean, sizes: PanelSizes): boolean {
  return !expanded && width < sizes.icon_tabs_below;
}

/**
 * The tab the panel shows: the selected one when this ticket has it, else its first (an epic has no Terminal tab, so
 * it opens on Ticket, or Diff in review; d0083). The selection itself is kept for the next ticket.
 */
export function shownTab(tabs: readonly PanelTab[], selected: PanelTab): PanelTab {
  return tabs.includes(selected) ? selected : (tabs[0] ?? selected);
}

/** The tab ⌘⌥`n` shows: the `n`th of the shown tabs (an epic's are numbered in their own order). */
export function numberedTab(tabs: readonly PanelTab[], n: number): PanelTab | undefined {
  return n >= 1 ? tabs[n - 1] : undefined;
}

/** The tab strip's marker, as the Tabs component draws it. */
export function tabIndicator(mark: PanelTabMark | null): TabIndicator | undefined {
  return mark ?? undefined;
}

/** Where a button goes: the service (`panel.action`), a tab, or a dialog. */
export type ActionRoute =
  | { kind: "service" }
  | { kind: "terminal" }
  | { kind: "validator" }
  | { kind: "answer_agent" }
  | { kind: "diff" }
  | { kind: "edit" }
  | { kind: "parent" }
  | { kind: "overlay"; id: "local_merge" | "send_back" | "start_from_scratch" }
  | { kind: "mark_closed" }
  | { kind: "run" }
  // Change agent / Change validator (T-0345): the Ticket tab with that row's picker open.
  | { kind: "agent_row"; role: AgentRole };

// No `default`: a new action fails ESLint's switch-exhaustiveness-check until it gets its route.
export function actionRoute(action: PanelAction): ActionRoute {
  switch (action) {
    case "start":
    case "retry_start":
    case "resume":
    case "retry_resume":
    case "reopen":
    case "mark_merged":
    case "open_pr_in_browser":
    case "move_to_review_anyway":
    case "another_round":
    case "retry_validation":
    case "retry_merge":
    case "move_to_review":
    case "resume_lead":
      return { kind: "service" };
    // The Terminal tab on the validator's side (its switch is T-0237's).
    case "open_validator":
    case "answer_validator":
      return { kind: "validator" };
    // Opens the note box under the bar; Send to agent calls `panel.answer_agent`.
    case "answer_agent":
      return { kind: "answer_agent" };
    case "open_terminal":
    case "answer_in_terminal":
      return { kind: "terminal" };
    case "view_diff":
      return { kind: "diff" };
    case "edit":
      return { kind: "edit" };
    case "view_parent":
      return { kind: "parent" };
    case "approve":
      return { kind: "overlay", id: "local_merge" };
    case "send_back":
      return { kind: "overlay", id: "send_back" };
    case "start_from_scratch":
      return { kind: "overlay", id: "start_from_scratch" };
    case "mark_closed":
      return { kind: "mark_closed" };
    case "run":
      return { kind: "run" };
    case "change_agent":
      return { kind: "agent_row", role: "developer" };
    case "change_validator":
      return { kind: "agent_row", role: "validator" };
  }
}

/** Where a "…" menu item goes; ids are `panel_actions.rs`'s, `close.rs`'s, `card_menu.rs`'s and `delete.rs`'s. */
export type MenuRoute =
  | { kind: "edit" }
  | { kind: "service" }
  | { kind: "copy"; what: "branch" | "attach" }
  | { kind: "close_epic" }
  | {
      kind: "overlay";
      id: "close_ticket" | "submit_for_review" | "delete_ticket" | "move_to_epic";
    }
  | { kind: "unknown" };

export function menuRoute(item: string): MenuRoute {
  switch (item) {
    case "edit-ticket":
      return { kind: "edit" };
    case "pin":
    case "unpin":
    case "stop-validation":
    case "skip-to-review":
    case "validate-and-submit":
      return { kind: "service" };
    case "copy-branch":
      return { kind: "copy", what: "branch" };
    case "copy-attach":
      return { kind: "copy", what: "attach" };
    case "close-ticket":
      return { kind: "overlay", id: "close_ticket" };
    case "submit-for-review":
      return { kind: "overlay", id: "submit_for_review" };
    case "delete":
      return { kind: "overlay", id: "delete_ticket" };
    case "move-to-epic":
      return { kind: "overlay", id: "move_to_epic" };
    case "close-epic":
      return { kind: "close_epic" };
    default:
      return { kind: "unknown" };
  }
}

/**
 * Where a board card's button goes once the panel shows its ticket (T-0220 leaves it in `cardActions`), as the
 * GPUI app's card buttons did: the service (`panel.card`), the answer list (an option picked, or Send answer with
 * the recommended one), or a dialog. Review only opens the panel; the Run chip shows the Run tab.
 */
export type CardRoute =
  | { kind: "service" }
  | { kind: "pick"; number: number }
  | { kind: "send_answer" }
  | { kind: "overlay"; id: "start_from_scratch" | "open_pr" }
  | { kind: "mark_closed" }
  | { kind: "run_tab" }
  | { kind: "agent_row"; role: AgentRole }
  | { kind: "none" };

export function cardRoute(kind: CardActionKind): CardRoute {
  if (typeof kind === "object") return { kind: "pick", number: kind.pick_option };
  switch (kind) {
    case "deliver_now":
    case "resume":
    case "retry_resume":
    case "retry_start":
    case "retry":
    case "reopen":
    case "mark_merged":
    case "retry_validation":
    case "retry_merge":
    case "move_to_review":
      return { kind: "service" };
    case "send_answer":
      return { kind: "send_answer" };
    case "start_from_scratch":
      return { kind: "overlay", id: "start_from_scratch" };
    // REV-14: Push to PR opens the PR dialog (T-0227).
    case "push_to_pr":
      return { kind: "overlay", id: "open_pr" };
    case "mark_closed":
      return { kind: "mark_closed" };
    // RUN-01: the `▶ :4020` chip shows the Run tab (its Open and Stop).
    case "run_chip":
      return { kind: "run_tab" };
    // AGT-05: the panel the click opens shows the Ticket tab with that row's picker open (T-0345).
    case "change_agent":
      return { kind: "agent_row", role: "developer" };
    case "change_validator":
      return { kind: "agent_row", role: "validator" };
    // Answer, Reply and Open (Ticket Card v2): the panel the click opens is where it is dealt with.
    case "open":
    case "review":
      return { kind: "none" };
  }
}

/** The answer picked when the list opens: the recommended option (Rust marks it picked), else nothing. */
export function initialPick(rows: readonly PanelAnswerRow[]): number | null {
  return rows.find((r) => r.picked)?.number ?? null;
}

/** A number key on the list: the option with that number, if there is one. */
export function pickByKey(key: string, rows: readonly PanelAnswerRow[]): number | null {
  const n = Number(key);
  return Number.isInteger(n) && rows.some((r) => r.number === n) ? n : null;
}

/** Whether the picked row is Other…, which needs typed text before Send is enabled. */
export function picksOther(pick: number | null, rows: readonly PanelAnswerRow[]): boolean {
  return rows.some((r) => r.other && r.number === pick);
}

/** Send is enabled once something is picked, and Other… has text (Rust checks it again: `answer_options.rs`). */
export function canSend(pick: number | null, other: string, rows: readonly PanelAnswerRow[]): boolean {
  if (pick === null) return false;
  return !picksOther(pick, rows) || other.trim() !== "";
}
