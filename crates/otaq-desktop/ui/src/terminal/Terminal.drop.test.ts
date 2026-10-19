// @vitest-environment happy-dom
// The terminal component registers itself for drops (d0099, T-0388): found from its well while mounted, pasting and
// taking the keyboard only while live and typeable, gone once unmounted.
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { TerminalFeed } from "../generated/TerminalFeed";
import { createRegistries, RegistriesContext } from "../app/registry";
import { dropTargets } from "./drops";
import { Terminal } from "./Terminal";

const xterm = vi.hoisted(() => ({ pasted: [] as string[], focused: 0 }));
const feeds = vi.hoisted(() => [] as ((feed: TerminalFeed) => void)[]);

// xterm.js needs a canvas: a stand-in with what the component calls.
vi.mock("@xterm/xterm", () => {
  const disposable = () => ({ dispose: () => undefined });
  class Terminal {
    options: Record<string, unknown> = {};
    cols = 80;
    rows = 24;
    element = undefined;
    textarea = undefined;
    buffer = { active: { baseY: 0, viewportY: 0 } };
    loadAddon = () => undefined;
    onData = disposable;
    onBinary = disposable;
    onResize = disposable;
    onScroll = disposable;
    onLineFeed = disposable;
    onWriteParsed = disposable;
    attachCustomKeyEventHandler = () => undefined;
    open = () => undefined;
    reset = () => undefined;
    write = () => undefined;
    dispose = () => undefined;
    paste = (text: string) => {
      xterm.pasted.push(text);
    };
    focus = () => {
      xterm.focused++;
    };
  }
  return { Terminal };
});
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit = () => undefined;
  },
}));
vi.mock("@xterm/addon-search", () => ({
  SearchAddon: class {
    onDidChangeResults = () => ({ dispose: () => undefined });
  },
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: class {
    dispose = () => undefined;
  },
}));
vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: class {
    dispose = () => undefined;
  },
}));
vi.mock("../service", () => ({ watchView: () => () => undefined }));
vi.mock("../tauri", () => ({
  request: () => Promise.resolve(null),
  attachTerminal: (_session: string, onFeed: (feed: TerminalFeed) => void) => {
    feeds.push(onFeed);
    return Promise.resolve(1);
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let mounted = false;
let container: HTMLElement;

beforeEach(() => {
  if (!("fonts" in document)) {
    Object.defineProperty(document, "fonts", {
      value: { load: () => Promise.resolve([]) },
      configurable: true,
    });
  }
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  mounted = true;
  xterm.pasted = [];
  xterm.focused = 0;
  feeds.length = 0;
});

function unmount() {
  act(() => {
    root.unmount();
  });
  mounted = false;
}

afterEach(() => {
  if (mounted) unmount();
  container.remove();
});

function mount(readOnly: boolean) {
  act(() => {
    root.render(
      createElement(
        RegistriesContext,
        { value: createRegistries() },
        createElement(Terminal, { session: "s1", label: "SHOP-19 · agent terminal", readOnly }),
      ),
    );
  });
  const well = container.querySelector(".terminal-well");
  const inside = container.querySelector(".terminal-host");
  if (!well || !inside) throw new Error("no terminal well");
  return { well, inside };
}

function phase(next: "live" | "ended") {
  act(() => {
    for (const feed of feeds) feed({ kind: "phase", phase: next });
  });
}

describe("a dropped file on the terminal component", () => {
  it("is pasted into a live terminal, which takes the keyboard; not before it is live, nor after it ended", () => {
    const { inside } = mount(false);
    const target = dropTargets.at(inside);
    expect(target?.takesTyping()).toBe(false);
    phase("live");
    expect(target?.takesTyping()).toBe(true);
    target?.paste("/tmp/a\\ b.png ");
    expect(xterm.pasted).toEqual(["/tmp/a\\ b.png "]);
    expect(xterm.focused).toBe(1);
    phase("ended");
    expect(target?.takesTyping()).toBe(false);
  });

  it("a read-only terminal never takes it", () => {
    const { inside } = mount(true);
    phase("live");
    expect(dropTargets.at(inside)?.takesTyping()).toBe(false);
  });

  it("is no longer found once the terminal is unmounted", () => {
    const { well } = mount(false);
    expect(dropTargets.at(well)).toBeDefined();
    unmount();
    expect(dropTargets.at(well)).toBeUndefined();
  });
});
