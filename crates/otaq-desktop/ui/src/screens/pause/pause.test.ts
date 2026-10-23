// @vitest-environment happy-dom
// Pause in the title bar (T-0457; PAU-08, d0112): the slot and ⇧⌘. are registered; the button draws Pause or Resume
// from Rust's view; the menu's keys move over This project and All projects and ↵ picks the highlighted one; a pick
// sends `pause.run` at once (no confirmation) with the open project for This project.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createRegistries } from "../../app/registry";
import type { PauseView } from "../../generated/PauseView";
import { pause } from ".";
import { closePauseMenu, menuKey, pauseMenu, runParams, togglePauseMenu } from "./logic";
import { PauseButton, PauseMenuBox } from "./Pause";

vi.mock("../../tauri", () => ({ request: vi.fn(), subscribeEvents: vi.fn(), windowAction: vi.fn() }));

/** `pause.rs`'s words for shop-web paused, billing-api paused too (`s24-08`). */
const RESUME: PauseView = {
  project: "SHOP",
  button: { label: "Resume", icon: "play", resumes: true, tooltip: "Pause / Resume agents", keys: "⇧⌘." },
  menu: {
    title: "Resume agents",
    resumes: true,
    rows: [
      { scope: "project", icon: "folder", label: "This project", detail: "shop-web · 5 paused", keys: "⇧⌘." },
      { scope: "all", icon: "layers", label: "All projects", detail: "7 paused in 2 projects", keys: null },
    ],
    note: "Resume this project only and billing-api stays paused, with its own banner.",
  },
};

describe("pause", () => {
  it("registers the title bar's slot and ⇧⌘.", () => {
    const app = createRegistries();
    pause(app);
    expect(app.slots.get("header_pause")).toBeDefined();
    expect(app.shortcuts.names()).toEqual(["pause"]);
  });

  it("⇧⌘. and the button toggle the menu", () => {
    closePauseMenu();
    togglePauseMenu();
    expect(pauseMenu.get()).toBe(true);
    togglePauseMenu();
    expect(pauseMenu.get()).toBe(false);
  });

  it("draws Pause as an icon and Resume with its label", () => {
    const pauseHtml = renderToStaticMarkup(
      createElement(PauseButton, {
        button: {
          label: "Pause",
          icon: "pause",
          resumes: false,
          tooltip: "Pause / Resume agents",
          keys: "⇧⌘.",
        },
      }),
    );
    expect(pauseHtml).toContain('aria-label="Pause"');
    expect(pauseHtml).not.toContain("data-resumes");
    const resumeHtml = renderToStaticMarkup(createElement(PauseButton, { button: RESUME.button }));
    expect(resumeHtml).toContain("data-resumes");
    expect(resumeHtml).toContain(">Resume<");
  });

  it("draws the menu as Rust words it, This project highlighted", () => {
    const html = renderToStaticMarkup(createElement(PauseMenuBox, { menu: RESUME.menu, cursor: 0 }));
    expect(html).toContain("Resume agents");
    expect(html).toContain("shop-web · 5 paused");
    expect(html).toContain("7 paused in 2 projects");
    expect(html).toContain("billing-api stays paused");
    expect(html.indexOf("data-highlighted")).toBeLessThan(html.indexOf("All projects"));
  });

  it("↑ ↓ move over the two rows and ↵ picks", () => {
    expect(menuKey("ArrowDown", 0, 2)).toEqual({ kind: "move", index: 1 });
    expect(menuKey("ArrowDown", 1, 2)).toEqual({ kind: "move", index: 0 });
    expect(menuKey("ArrowUp", 0, 2)).toEqual({ kind: "move", index: 1 });
    expect(menuKey("Enter", 0, 2)).toEqual({ kind: "pick", index: 0 });
    expect(menuKey("a", 0, 2)).toEqual({ kind: "pass" });
  });

  it("a pick resumes or pauses its scope, This project with the open project", () => {
    const [here, all] = RESUME.menu.rows;
    if (!here || !all) throw new Error("two rows");
    expect(runParams(here, RESUME)).toEqual({ scope: "project", project: "SHOP", resume: true });
    expect(runParams(all, { ...RESUME, project: null })).toEqual({ scope: "all", resume: true });
  });
});
