import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { registerScreens } from "../../app/registry";
import type { AddProjectView } from "../../generated/AddProjectView";
import type { AgentToolRow } from "../../generated/AgentToolRow";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { ServiceConnection } from "../../generated/ServiceConnection";
import type { UsageDataDialog } from "../../generated/UsageDataDialog";
import { droppedFolder, folderDropped } from "./drops";
import { onboarding, openOnboardingWhenNeeded } from ".";
import {
  adoptKey,
  afterTools,
  baseText,
  finishOnboarding,
  keyText,
  newForm,
  typeBase,
  typeKey,
} from "./logic";
import { AddButton, AddProjectFields, staticAddProject } from "./AddProjectForm";
import { ToolsList } from "./Tools";
import { askUsageDataWhenNeeded } from "./UsageData";

afterEach(() => {
  overlays.store.set([]);
  droppedFolder.set(null);
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const TOOLS: OnboardingTools = {
  title: "Tools",
  text: "Otaq uses the tools already on your machine.",
  warning: null,
  agent_tools: {
    heading: "AGENT TOOLS · AT LEAST ONE",
    count: "none connected",
    rows: [],
    blocked: null,
    menu_label: "More actions",
  },
  other_heading: "OTHER TOOLS",
  rows: [
    {
      name: "claude",
      badge: "required",
      look: "missing_required",
      icon: "circle-x",
      status: "not found on PATH",
      explanation: "nothing can start without it.",
      command: "npm install -g @anthropic-ai/claude-code",
    },
  ],
  waiting: null,
  error: null,
  recheck: "Re-check",
  continue_label: "Continue",
  continue_enabled: false,
  continue_reason: "Install claude to continue",
  has_projects: false,
  copy: "Copy",
  agents_line: null,
};

describe("the onboarding screen", () => {
  it("registers onboarding, Add project and Help improve Otaq", () => {
    const app = registerScreens({ onboarding });
    expect(app.overlays.ids().toSorted()).toEqual(["add_project", "onboarding", "usage_data"]);
  });

  it("opens once, after the first connection, when no project is stored", async () => {
    const asked: string[] = [];
    let push: (c: ServiceConnection) => void = () => undefined;
    const watch = (onChange: (c: ServiceConnection) => void) => {
      push = onChange;
      return () => undefined;
    };
    openOnboardingWhenNeeded((method) => {
      asked.push(method);
      return Promise.resolve({ show: true });
    }, watch);
    push({ state: "reconnecting", attempt: 1 });
    expect(asked).toEqual([]);
    const connected = {
      state: "connected",
      build_id: "x",
      compatibility: "same",
      agents_working: 0,
      agents_waiting: 0,
    } as ServiceConnection;
    push(connected);
    push(connected);
    await flush();
    expect(asked).toEqual(["onboarding.start"]);
    expect(overlays.store.get()).toEqual([{ id: "onboarding", params: undefined }]);
  });

  it("quitting during the Flow step reopens it: onboarding opens on that project's Flow step (ASK-04)", async () => {
    let push: (c: ServiceConnection) => void = () => undefined;
    openOnboardingWhenNeeded(
      () => Promise.resolve({ show: true, step: "flow", project: "SHOP" }),
      (onChange) => {
        push = onChange;
        return () => undefined;
      },
    );
    push({ state: "connected" } as ServiceConnection);
    await flush();
    expect(overlays.store.get()).toEqual([{ id: "onboarding", params: { step: "flow", project: "SHOP" } }]);
  });

  it("stays closed when a project exists", async () => {
    let push: (c: ServiceConnection) => void = () => undefined;
    openOnboardingWhenNeeded(
      () => Promise.resolve({ show: false }),
      (onChange) => {
        push = onChange;
        return () => undefined;
      },
    );
    push({ state: "connected" } as ServiceConnection);
    await flush();
    expect(overlays.store.get()).toEqual([]);
  });

  it("asks about usage data once a user with projects starts, until answered (d0085)", async () => {
    const dialog = { title: "Help improve Otaq" } as UsageDataDialog;
    let push: (c: ServiceConnection) => void = () => undefined;
    const asked: string[] = [];
    openOnboardingWhenNeeded(
      (method) => {
        asked.push(method);
        return Promise.resolve(method === "usage.dialog" ? { show: true, dialog } : { show: false });
      },
      (onChange) => {
        push = onChange;
        return () => undefined;
      },
    );
    push({ state: "connected" } as ServiceConnection);
    await flush();
    await flush();
    expect(asked).toEqual(["onboarding.start", "usage.dialog"]);
    expect(overlays.store.get()).toEqual([{ id: "usage_data", params: dialog }]);

    // Answered: it doesn't open again.
    overlays.store.set([]);
    const opened = await askUsageDataWhenNeeded(
      () => Promise.resolve({ show: false, dialog }),
      () => {
        throw new Error("must not open");
      },
    );
    expect(opened).toBe(false);
  });

  it("a dropped folder opens Add project, or goes to onboarding's project step", () => {
    folderDropped("/code/shop-web");
    expect(overlays.store.get()).toEqual([{ id: "add_project", params: { folder: "/code/shop-web" } }]);
    overlays.store.set([{ id: "onboarding", params: undefined }]);
    folderDropped("/code/billing-api");
    folderDropped("/code/billing-api");
    expect(droppedFolder.get()).toEqual({ path: "/code/billing-api", seq: 2 });
    expect(overlays.store.get().map((o) => o.id)).toEqual(["onboarding"]);
  });
});

describe("the steps", () => {
  it("Continue goes to the project step, or ends for a user with projects", () => {
    expect(afterTools(TOOLS)).toBe("project");
    expect(afterTools({ ...TOOLS, has_projects: true })).toBe("done");
    // Back to Tools from the Flow step: Continue returns to it, the project still in Flow setup.
    expect(afterTools({ ...TOOLS, has_projects: true }, "SHOP")).toBe("flow");
  });

  it("ending raises the permission banner only when the warning showed", async () => {
    const asked: string[] = [];
    const ask = (method: string) => {
      asked.push(method);
      return Promise.resolve(null);
    };
    expect(await finishOnboarding(false, ask)).toBe(false);
    expect(asked).toEqual([]);
    expect(await finishOnboarding(true, ask)).toBe(true);
    expect(asked).toEqual(["notices.permission_warning"]);
  });

  it("a missing claude blocks Continue, with the reason and the copyable command", () => {
    const html = renderToStaticMarkup(
      createElement(ToolsList, {
        tools: TOOLS,
        checking: false,
        warning: false,
        onDismiss: () => undefined,
        onRecheck: () => undefined,
        onContinue: () => undefined,
        onAgentAction: () => undefined,
      }),
    );
    expect(html).toContain("Install claude to continue");
    expect(html).toContain("npm install -g @anthropic-ai/claude-code");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Continue/);
    expect(html).toContain('data-look="missing_required"');
  });
});

// An agent tool's row as `agent_tools_page::row` sends it for the Tools step (no pill, no Disconnect…).
const agentRow = (tool: string, name: string, more: Partial<AgentToolRow>): AgentToolRow => ({
  tool,
  avatar: { tool: "codex", letters: "CX", background: "tool-cx", foreground: "tool-cx-fg", hover: name },
  name,
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
  menu: [{ action: "recheck", label: "Re-check this tool", destructive: false }],
  ...more,
});

describe("the agent tools on the Tools step", () => {
  const render = (tools: OnboardingTools) =>
    renderToStaticMarkup(
      createElement(ToolsList, {
        tools,
        checking: false,
        warning: false,
        onDismiss: () => undefined,
        onRecheck: () => undefined,
        onContinue: () => undefined,
        onAgentAction: () => undefined,
      }),
    );

  it("none connected: the blocking line, each install command, and Continue off with its reason", () => {
    const install = {
      label: "How to install (macOS)",
      command: "brew install codex",
      copy: "Copy",
    };
    const html = render({
      ...TOOLS,
      rows: [],
      continue_reason: "Connect one agent tool to continue",
      agent_tools: {
        ...TOOLS.agent_tools,
        blocked:
          "No agent tool is connected. Install and sign in to at least one; agents can’t start without it.",
        rows: [agentRow("codex", "Codex", { detail: "not installed", install })],
      },
    });
    expect(html).toContain('class="agent-tools-blocked" role="alert"');
    expect(html).toContain("none connected");
    expect(html).toContain('data-mark="missing"');
    expect(html).toContain("brew install codex");
    expect(html).toContain("Connect one agent tool to continue");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Continue/);
    expect(html).toContain("OTHER TOOLS");
  });

  it("one connected: Continue on; a signed-out tool offers Sign in…, a hidden one Show", () => {
    const html = render({
      ...TOOLS,
      rows: [],
      continue_enabled: true,
      continue_reason: null,
      agent_tools: {
        ...TOOLS.agent_tools,
        count: "1 connected · 1 hidden",
        rows: [
          agentRow("claude", "Claude Code", {
            detail: "v2.1.301 · signed in",
            can_see: { label: "Otaq can see", chips: [{ label: "prompts", seen: true }] },
          }),
          agentRow("copilot", "GitHub Copilot", { detail: "found · not signed in", sign_in: "Sign in…" }),
          agentRow("opencode", "OpenCode", { detail: "hidden by you", show: "Show" }),
        ],
      },
    });
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>.*Continue/);
    expect(html).toContain("1 connected · 1 hidden");
    expect(html.match(/data-mark="([a-z]+)"/g)).toEqual([
      'data-mark="connected"',
      'data-mark="attention"',
      'data-mark="quiet"',
    ]);
    expect(html).toContain("Sign in…");
    expect(html).toContain("hidden by you · ");
    expect(html).toContain(">Show</button>");
    expect(html).not.toContain("Disconnect");
  });
});

