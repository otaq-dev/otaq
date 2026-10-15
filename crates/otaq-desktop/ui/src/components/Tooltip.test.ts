// @vitest-environment happy-dom
// The tooltip shows on focus, so it is mounted in a DOM.
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import { Tooltip, type TooltipProps } from "./Tooltip";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let unmount = (): void => undefined;

afterEach(() => {
  unmount();
  vi.useRealTimers();
});

/** Mounts a tooltip around a button, focuses the button and returns the bubble. */
function bubble(props: Omit<TooltipProps, "children">): HTMLElement | null {
  unmount();
  vi.useFakeTimers();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  act(() => {
    root.render(
      createElement(Tooltip, { ...props, children: createElement("button", { type: "button" }, "x") }),
    );
  });
  unmount = () => {
    act(() => {
      root.unmount();
    });
    host.remove();
  };
  act(() => {
    host.querySelector("button")?.focus();
  });
  act(() => {
    vi.advanceTimersByTime(0);
  });
  return host.querySelector<HTMLElement>('[role="tooltip"]');
}

describe("Tooltip", () => {
  it("without an icon shows the text and the shortcut, as before", () => {
    expect(bubble({ text: "Save", keys: "⌘S" })?.innerHTML).toBe(
      '<span>Save</span><kbd class="tooltip-keys">⌘S</kbd>',
    );
    expect(bubble({ text: "Expand" })?.innerHTML).toBe("<span>Expand</span>");
  });

  it("with an icon shows it before the text, hidden from screen readers", () => {
    const tip = bubble({ text: "Copied", icon: "check" });
    const [icon, text] = Array.from(tip?.children ?? []);
    expect(icon?.className).toBe("icon");
    expect(icon?.getAttribute("aria-hidden")).toBe("true");
    expect((icon as HTMLElement | undefined)?.style.maskImage).toContain("lucide-check'");
    expect(text?.outerHTML).toBe("<span>Copied</span>");
    expect(tip?.textContent).toBe("Copied");
  });
});
