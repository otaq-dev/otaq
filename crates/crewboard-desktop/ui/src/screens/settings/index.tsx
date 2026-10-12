// Area 15 · Settings (T-0234): the Settings overlay (⌘,), and the global and project sections it draws, one
// registered component per section id. A later ticket adds its own sections (T-0240) by registering their ids.
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import type { GlobalSection, ProjectSection } from "../../app/registry";
import { sectionComponent } from "./Rows";
import { Settings } from "./Settings";
import "./settings.css";

/** The sections this screen draws; Validation & Auto-merge and the Project agent instructions are T-0240's. */
export const GLOBAL: readonly GlobalSection[] = [
  "agent_limits",
  "agent_instructions",
  "workflow_skills",
  "usage",
  "notifications",
  "data",
  "appearance",
  "tools",
];
export const PROJECT: readonly ProjectSection[] = [
  "general",
  "agent_instructions",
  "workflow_skills",
  "answers",
  "commands",
  "pr_provider",
  "remove_project",
];

export const settings: Screen = (app) => {
  app.overlays.register("settings", { Component: Settings });
  for (const id of GLOBAL) app.settings.global.register(id, { Component: sectionComponent(id) });
  for (const id of PROJECT) app.settings.project.register(id, { Component: sectionComponent(id) });
  app.shortcuts.on("settings", () => {
    openOverlay("settings");
  });
};
