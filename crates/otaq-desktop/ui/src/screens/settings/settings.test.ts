import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { registerScreens } from "../../app/registry";
import type { SettingsPage } from "../../generated/SettingsPage";
import type { SettingsSection } from "../../generated/SettingsSection";
import { settings } from ".";
import { SettingsContext, sectionInView, sectionOf } from "./context";
import { Section, sectionComponent } from "./Rows";

const html = (element: ReactElement) => renderToStaticMarkup(element);

afterEach(() => {
  overlays.store.set([]);
});

// The shape `settings_page.rs` sends for *Agent limits* while they are off.
const LIMITS: SettingsSection<"agent_limits"> = {
  id: "agent_limits",
  title: "Agent limits",
  intro: "Optional caps on how many agents run at once.",
  rows: [
    {
      field: "agent_limits.enabled",
      label: "Limit agents",
      description: "Off by default. When on, the header shows “3 / 5 agents”.",
      control: { kind: "toggle", on: false },
      dimmed: false,
      reset: null,
      button: null,
    },
    {
      field: "agent_limits.max_concurrency",
      label: "Max agents running at once",
      description: "Suggested 5. Queued tickets wait for a free slot.",
      control: { kind: "stepper", value: 5, min: 1, max: 50, suffix: "", disabled: true, empty: null },
      dimmed: true,
      reset: null,
      button: null,
    },
  ],
  note: null,
};

const PAGE: SettingsPage = { page: "global", title: "General", sections: [LIMITS] };

const withPage = (child: ReactElement) =>
  createElement(
    SettingsContext,
    { value: { page: PAGE, project: null, edit: () => undefined, act: () => undefined } },
    child,
  );

describe("the Settings screen", () => {
  it("registers the overlay, its sections and ⌘,", () => {
    const app = registerScreens({ settings });
    expect(app.overlays.get("settings")).toBeDefined();
    expect(app.settings.global.ids()).toEqual([
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
    ]);
    expect(app.settings.project.ids()).toContain("remove_project");
    expect(app.settings.project.ids()).toContain("validation");
    expect(app.settings.project.ids()).toContain("project_agent_instructions");
    expect(app.shortcuts.run({ action: "settings" })).toBe(true);
    expect(overlays.store.get()).toEqual([{ id: "settings", params: undefined }]);
  });

  it("draws a section's rows from the Rust view: dimmed rows, a disabled stepper, a switch", () => {
    const out = html(withPage(createElement(Section, { section: LIMITS })));
    expect(out).toContain('<h3 class="settings-section-title">Agent limits</h3>');
    expect(out).toContain("Optional caps on how many agents run at once.");
    expect(out).toContain('role="switch"');
    expect(out).toContain('aria-checked="false"');
    expect(out).toContain('data-dimmed="true"');
    expect(out).toMatch(/<button type="button" aria-label="−" disabled="">/);
  });

  it("a registered section draws only when the page has it", () => {
    expect(html(withPage(createElement(sectionComponent("agent_limits"))))).toContain("Limit agents");
    expect(html(withPage(createElement(sectionComponent("tools"))))).toBe("");
    expect(sectionOf(PAGE, "agent_limits")?.rows).toHaveLength(2);
  });

  it("a flagged value shows its reason under the field", () => {
    const section: SettingsSection<"commands"> = {
      id: "commands",
      title: null,
      intro: null,
      rows: [
        {
          field: "open_url",
          label: "Open URL",
          description: "",
          control: {
            kind: "text",
            value: "localhost",
            placeholder: "none",
            mono: true,
            multiline: false,
            problem: "localhost isn't a URL",
          },
          dimmed: false,
          reset: null,
          button: null,
        },
      ],
      note: null,
    };
    const out = html(withPage(createElement(Section, { section })));
    expect(out).toContain('aria-invalid="true"');
    expect(out).toContain('<span class="settings-problem">localhost isn&#x27;t a URL</span>');
  });

  it("Ticket resources come right after Commands, and schema paths are one multi-line field", () => {
    const app = registerScreens({ settings });
    const ids = app.settings.project.ids();
    expect(ids.indexOf("ticket_resources")).toBe(ids.indexOf("commands") + 1);
    const section: SettingsSection<"ticket_resources"> = {
      id: "ticket_resources",
      title: "Ticket resources",
      intro: null,
      rows: [
        {
          field: "schema_paths",
          label: "Schema paths",
          description: "One path per line, like db/migrations/**.",
          control: {
            kind: "text",
            value: "db/migrations/**",
            placeholder: "none",
            mono: true,
            multiline: true,
            problem: "../x must be a path inside the repository",
          },
          dimmed: false,
          reset: null,
          button: null,
        },
      ],
      note: null,
    };
    const out = html(withPage(createElement(Section, { section })));
    expect(out).toContain('<h3 class="settings-section-title">Ticket resources</h3>');
    expect(out).toContain("<textarea");
    expect(out).toContain("db/migrations/**");
    expect(out).toContain('aria-invalid="true"');
  });

  it("a project's validation rows: Use global, and Auto-merge disabled with its note", () => {
    const section: SettingsSection<"validation"> = {
      id: "validation",
      title: null,
      intro: null,
      rows: [
        {
          field: "validation",
          label: "Validation",
          description: "Off for this project.",
          control: { kind: "toggle", on: false },
          dimmed: false,
          reset: "Use global",
          button: null,
        },
        {
          field: "validation_rounds",
          label: "Validation rounds",
          description: "1–5.",
          control: {
            kind: "stepper",
            value: 3,
            min: 1,
            max: 5,
            suffix: "",
            disabled: false,
            empty: "Use global (3)",
          },
          dimmed: false,
          reset: null,
          button: null,
        },
        {
          field: "auto_merge",
          label: "Auto-merge",
          description: "",
          control: {
            kind: "radio",
            choices: [
              {
                id: "off",
                label: "Off",
                detail: "Validated tickets wait in Ready for Review for you. (default)",
              },
              { id: "all", label: "All tickets", detail: "Merged into the base branch." },
            ],
            value: null,
            disabled: true,
            empty: "Use global (Off)",
            note: "Validation is off for otaq, so Auto-merge does nothing. Turn Validation on to use it.",
          },
          dimmed: false,
          reset: null,
          button: null,
        },
      ],
      note: null,
    };
    const out = html(withPage(createElement(Section, { section })));
    expect(out).toContain('<button type="button" class="settings-reset">Use global</button>');
    expect(out).toContain("Use global (3)");
    expect(out).toContain("Use global (Off)");
    expect(out).toContain("Validation is off for otaq");
    expect(out.match(/type="radio"[^>]*disabled=""/g)).toHaveLength(2);
    expect(out).not.toContain(' checked=""');
  });

  it("the nav follows the section in view", () => {
    const tops = [0, 300, 520, 900];
    expect(sectionInView(tops, 0, false)).toBe(0);
    expect(sectionInView(tops, 310, false)).toBe(1);
    expect(sectionInView(tops, 600, false)).toBe(2);
    expect(sectionInView(tops, 600, true)).toBe(3);
    expect(sectionInView([], 10, false)).toBe(0);
  });
});
