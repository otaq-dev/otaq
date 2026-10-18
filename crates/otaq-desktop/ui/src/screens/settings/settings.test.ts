// @vitest-environment happy-dom
import { act, createElement, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import { overlays } from "../../app/overlays";
import { registerScreens } from "../../app/registry";
import type { GlobalSectionId } from "../../generated/GlobalSectionId";
import type { SettingsNavRow } from "../../generated/SettingsNavRow";
import type { SettingsPage } from "../../generated/SettingsPage";
import type { SettingsSection } from "../../generated/SettingsSection";
import { settings } from ".";
import { SettingsContext, sectionOf, topicRow } from "./context";
import { Section, sectionComponent, SkillRowView, skillMenu, skillStep } from "./Rows";
import type { SettingsSkillRow } from "../../generated/SettingsSkillRow";
import type { SettingsContextValue } from "./context";
import { pickFolder, request } from "../../tauri";

vi.mock("../../tauri", async (original) => ({
  ...(await original<typeof import("../../tauri")>()),
  request: vi.fn(() => Promise.resolve(null)),
  pickFolder: vi.fn(),
}));
import type { AgentToolRow } from "../../generated/AgentToolRow";
import type { AgentToolsPage } from "../../generated/AgentToolsPage";
import { agentToolMenu, AgentToolsPageBody, DisconnectDialogView, markOf, stepOf } from "./AgentTools";

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
      description: "Off by default. When on, the header shows “3/5 agents”.",
      control: { kind: "toggle", on: false },
      dimmed: false,
      reset: null,
      button: null,
    },
    {
      field: "agent_limits.max_concurrency",
      label: "Max agents running at once",
      description: "Suggested 5. Tickets in Ready for Dev wait for a free slot.",
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
      "agent_tools",
      "agents",
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
    expect(app.settings.project.ids().slice(0, 2)).toEqual(["general", "agents"]);
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

  it("opens the topic that holds a section, else the first", () => {
    const row = (label: string, sections: GlobalSectionId[]): SettingsNavRow => ({
      label,
      icon: "bot",
      target: { page: "global", section: sections[0] ?? "about", sections },
      alert: false,
    });
    const rows = [
      row("Agent tools", ["agent_tools"]),
      row("Agents", ["agents", "agent_limits", "agent_instructions"]),
    ];
    expect(topicRow(rows, "agent_instructions")?.label).toBe("Agents");
    expect(topicRow(rows, "agents")?.label).toBe("Agents");
    expect(topicRow(rows, null)?.label).toBe("Agent tools");
    expect(topicRow([], null)).toBeUndefined();
  });
});

// Rows in the shapes `agent_tools_page.rs` sends for Settings (*15* `bGHVl`).
const toolRow = (tool: string, more: Partial<AgentToolRow>): AgentToolRow => ({
  tool,
  avatar: { tool: "codex", letters: "CX", background: "tool-cx", foreground: "tool-cx-fg", hover: "Codex" },
  name: "Codex",
  pill: null,
  detail: "",
  sign_in: null,
  show: null,
  install: null,
  update: null,
  can_see: null,
  notes: [],
  added: null,
  setup_failed: null,
  warning: null,
  menu: [],
  ...more,
});
const HIDE = { action: "hide", label: "Hide from Otaq", destructive: false } as const;
const RECHECK = { action: "recheck", label: "Re-check this tool", destructive: false } as const;
const SHOW = { action: "show", label: "Show", destructive: false } as const;
const DISCONNECT = { action: "disconnect", label: "Disconnect…", destructive: true } as const;
const INSTALL = { label: "How to install (macOS)", command: "brew install codex", copy: "Copy" };

