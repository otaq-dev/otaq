// Area 02 · App shell & project switcher (T-0219): the header, the project rail, the quick switcher (⌘P), ⌘1–9 and
// the agent limits popover. What they say is Rust's (`src/api/shell.rs`, the `otaq-view` view models).
import { toggleOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import { selectProject } from "../../app/selection";
import type { ProjectDigitParams } from "../../generated/ProjectDigitParams";
import { request } from "../../tauri";
import { ShellHeader } from "./Header";
import { LimitsOverlay } from "./LimitsPopover";
import { QuickSwitcher } from "./QuickSwitcher";
import { ShellRail } from "./Rail";
import "./shell.css";

/** ⌘1–9: the project at that place in the rail, if there is one. */
export async function switchToDigit(digit: number, ask: typeof request = request): Promise<string | null> {
  const params: ProjectDigitParams = { digit };
  const key = (await ask("shell.project_for_digit", params)) as string | null;
  if (key !== null) selectProject(key);
  return key;
}

export const shell: Screen = (app) => {
  app.slots.register("header", { Component: ShellHeader });
  app.slots.register("rail", { Component: ShellRail });
  app.overlays.register("quick_switcher", { Component: QuickSwitcher });
  app.overlays.register("limits", { Component: LimitsOverlay });
  app.shortcuts.on("switch_project", () => {
    toggleOverlay("quick_switcher");
  });
  app.shortcuts.on("project", ({ number }) => {
    switchToDigit(number).catch(() => undefined);
  });
};