describe("the Add project form", () => {
  const view = (key: string, base = "main") =>
    ({ key: { value: key }, base: { value: base } }) as unknown as AddProjectView;

  it("shows Rust's suggestion until the user types, then what they typed", () => {
    const form = newForm(true, "/code/shop-web");
    expect(form).toEqual({
      folder: "/code/shop-web",
      key: "",
      key_edited: false,
      base_branch: "",
      base_edited: false,
      first_run: true,
    });
    expect(keyText(form, view("SHOP"))).toBe("SHOP");
    const typed = typeKey(form, "sh");
    expect(keyText(typed, view("SHOP"))).toBe("sh");
    expect(baseText(form, view("SHOP", "develop"))).toBe("develop");
    expect(baseText(typeBase(form, ""), view("SHOP", "develop"))).toBe("");
  });

  it("takes Rust's spelling of the key, unless the user typed on meanwhile", () => {
    const asked = typeKey(newForm(false), "shop");
    expect(adoptKey(asked, asked, view("SHOP")).key).toBe("SHOP");
    const later = typeKey(asked, "shopx");
    expect(adoptKey(later, asked, view("SHOP"))).toBe(later);
    const untouched = newForm(false);
    expect(adoptKey(untouched, untouched, view("SHOP"))).toBe(untouched);
  });
});