const ROWS = {
  connected: toolRow("codex", {
    pill: { label: "Connected", look: "connected" },
    detail: "v0.157.1 · signed in with ChatGPT",
    can_see: {
      label: "Otaq can see",
      chips: [
        { label: "permission prompts", seen: true },
        { label: "limits not reported", seen: false },
      ],
    },
    notes: ["Limits come from Codex’s session file."],
    added: {
      text: "Otaq added: trust for your projects, its hooks, otaq pre-approved, its skills",
      stays: "Trust for your projects stays when you disconnect.",
    },
    setup_failed: "Otaq couldn’t add its skills (the plugin folder couldn’t be written).",
    warning: {
      title: "Your Codex settings ask for permission prompts",
      text: "With 5–10 agents…",
      link: null,
    },
    menu: [HIDE, RECHECK, DISCONNECT],
  }),
  signed_out: toolRow("copilot", {
    pill: { label: "Found · not signed in", look: "not_signed_in" },
    detail: "v1.0.2 · not signed in",
    sign_in: "Sign in…",
    menu: [HIDE, RECHECK],
  }),
  update: toolRow("copilot", {
    pill: { label: "Found · update needed", look: "update_needed" },
    detail: "v0.0.404 · too old for this app",
    update: {
      label: "Update GitHub Copilot to a version Otaq runs",
      command: "npm install -g @github/copilot",
      copy: "Copy",
    },
    menu: [HIDE, RECHECK],
  }),
  missing: toolRow("codex", {
    pill: { label: "Not installed", look: "not_installed" },
    detail: "not installed",
    install: INSTALL,
    menu: [RECHECK],
  }),
  hidden: toolRow("opencode", {
    pill: { label: "Hidden", look: "hidden" },
    detail: "Found, hidden by you",
    show: "Show",
    menu: [SHOW, RECHECK],
  }),
};

const toolsPage = (rows: AgentToolRow[]): AgentToolsPage => ({
  title: "Agent tools",
  intro: "The AI tools already on your machine.",
  rows,
  recheck: "Re-check",
  checked: "Checked 2 minutes ago",
  menu_label: "More actions",
  not_shown_title: "Not shown here",
  not_shown: "git, gh and az stay under Tools: they aren’t agents.",
  error: null,
});

const drawRow = (row: AgentToolRow) =>
  html(
    createElement(AgentToolsPageBody, {
      page: toolsPage([row]),
      onAction: () => undefined,
      onRecheck: () => undefined,
    }),
  );

