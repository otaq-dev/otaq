import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it } from "vitest";

import { overlays } from "../../app/overlays";
import { registerScreens } from "../../app/registry";
import type { AddProjectView } from "../../generated/AddProjectView";
import type { OnboardingTools } from "../../generated/OnboardingTools";
import type { ServiceConnection } from "../../generated/ServiceConnection";
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
import { ToolsList } from "./Tools";

afterEach(() => {
  overlays.store.set([]);
  droppedFolder.set(null);
});

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

const TOOLS: OnboardingTools = {
  title: "Tools",
  text: "Crewboard uses the tools already on your machine.",
  warning: null,
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
};

describe("the onboarding screen", () => {
  it("registers onboarding and Add project", () => {
    const app = registerScreens({ onboarding });
    expect(app.overlays.ids().toSorted()).toEqual(["add_project", "onboarding"]);
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
      }),
    );
    expect(html).toContain("Install claude to continue");
    expect(html).toContain("npm install -g @anthropic-ai/claude-code");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Continue/);
    expect(html).toContain('data-look="missing_required"');
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
