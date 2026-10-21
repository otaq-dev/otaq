// A later project's Flow setup page (T-0436; ASK-04, ASK-08, d0108; *14 → Row · Another project, and the presets*
// `s14-18`/`s14-19`), without the DOM: whether the main area shows the page or the board, and where Back to Tools
// goes. Rust says whether the project is in Flow setup (`onboarding.flow_setup_page`).
import { openOverlay } from "../../app/overlays";
import type { OverlayParams } from "../../app/registry";
import type { FlowSetupPageView } from "../../generated/FlowSetupPageView";

/** What changes whether the page shows: a project added or settled, and the Flow setup's own push. */
export const FLOW_SETUP_PAGE_EVENTS = ["projects_changed", "flow_setup_changed"] as const;

/**
 * What the main area draws for `project`: its board, its Flow setup page, or nothing while the answer for it hasn't
 * come (a project in Flow setup never flashes a board). While onboarding is open its Flow step shows that project's
 * Flow setup itself, so the page behind it stays empty.
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

/** Back to Tools on the page (no tool connected): Settings → Agent tools (first run goes to its Tools step). */
export function backToTools(
  view: FlowSetupPageView,
  open: (params: OverlayParams["settings"]) => void = (params) => {
    openOverlay("settings", params);
  },
): void {
  open({ section: view.back_to_tools as NonNullable<OverlayParams["settings"]>["section"] });
}
