// @vitest-environment happy-dom
// The side panel's wait for its content before it slides in (0095).
import { afterEach, describe, expect, it, vi } from "vitest";

import { ENTER_WAIT, playEnterWhenReady } from "./motion";

afterEach(() => {
  vi.useRealTimers();
  document.body.replaceChildren();
});

function layer(loading: boolean): { el: HTMLElement; content: HTMLElement } {
  const el = document.body.appendChild(document.createElement("div"));
  const content = el.appendChild(document.createElement("div"));
  if (loading) content.setAttribute("data-loading", "");
  return { el, content };
}

describe("playEnterWhenReady", () => {
  it("comes in at once when its content is ready", () => {
    const { el } = layer(false);
    playEnterWhenReady(el, "panel");
    expect(el.style.visibility).toBe("");
  });

  it("stays hidden while its content loads, and shows once it has loaded", async () => {
    const { el, content } = layer(true);
    playEnterWhenReady(el, "panel");
    expect(el.style.visibility).toBe("hidden");
    content.removeAttribute("data-loading");
    await Promise.resolve();
    expect(el.style.visibility).toBe("");
  });

  it("comes in anyway when the content takes too long", () => {
    vi.useFakeTimers();
    const { el } = layer(true);
    playEnterWhenReady(el, "panel");
    vi.advanceTimersByTime(ENTER_WAIT);
    expect(el.style.visibility).toBe("");
  });

  it("shows the layer when it stops waiting", () => {
    const { el } = layer(true);
    const stop = playEnterWhenReady(el, "panel");
    stop();
    expect(el.style.visibility).toBe("");
  });
});