describe("Settings → Agent tools", () => {
  it("each state shows its pill and its own actions", () => {
    const connected = drawRow(ROWS.connected);
    expect(connected).toContain('data-look="connected"');
    expect(connected).toContain("v0.157.1 · signed in with ChatGPT");
    expect(connected).toContain("permission prompts");
    expect(connected).toContain("limits not reported");
    expect(connected).toContain("Otaq added: trust for your projects");
    expect(connected).toContain("Trust for your projects stays when you disconnect.");
    expect(connected).toContain('class="agent-tool-failed" role="alert"');
    expect(connected).toContain('class="agent-tool-warning" role="note"');
    expect(connected).not.toContain("Sign in…");

    const signedOut = drawRow(ROWS.signed_out);
    expect(signedOut).toContain('data-look="not_signed_in"');
    expect(signedOut).toContain("Sign in…");

    const update = drawRow(ROWS.update);
    expect(update).toContain("Update GitHub Copilot to a version Otaq runs");
    expect(update).toContain("npm install -g @github/copilot");

    const missing = drawRow(ROWS.missing);
    expect(missing).toContain("How to install (macOS)");
    expect(missing).toContain("brew install codex");
    expect(missing).not.toContain("Otaq can see");

    const hidden = drawRow(ROWS.hidden);
    expect(hidden).toContain("Found, hidden by you · ");
    expect(hidden).toContain(">Show</button>");

    const page = drawRow(ROWS.connected);
    expect(page).toContain("Re-check");
    expect(page).toContain("Checked 2 minutes ago");
    expect(page).toContain("git, gh and az stay under Tools");
  });

  it("no state is drawn turquoise: the warning, a failed setup and the found states are amber", () => {
    const all = Object.values(ROWS).map(drawRow).join("");
    expect(all).not.toMatch(/needs-you|attention-bg|--st-needs/);
    expect(markOf(ROWS.signed_out)).toBe("attention");
    expect(markOf(ROWS.update)).toBe("attention");
    expect(markOf(ROWS.connected)).toBe("connected");
    expect(markOf(ROWS.missing)).toBe("missing");
    expect(markOf(ROWS.hidden)).toBe("quiet");
  });

  it("each action sends its request; Disconnect asks first", () => {
    expect(stepOf("codex", { kind: "sign_in" })).toEqual({ kind: "sign_in", params: { tool: "codex" } });
    expect(stepOf("opencode", { kind: "show" })).toEqual({
      kind: "page",
      method: "agent_tools.set_hidden",
      params: { tool: "opencode", hidden: false },
    });
    expect(stepOf("codex", { kind: "menu", action: "hide" })).toEqual({
      kind: "page",
      method: "agent_tools.set_hidden",
      params: { tool: "codex", hidden: true },
    });
    expect(stepOf("codex", { kind: "menu", action: "recheck" })).toEqual({
      kind: "page",
      method: "agent_tools.recheck",
      params: { tool: "codex" },
    });
    // Never `agent_tools.disconnect` straight from the menu: the dialog comes first.
    expect(stepOf("codex", { kind: "menu", action: "disconnect" })).toEqual({
      kind: "ask_disconnect",
      params: { tool: "codex" },
    });
    expect(stepOf("claude", { kind: "docs" })).toEqual({ kind: "docs", params: { tool: "claude" } });
    const menu = agentToolMenu(ROWS.connected.menu);
    expect(menu.map((m) => m.id)).toEqual(["hide", "recheck", "disconnect"]);
    expect(menu[2]).toMatchObject({ danger: true, separated: true });
    expect(agentToolMenu(ROWS.hidden.menu).map((m) => m.id)).toEqual(["show", "recheck"]);
  });

  it("Disconnect's dialog: what it removes, what stays, and its refusal turns Disconnect off", () => {
    const dialog = {
      tool: "codex",
      title: "Disconnect Codex?",
      text: "Removes only what Otaq added (its hooks, otaq pre-approved, its skills).",
      stays: "Otaq added trust for your projects; it stays when you disconnect.",
      hidden: "Codex is then hidden: it doesn’t count until you show it again.",
      cancel: "Cancel",
      confirm: "Disconnect",
      error: null,
      can_confirm: true,
    };
    const props = { onCancel: () => undefined, onConfirm: () => undefined };
    const asks = html(createElement(DisconnectDialogView, { dialog, ...props }));
    expect(asks).toContain("Disconnect Codex?");
    expect(asks).toContain("it stays when you disconnect.");
    expect(asks).not.toMatch(/<button[^>]*disabled=""[^>]*>.*Disconnect<\/span>/);
    const refused = html(
      createElement(DisconnectDialogView, {
        dialog: { ...dialog, error: "Codex has 2 agents running. End them first.", can_confirm: false },
        ...props,
      }),
    );
    expect(refused).toContain("Codex has 2 agents running. End them first.");
    expect(refused).toMatch(/<button[^>]*disabled=""[^>]*>/);
  });
});

// ---- Settings → Workflow skills (T-0380; WSK-01, WSK-02, WSK-04, WSK-07; d0092, d0058) ----

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

