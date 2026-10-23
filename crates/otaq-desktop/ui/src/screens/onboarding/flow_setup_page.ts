// A later project's Flow setup page (T-0436; ASK-04, FED-09), without the DOM: whether the main area shows the page
// (the flow editor) or the board. Rust says whether the project is in Flow setup (`onboarding.flow_setup_page`).
import type { FlowSetupPageView } from "../../generated/FlowSetupPageView";

/** What changes whether the page shows: a project added or settled, and the Flow setup's own push. */
export const FLOW_SETUP_PAGE_EVENTS = ["projects_changed", "flow_setup_changed"] as const;

/**
 * What the main area draws for `project`: its board, its Flow setup page, or nothing while the answer for it hasn't
 * come (a project in Flow setup never flashes a board). While onboarding is open its Flow step shows that project's
 * editor itself, so the page behind it stays empty.
 */
export function mainAreaShows(
  view: FlowSetupPageView | undefined,
  project: string | null,
  onboardingOpen: boolean,
): "board" | "page" | "nothing" {
  if (project === null) return "board";
  if (view?.project !== project) return "nothing";
  if (!view.shows) return "board";
  return onboardingOpen ? "nothing" : "page";
}
