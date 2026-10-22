// @vitest-environment happy-dom
// A floating layer and a scroll behind it: mounted in a DOM, inside a scrolling container.
import { act, createElement, createRef } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import type { MenuItem } from "../generated/MenuItem";
import { useFloatingStyle } from "./Floating";
import { AnchoredMenu } from "./Menu";
import { Select } from "./Select";

beforeAll(() => {
  (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
});

let unmount = (): void => undefined;

afterEach(() => {
  unmount();
});

function item(id: string, checked = false): MenuItem {
  return {
    id,
    label: id,
    icon: null,
    shortcut: null,
    danger: false,
    disabled: false,
    hint: null,
    checked,
    separated: false,
    header: false,
  };
}

/** Mounts a Select inside a scrolling container, opens it and returns the container. */
function openSelect(onChange: (id: string) => void): HTMLElement {
  unmount();
  const host = document.body.appendChild(document.createElement("div"));
  const root = createRoot(host);
  act(() => {
    root.render(
      createElement(
        "div",
        { className: "scroller", style: { overflow: "auto", height: "100px" } },
        createElement(Select, {
          label: "Developer",
          value: "Claude Code",
          items: [item("claude", true), item("codex")],
          onChange,
        }),
      ),
    );
  });
  unmount = () => {
    act(() => {
      root.unmount();
    });
    host.remove();
  };
  act(() => {
    host.querySelector<HTMLButtonElement>(".select")?.click();
  });
  expect(host.querySelector('[role="menu"]')).not.toBeNull();
  return host;
}

function scroll(target: EventTarget) {
  act(() => {
    target.dispatchEvent(new Event("scroll"));
  });
}

describe("A floating layer and a scroll", () => {
  it("closes a Select's list when its scrolling container scrolls, changing nothing", () => {
    const onChange = vi.fn();
    const host = openSelect(onChange);
    scroll(host.querySelector(".scroller") as HTMLElement);
    expect(host.querySelector('[role="menu"]')).toBeNull();
    expect(host.querySelector(".select")?.getAttribute("aria-expanded")).toBe("false");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("closes it when the page itself scrolls", () => {
    const host = openSelect(vi.fn());
    scroll(document);
    expect(host.querySelector('[role="menu"]')).toBeNull();
  });

  it("keeps it open when the list itself scrolls", () => {
    const host = openSelect(vi.fn());
    scroll(host.querySelector('[role="menu"]') as HTMLElement);
    expect(host.querySelector('[role="menu"]')).not.toBeNull();
  });
});

describe("A menu and a press outside it", () => {
  it("closes on a press outside that moves no focus (the header's drag region)", () => {
    unmount();
    const host = document.body.appendChild(document.createElement("div"));
    const outside = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    const onClose = vi.fn();
    const anchor = { current: null as HTMLElement | null };
    act(() => {
      root.render(
        createElement(AnchoredMenu, {
          anchor,
          label: "Project",
          items: [item("copy_path")],
          onChoose: vi.fn(),
          onClose,
        }),
      );
    });
    unmount = () => {
      act(() => {
        root.unmount();
      });
      host.remove();
      outside.remove();
    };
    act(() => {
      host.querySelector(".menu-item")?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    });
    expect(onClose).not.toHaveBeenCalled();
    act(() => {
      outside.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    });
    expect(onClose).toHaveBeenCalledWith("outside");
  });
});

/** A floating layer of its own, as a new dropdown would write it: placed with `useFloatingStyle` and its `onDismiss`. */
const ownAnchor = createRef<HTMLButtonElement>();
const ownFloating = createRef<HTMLDivElement>();

function OwnLayer({ onClose }: { onClose: () => void }) {
  const anchor = ownAnchor;
  const floating = ownFloating;
  const style = useFloatingStyle(anchor, floating, "start", onClose);
  return createElement(
    "div",
    null,
    createElement("button", { ref: anchor, className: "own-trigger" }),
    createElement("div", { ref: floating, className: "own-layer", style }),
  );
}

describe("Any floating layer that can close and a press outside it", () => {
  it("closes once on a press outside, never on a press on the layer or its trigger", () => {
    unmount();
    const host = document.body.appendChild(document.createElement("div"));
    const outside = document.body.appendChild(document.createElement("div"));
    const root = createRoot(host);
    const onClose = vi.fn();
    act(() => {
      root.render(createElement(OwnLayer, { onClose }));
    });
    unmount = () => {
      act(() => {
        root.unmount();
      });
      host.remove();
      outside.remove();
    };
    const press = (target: Element | null) => {
      act(() => {
        target?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      });
    };
    press(host.querySelector(".own-layer"));
    press(host.querySelector(".own-trigger"));
    expect(onClose).not.toHaveBeenCalled();
    // A handler that stops the press (bubbling) can't keep the layer open.
    outside.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
    });
    press(outside);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