// The shape `settings_page.rs` sends for an imported Commit (its test `workflow_skills_imported_row`).
const MY_COMMIT: SettingsSkillRow = {
  field: "workflow_skills.commit",
  step: "commit",
  label: "Commit",
  name: "my-commit",
  name_muted: false,
  tag: { label: "Imported", look: "quiet" },
  folder: "~/team-skills/my-commit",
  source: "· imported today, 10:42",
  lines: [
    {
      icon: "info",
      text: "Otaq uses its own copy. Changes to the original apply after Re-import.",
      look: "quiet",
      link: null,
    },
  ],
  button: { label: "Re-import", icon: "refresh-cw", action: "reimport", disabled: false },
  menu: [
    { action: "reveal", label: "Show Otaq’s copy in Finder", icon: "folder-open", separated: false },
    { action: "import", label: "Import a different folder…", icon: "folder-input", separated: false },
    { action: "reset", label: "Use built-in (otaq:commit)", icon: "rotate-ccw", separated: true },
  ],
  menu_label: "More for Commit",
  picker_title: "Import skill folder",
  reimport: {
    title: "Re-import my-commit?",
    text: "Otaq replaces its copy with the folder at ~/team-skills/my-commit. Sessions already running keep the copy they started with; the next launch uses the new one.",
    note: "Edits made to Otaq’s copy in ~/.otaq/skills/c1 are lost.",
    confirm: "Re-import",
    destructive: false,
  },
  reset: {
    title: "Use the built-in otaq:commit?",
    text: "Commit goes back to Otaq’s built-in skill and Otaq’s copy of my-commit is deleted. The original at ~/team-skills/my-commit isn’t touched.",
    note: "Otaq’s copy has edits that aren’t in the original. Going back to the built-in deletes them.",
    confirm: "Use built-in",
    destructive: true,
  },
};

const BUILT_IN: SettingsSkillRow = {
  ...MY_COMMIT,
  name: "otaq:commit",
  tag: { label: "Built-in", look: "quiet" },
  folder: null,
  source: "Otaq’s default",
  lines: [],
  button: { label: "Import skill folder…", icon: "folder-input", action: "import", disabled: false },
  menu: [],
  reimport: null,
  reset: null,
};

/** The same row with its copy missing: Import again in the red line; `again` is what it does. */
const missing = (again: "import_again" | "import"): SettingsSkillRow => ({
  ...MY_COMMIT,
  tag: { label: "Skill missing", look: "error" },
  lines: [
    {
      icon: "circle-alert",
      text: "Skill missing ·",
      look: "error",
      link: { label: "Import again", icon: null, action: again, disabled: false },
    },
  ],
  button: null,
});

