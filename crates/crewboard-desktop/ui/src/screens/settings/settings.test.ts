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
    },
    {
      field: "agent_limits.max_concurrency",
      label: "Max agents running at once",
      description: "Suggested 5. Queued tickets wait for a free slot.",
      control: { kind: "stepper", value: 5, min: 1, max: 50, suffix: "", disabled: true },
      dimmed: true,
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
      "usage",
      "notifications",
      "data",
      "appearance",
      "tools",
    ]);
    expect(app.settings.project.ids()).toContain("remove_project");
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
        },
      ],
      note: null,
    };
    const out = html(withPage(createElement(Section, { section })));
    expect(out).toContain('aria-invalid="true"');
    expect(out).toContain('<span class="settings-problem">localhost isn&#x27;t a URL</span>');
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
