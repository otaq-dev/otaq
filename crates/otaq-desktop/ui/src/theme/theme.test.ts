import { describe, expect, it, vi } from "vitest";

import type { ActiveTheme } from "../generated/ActiveTheme";
import type { PageEvent } from "../generated/PageEvent";
import { createPageEvents } from "../service";
import { request } from "../tauri";
import { COLOR_TOKENS, FIRST_LAUNCH, THEME_KEYS } from "./generated/tokens";
import { ICON_COUNT, iconUrl } from "./icons";
import { followTheme, terminalColors, watchTheme } from "./theme";

vi.mock("../tauri", () => ({ request: vi.fn(), subscribeEvents: vi.fn() }));
const requestMock = vi.mocked(request);

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const theme = (key: string, dark: boolean): ActiveTheme => ({ key, name: key, dark });

describe("the active theme", () => {
  it("is asked with the OS appearance, and again on each settings change", async () => {
    let send: (event: PageEvent) => void = () => undefined;
    const events = createPageEvents((onEvent) => {
      send = onEvent;
      return Promise.resolve();
    });
    requestMock
      .mockResolvedValueOnce(theme(FIRST_LAUNCH.dark, true))
      .mockResolvedValueOnce(theme("daylight", false));
    let osDark = true;
    const seen: string[] = [];
    const stop = watchTheme(
      (t) => seen.push(t.key),
      () => osDark,
      events,
    );
    await settle();
    expect(requestMock).toHaveBeenLastCalledWith("theme.active", { os_dark: true });

    osDark = false;
    send({ kind: "service", name: "ticket_changed", event: {} });
    send({ kind: "service", name: "settings_changed", event: {} });
    await settle();
    expect(requestMock).toHaveBeenCalledTimes(2);
    expect(requestMock).toHaveBeenLastCalledWith("theme.active", { os_dark: false });
    expect(seen).toEqual([FIRST_LAUNCH.dark, "daylight"]);
    stop();
  });
});

describe("the page's theme", () => {
  it("starts on the OS's first-launch theme and recolors on each settings change, without a reload", async () => {
    let send: (event: PageEvent) => void = () => undefined;
    const events = createPageEvents((onEvent) => {
      send = onEvent;
      return Promise.resolve();
    });
    requestMock.mockReset();
    requestMock
      .mockResolvedValueOnce(theme(FIRST_LAUNCH.light, false))
      .mockResolvedValueOnce(theme("tokyo-night", true));
    const root = { dataset: {} as DOMStringMap };
    const stop = followTheme(
      root,
      () => undefined,
      () => false,
      events,
    );
    expect(root.dataset.theme).toBe(FIRST_LAUNCH.light);
    await settle();
    expect(root.dataset.theme).toBe(FIRST_LAUNCH.light);

    send({ kind: "service", name: "settings_changed", event: {} });
    await settle();
    expect(root.dataset.theme).toBe("tokyo-night");
    stop();
  });
});

describe("the tokens", () => {
  it("list the 49 colours and the 15 themes", () => {
    expect(COLOR_TOKENS).toHaveLength(49);
    expect(THEME_KEYS).toHaveLength(15);
    expect(THEME_KEYS).toContain(FIRST_LAUNCH.dark);
    expect(THEME_KEYS).toContain(FIRST_LAUNCH.light);
  });

  it("give a terminal its colours by xterm.js's names", () => {
    const vars: Record<string, string> = {
      "--term-bg": " #0a0b0d",
      "--term-fg": "#d6d8dd",
      "--ansi-9": "#ff8a8f",
    };
    const colors = terminalColors({ getPropertyValue: (name) => vars[name] ?? "" });
    expect(colors.background).toBe("#0a0b0d");
    expect(colors.cursor).toBe("#d6d8dd");
    expect(colors.brightRed).toBe("#ff8a8f");
    expect(colors.black).toBe("");
  });

  it("give every theme's selected text its terminal text colour on the selection tint, focused or not", async () => {
    // Read as text with Node's fs (the test runner hands CSS imports over empty; the page has no Node types).
    const fsModule = "node:fs";
    const fs = (await import(fsModule)) as { readFileSync(path: URL, encoding: "utf8"): string };
    const tokensCss = fs.readFileSync(new URL("generated/tokens.css", import.meta.url), "utf8");
    const themes = [...tokensCss.matchAll(/\[data-theme="([^"]+)"\] \{([^}]*)\}/g)];
    expect(themes.map((t) => t[1])).toEqual([...THEME_KEYS]);
    for (const [, key, body] of themes) {
      const vars = new Map([...(body ?? "").matchAll(/(--[\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2]]));
      const colors = terminalColors({ getPropertyValue: (name) => vars.get(name) ?? "" });
      expect(colors.selectionForeground, key).toBe(vars.get("--term-fg"));
      expect(colors.selectionForeground, key).toMatch(/^#[0-9a-f]{6}$/);
      expect(colors.selectionBackground, key).toBe(vars.get("--selection"));
      expect(colors.selectionInactiveBackground, key).toBe(vars.get("--selection"));
      expect(colors.selectionInactiveBackground, key).toMatch(/^#[0-9a-f]{6,8}$/);
    }
  });
});

describe("the icons", () => {
  it("are the bundled SVG files, by their Lucide names", () => {
    expect(ICON_COUNT).toBeGreaterThanOrEqual(70);
    expect(iconUrl("bell")).toMatch(/bell/);
    expect(iconUrl("message-circle-question-mark")).toBeDefined();
  });
});
