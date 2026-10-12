import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { CAPTION_BUTTONS, TitleBarEnd, TitleBarStart } from ".";
import type { WindowChrome } from "../../generated/WindowChrome";

const start = (chrome: WindowChrome | undefined) =>
  renderToStaticMarkup(createElement(TitleBarStart, { chrome }));
const end = (chrome: WindowChrome | undefined) =>
  renderToStaticMarkup(createElement(TitleBarEnd, { chrome }));
const buttons = (html: string) => [...html.matchAll(/aria-label="([A-Za-z]+)"/g)].map((m) => m[1]);

describe("the title bar", () => {
  it("on macOS keeps room for the traffic lights and draws no buttons", () => {
    expect(start("traffic_lights")).toBe(
      '<div class="title-bar-start title-bar-start-traffic_lights"></div>',
    );
    expect(buttons(end("traffic_lights"))).toEqual([]);
  });

  it("on Windows and Linux draws minimize, maximize and close at the end, after a divider", () => {
    expect(start("caption_buttons")).toBe(
      '<div class="title-bar-start title-bar-start-caption_buttons"></div>',
    );
    const html = end("caption_buttons");
    expect(html).toMatch(/^<div class="title-bar-end"><div class="title-bar-divider"><\/div>/);
    expect(buttons(html)).toEqual(["Minimize", "Maximize", "Close"]);
    expect(html.match(/<button type="button"/g)).toHaveLength(3);
  });

  it("draws nothing until the Rust side says which controls the OS has", () => {
    expect(start(undefined)).toBe("");
    expect(end(undefined)).toBe("");
  });

  it("each caption button has its window action and glyph", () => {
    expect(CAPTION_BUTTONS.map((b) => [b.action, b.icon])).toEqual([
      ["minimize", "minus"],
      ["toggle_maximize", "square"],
      ["close", "x"],
    ]);
  });
});
