// Area 15 · Settings (T-0234): the Settings overlay (⌘,), and the global and project sections it draws, one
// registered component per section id (T-0240 registered Validation & Auto-merge and the Project agent instructions).
import { openOverlay } from "../../app/overlays";
import type { Screen } from "../../app/registry";
import type { GlobalSection, ProjectSection } from "../../app/registry";
import { AboutSection } from "./About";
import { sectionComponent } from "./Rows";
import { Settings } from "./Settings";
import "./settings.css";

/** The sections this screen draws (all of them since T-0240 added Validation & Auto-merge and Project agents). */
export const GLOBAL: readonly GlobalSection[] = [
  "agent_limits",
  "agent_instructions",
  "workflow_skills",
  "validation",
  "project_agent_instructions",
  "usage",
  "notifications",
  "data",
  "appearance",
  "tools",
  "about",
];
export const PROJECT: readonly ProjectSection[] = [
  "general",
  "agent_instructions",
  "project_agent_instructions",
  "workflow_skills",
  "validation",
  "answers",
  "commands",
  "pr_provider",
  "remove_project",
];

export const settings: Screen = (app) => {
  app.overlays.register("settings", { Component: Settings });
  for (const id of GLOBAL)
    app.settings.global.register(id, { Component: id === "about" ? AboutSection : sectionComponent(id) });
  for (const id of PROJECT) app.settings.project.register(id, { Component: sectionComponent(id) });
  app.shortcuts.on("settings", () => {
    openOverlay("settings");
  });
};
