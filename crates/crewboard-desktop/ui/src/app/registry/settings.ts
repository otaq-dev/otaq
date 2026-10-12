// Settings sections (*15 · Settings*), in nav order: the global page's and each project page's. The Settings screen
// (T-0234) draws the nav and the page from the sections registered here; each section's screen registers its own.
// Labels and icons come from the Rust settings view model.
import type { ComponentType } from "react";

export const GLOBAL_SECTIONS = [
  "agent_limits",
  "agent_instructions",
  "workflow_skills",
  "usage",
  "notifications",
  "data",
  "appearance",
  "tools",
  /** M12: Validation & Auto-merge (T-0240). */
  "validation",
  /** M12 (T-0240). */
  "project_agent_instructions",
] as const;

export const PROJECT_SECTIONS = [
  /** Name, key, repository, base branch. */
  "general",
  "agent_instructions",
  /** Setup and run commands, Open URL, env files. */
  "commands",
  "pr_provider",
  /** "Let finished tickets answer questions". */
  "answers",
  /** M12 (T-0240). */
  "validation",
  /** M12 (T-0240). */
  "project_agent_instructions",
  "remove_project",
] as const;

export type GlobalSection = (typeof GLOBAL_SECTIONS)[number];
export type ProjectSection = (typeof PROJECT_SECTIONS)[number];

export type GlobalSectionEntries = Record<GlobalSection, { Component: ComponentType }>;
/** A project section is drawn for one project, by key. */
export type ProjectSectionEntries = Record<ProjectSection, { Component: ComponentType<{ project: string }> }>;