describe("the folder trust in the Add project form", () => {
  const LINE =
    "Adding a project also lets Claude Code work in this folder (its hooks and settings will run).";
  const FORM: AddProjectView = {
    first_run: false,
    title: "Add project",
    text: null,
    folder: "~/code/pocket-notes",
    folder_look: "valid",
    choose: "Change…",
    choose_quiet: true,
    picker_title: "Add project",
    error: null,
    repo_path: "/code/pocket-notes",
    name: null,
    base: null,
    key: null,
    footnote: null,
    add: "Add project",
    adding: "Adding…",
    can_add: true,
    add_error: null,
    cancel: "Cancel",
    trust_line: LINE,
    trust_error: null,
    continue_label: "Continue",
  };
  const draw = (view: AddProjectView) => {
    const state = staticAddProject(view);
    return renderToStaticMarkup(
      createElement(
        "div",
        null,
        createElement(AddProjectFields, { state }),
        createElement(AddButton, { state }),
      ),
    );
  };

  it("shows the line under the folder, muted, and Add", () => {
    const html = draw(FORM);
    expect(html).toContain('<span class="add-project-trust"><span class="icon"');
    expect(html).toContain(`<span>${LINE}</span>`);
    expect(html).not.toContain('role="alert"');
    expect(html).toContain("Add project");
    expect(draw({ ...FORM, trust_line: null })).not.toContain("add-project-trust");
  });

  it("after a failed trust: the reason in the inline error style, and Continue instead of Add", () => {
    const html = draw({
      ...FORM,
      can_add: false,
      trust_error:
        "Claude Code’s trust couldn’t be recorded: why. Its agents will wait on Claude Code’s trust screen.",
    });
    expect(html).toMatch(
      /<span class="add-project-failure" role="alert">Claude Code’s trust couldn’t be recorded/,
    );
    expect(html).toMatch(/<button[^>]*>.*Continue/);
    expect(html).not.toMatch(/disabled=""[^>]*>.*Continue/);
    expect(html).not.toContain("Adding…");
  });
});