describe("Settings → Workflow skills", () => {
  let root: Root;
  let box: HTMLElement;
  let calls: [string, Record<string, unknown>][];
  let answer: "done" | "confirm";

  const mount = (row: SettingsSkillRow) => {
    calls = [];
    answer = "done";
    box = document.createElement("div");
    document.body.append(box);
    root = createRoot(box);
    const value: SettingsContextValue = {
      page: PAGE,
      project: null,
      edit: () => undefined,
      act: () => undefined,
      skill: (method, params) => {
        calls.push([method, params]);
        return Promise.resolve(answer);
      },
    };
    act(() => {
      root.render(createElement(SettingsContext, { value }, createElement(SkillRowView, { row })));
    });
  };
  const unmount = () => {
    act(() => {
      root.unmount();
    });
    box.remove();
    document.body.innerHTML = "";
    vi.mocked(pickFolder).mockReset();
    vi.mocked(request).mockClear();
  };
  const settle = () =>
    act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
  const button = (text: string) => {
    const found = [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === text);
    if (!found) throw new Error(`no button ${text}`);
    return found;
  };
  const click = (target: Element) => {
    act(() => {
      (target as HTMLElement).click();
    });
  };
  const key = (target: Element, k: string) => {
    act(() => {
      target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
    });
  };
  const dialogTitle = () => document.querySelector(".dialog-title")?.textContent ?? null;

  afterEach(() => {
    if (box.isConnected) unmount();
  });

  it("draws the row from the view: name, tag, folder, the copy note, Re-import and ⋯", () => {
    mount(MY_COMMIT);
    const row = document.querySelector(".settings-skill");
    expect(row?.querySelector(".settings-skill-value")?.textContent).toBe("my-commit");
    expect(row?.querySelector(".settings-skill-tag")?.getAttribute("data-look")).toBe("quiet");
    expect(row?.querySelector(".settings-skill-folder")?.textContent).toBe("~/team-skills/my-commit");
    expect(row?.textContent).toContain("Changes to the original apply after Re-import.");
    expect(document.querySelector('[aria-label="More for Commit"]')).not.toBeNull();
    // "use your own Open PR skill" focuses the first button inside the row's actions.
    expect(row?.querySelector("#settings-field-workflow_skills-commit button")?.textContent).toContain(
      "Re-import",
    );
  });

  it("Import skill folder… opens the picker and sends the folder; a cancelled picker sends nothing", async () => {
    mount(BUILT_IN);
    vi.mocked(pickFolder).mockResolvedValueOnce(null);
    click(button("Import skill folder…"));
    await settle();
    expect(pickFolder).toHaveBeenCalledWith("Import skill folder");
    expect(calls).toEqual([]);
    vi.mocked(pickFolder).mockResolvedValueOnce("/tmp/m27/team-skills/my-commit");
    click(button("Import skill folder…"));
    await settle();
    expect(calls).toEqual([
      ["settings.skill_import", { project: null, step: "commit", folder: "/tmp/m27/team-skills/my-commit" }],
    ]);
    expect(dialogTitle()).toBeNull();
  });

  it("a refusal is the red line under the step; nothing else on the row changes", () => {
    mount({
      ...BUILT_IN,
      lines: [
        {
          icon: "circle-alert",
          text: "Not imported: ~/team-skills/commit-helpers has no SKILL.md.",
          look: "error",
          link: null,
        },
      ],
    });
    const line = document.querySelector('.settings-skill-line[data-look="error"]');
    expect(line?.textContent).toBe("Not imported: ~/team-skills/commit-helpers has no SKILL.md.");
    expect(document.querySelector(".settings-skill-tag")?.textContent).toBe("Built-in");
    expect(calls).toEqual([]);
  });

  it("Re-import asks first; Cancel sends nothing, the accent Re-import sends it", () => {
    mount(MY_COMMIT);
    click(button("Re-import"));
    expect(dialogTitle()).toBe("Re-import my-commit?");
    expect(document.querySelector(".dialog")?.textContent).toContain("Edits made to Otaq’s copy");
    expect(calls).toEqual([]);
    click(button("Cancel"));
    expect(dialogTitle()).toBeNull();
    expect(calls).toEqual([]);
    click(button("Re-import"));
    const confirm = [...document.querySelectorAll(".dialog button")].find(
      (b) => b.textContent.trim() === "Re-import",
    );
    expect(confirm?.getAttribute("data-variant")).toBe("primary");
    click(confirm as Element);
    expect(calls).toEqual([["settings.skill_reimport", { project: null, step: "commit" }]]);
  });

  it("a disabled Re-import (the original is gone) does nothing", () => {
    mount({
      ...MY_COMMIT,
      button: { label: "Re-import", icon: "refresh-cw", action: "reimport", disabled: true },
    });
    expect(button("Re-import").disabled).toBe(true);
    click(button("Re-import"));
    expect(dialogTitle()).toBeNull();
  });

  it("Use built-in goes back at once; with needs_confirmation the red dialog asks, then it is sent confirmed", async () => {
    mount(MY_COMMIT);
    click(document.querySelector('[aria-label="More for Commit"]') as Element);
    click(button("Use built-in (otaq:commit)"));
    await settle();
    expect(calls).toEqual([["settings.skill_reset", { project: null, step: "commit", confirmed: false }]]);
    expect(dialogTitle()).toBeNull();

    answer = "confirm";
    calls.length = 0;
    click(document.querySelector('[aria-label="More for Commit"]') as Element);
    click(button("Use built-in (otaq:commit)"));
    await settle();
    expect(dialogTitle()).toBe("Use the built-in otaq:commit?");
    const red = [...document.querySelectorAll(".dialog button")].find(
      (b) => b.textContent.trim() === "Use built-in",
    );
    expect(red?.getAttribute("data-variant")).toBe("destructive");
    click(red as Element);
    expect(calls).toEqual([
      ["settings.skill_reset", { project: null, step: "commit", confirmed: false }],
      ["settings.skill_reset", { project: null, step: "commit", confirmed: true }],
    ]);
  });

  it("Import again re-imports from the original, or opens the picker when it is gone", async () => {
    mount(missing("import_again"));
    expect(document.querySelector(".settings-skill-tag")?.getAttribute("data-look")).toBe("error");
    click(button("Import again"));
    await settle();
    expect(calls).toEqual([["settings.skill_reimport", { project: null, step: "commit" }]]);
    expect(pickFolder).not.toHaveBeenCalled();
    unmount();
    mount(missing("import"));
    vi.mocked(pickFolder).mockResolvedValueOnce("/tmp/m27/moved/my-commit");
    click(button("Import again"));
    await settle();
    expect(pickFolder).toHaveBeenCalledTimes(1);
    expect(calls).toEqual([
      ["settings.skill_import", { project: null, step: "commit", folder: "/tmp/m27/moved/my-commit" }],
    ]);
  });

  it("Show Otaq's copy opens it in the file manager", () => {
    mount(MY_COMMIT);
    click(document.querySelector('[aria-label="More for Commit"]') as Element);
    click(button("Show Otaq’s copy in Finder"));
    expect(request).toHaveBeenCalledWith("settings.reveal", {
      target: { skill: { project: null, step: "commit" } },
    });
  });

  it("the keyboard reaches every action and the menu (0058): buttons, ↓ ↵ in the menu, Esc back to ⋯", async () => {
    mount(MY_COMMIT);
    const actions = [...document.querySelectorAll<HTMLButtonElement>(".settings-skill button")];
    expect(actions.map((b) => b.getAttribute("aria-label") ?? b.textContent.trim())).toEqual([
      "Re-import",
      "More for Commit",
    ]);
    expect(actions.every((b) => b.tabIndex >= 0 && !b.disabled)).toBe(true);
    const more = actions[1] as HTMLButtonElement;
    click(more);
    const menu = document.querySelector('[role="menu"]') as HTMLElement;
    expect(document.activeElement).toBe(menu);
    key(menu, "Escape");
    expect(document.querySelector('[role="menu"]')).toBeNull();
    expect(document.activeElement).toBe(more);
    click(more);
    const again = document.querySelector('[role="menu"]') as HTMLElement;
    key(again, "ArrowDown");
    key(again, "ArrowDown");
    vi.mocked(pickFolder).mockResolvedValueOnce(null);
    key(again, "Enter");
    await settle();
    // Import a different folder… opens the picker.
    expect(pickFolder).toHaveBeenCalledWith("Import skill folder");
  });

  it("each action's plan and the menu rows", () => {
    expect(skillStep(MY_COMMIT, "import", "NOTE")).toEqual({
      kind: "pick",
      title: "Import skill folder",
      params: { project: "NOTE", step: "commit" },
    });
    expect(skillStep(MY_COMMIT, "reimport", null)).toEqual({ kind: "ask", dialog: "reimport" });
    expect(skillStep(MY_COMMIT, "reset", "NOTE")).toEqual({
      kind: "send",
      method: "settings.skill_reset",
      params: { project: "NOTE", step: "commit", confirmed: false },
    });
    const menu = skillMenu(MY_COMMIT.menu);
    expect(menu.map((m) => m.id)).toEqual(["reveal", "import", "reset"]);
    expect(menu[2]).toMatchObject({ separated: true, danger: false });
  });
});
