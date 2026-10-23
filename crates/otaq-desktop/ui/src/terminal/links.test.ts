// @vitest-environment happy-dom
// Terminal links (d0117 points 7 and 9, T-0484): every link opens on ⌘-click (Ctrl-click on Windows and Linux) only,
// plain URLs and OSC 8 links alike; the underline and the hint show only while the key is held. The real xterm.js
// finds the links (its own OSC 8 provider, the web links add-on); xterm.js's mouse handling needs a drawn terminal, so
// the tests call the links' hover, leave and activate as it would.
import { WebLinksAddon } from "@xterm/addon-web-links";
import { type ILink, type ILinkProvider, Terminal } from "@xterm/xterm";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  createLinkGate,
  gateHiddenLinks,
  type HiddenLinkOptions,
  hiddenLinkHandler,
  hiddenLinkOpens,
  hiddenLinkTarget,
  isOpenGesture,
  type LinkGate,
  type LinkHint,
  openHint,
  withGatedLinks,
} from "./links";

const PLAIN = { metaKey: false, ctrlKey: false };
const CMD = { metaKey: true, ctrlKey: false };
const CTRL = { metaKey: false, ctrlKey: true };

function mouse(keys: typeof PLAIN, x = 40, y = 20): MouseEvent {
  return new MouseEvent("mouseup", { ...keys, clientX: x, clientY: y });
}

function press(key: "Meta" | "Control", type: "keydown" | "keyup") {
  window.dispatchEvent(
    new KeyboardEvent(type, {
      key,
      metaKey: key === "Meta" && type === "keydown",
      ctrlKey: key === "Control" && type === "keydown",
    }),
  );
}

describe("the open gesture", () => {
  it("is ⌘ on macOS and Ctrl elsewhere, with the hint to match", () => {
    expect(isOpenGesture(CMD, true)).toBe(true);
    expect(isOpenGesture(CTRL, true)).toBe(false);
    expect(isOpenGesture(PLAIN, true)).toBe(false);
    expect(isOpenGesture(CTRL, false)).toBe(true);
    expect(isOpenGesture(CMD, false)).toBe(false);
    expect(isOpenGesture(PLAIN, false)).toBe(false);
    expect(isOpenGesture({ metaKey: true, ctrlKey: true }, true)).toBe(false);
    expect(openHint(true)).toBe("⌘-click to open");
    expect(openHint(false)).toBe("Ctrl-click to open");
  });
});

describe("an OSC 8 link's target", () => {
  it("is a web page for http and https, a path for file links, nothing for other schemes", () => {
    expect(hiddenLinkTarget("https://example.com/a?b=1")).toEqual({
      kind: "web",
      url: "https://example.com/a?b=1",
    });
    expect(hiddenLinkTarget("http://localhost:3000")).toEqual({ kind: "web", url: "http://localhost:3000" });
    expect(hiddenLinkTarget("file://my-mac.local/Users/me/shop/a%20b.rs")).toEqual({
      kind: "file",
      path: "/Users/me/shop/a b.rs",
    });
    for (const uri of [
      "javascript:alert(1)",
      "ssh://host",
      "mailto:a@b.c",
      "vscode://file/x",
      "data:text/html,x",
      "nope",
    ]) {
      expect(hiddenLinkTarget(uri)).toBeUndefined();
    }
  });

  it("opens only web links until file links can open (T-0485)", () => {
    const options: HiddenLinkOptions = { mac: () => true, openWeb: () => undefined };
    expect(hiddenLinkOpens("https://example.com", options)).toBe(true);
    expect(hiddenLinkOpens("file:///Users/me/shop/a.rs", options)).toBe(false);
    expect(hiddenLinkOpens("file:///Users/me/shop/a.rs", { ...options, openFile: () => undefined })).toBe(
      true,
    );
    expect(hiddenLinkOpens("javascript:alert(1)", options)).toBe(false);
  });
});

/** The links a provider gives for buffer line `y` (1-based). */
function linksOf(provider: ILinkProvider | undefined, y = 1): ILink[] {
  let found: ILink[] = [];
  provider?.provideLinks(y, (links) => {
    found = links ?? [];
  });
  return found;
}

function providersOf(term: Terminal): ILinkProvider[] {
  return (term as unknown as { _core: { _linkProviderService: { linkProviders: ILinkProvider[] } } })._core
    ._linkProviderService.linkProviders;
}

function written(term: Terminal, text: string): Promise<void> {
  return new Promise((resolve) => {
    term.write(text, resolve);
  });
}

describe.each([
  { platform: "macOS", mac: true, gesture: CMD, key: "Meta" as const },
  { platform: "Windows and Linux", mac: false, gesture: CTRL, key: "Control" as const },
])("links on $platform", ({ mac, gesture, key }) => {
  let term: Terminal;
  let element: HTMLElement;
  let gate: LinkGate;
  let hints: (LinkHint | null)[];
  let openWeb: ReturnType<typeof vi.fn<(url: string) => void>>;
  let opened: string[];

  beforeEach(() => {
    term = new Terminal({ allowProposedApi: true, cols: 80, rows: 5 });
    element = document.createElement("div");
    hints = [];
    opened = [];
    openWeb = vi.fn((url: string) => {
      opened.push(url);
    });
    const hidden: HiddenLinkOptions = { mac: () => mac, openWeb };
    term.options.linkHandler = hiddenLinkHandler(hidden);
    gate = createLinkGate({ mac: () => mac, element, onHint: (h) => hints.push(h) });
    expect(gateHiddenLinks(term, gate, hidden)).toBe(true);
    const web = new WebLinksAddon((_event, uri) => {
      openWeb(uri);
    });
    term.loadAddon({
      activate: (t) => {
        web.activate(withGatedLinks(t, gate));
      },
      dispose: () => {
        web.dispose();
      },
    });
  });

  afterEach(() => {
    press(key, "keyup");
    gate.dispose();
    term.dispose();
  });

  it("open a plain URL on the gesture only, never on a plain click", async () => {
    await written(term, "see https://example.com/docs now");
    const [link] = linksOf(providersOf(term)[1]);
    expect(link?.text).toBe("https://example.com/docs");
    link?.activate(mouse(PLAIN), link.text);
    expect(opened).toEqual([]);
    link?.activate(mouse(gesture), link.text);
    expect(opened).toEqual(["https://example.com/docs"]);
  });

  it("open an OSC 8 https link with its address on the gesture only", async () => {
    await written(term, "\x1b]8;;https://example.com/pr/12\x07PR #12\x1b]8;;\x07 done");
    const [link] = linksOf(providersOf(term)[0]);
    expect(link?.text).toBe("https://example.com/pr/12");
    link?.activate(mouse(PLAIN), link.text);
    expect(openWeb).not.toHaveBeenCalled();
    link?.activate(mouse(gesture), link.text);
    expect(openWeb).toHaveBeenCalledExactlyOnceWith("https://example.com/pr/12");
  });

  it("make no link of OSC 8 links to other schemes, and file links aren't opened yet", async () => {
    const schemes = ["javascript:alert(1)", "ssh://host", "file:///Users/me/shop/a.rs", "mailto:a@b.c"];
    await written(term, schemes.map((uri) => `\x1b]8;;${uri}\x07x\x1b]8;;\x07 `).join(""));
    expect(linksOf(providersOf(term)[0])).toEqual([]);
    // Even reaching the handler (xterm.js's own provider ungated), they don't open.
    const range = { start: { x: 1, y: 1 }, end: { x: 1, y: 1 } };
    for (const uri of schemes) term.options.linkHandler?.activate(mouse(gesture), uri, range);
    expect(openWeb).not.toHaveBeenCalled();
  });

  it("underline and hint only while the key is held", async () => {
    await written(term, "\x1b]8;;https://example.com\x07site\x1b]8;;\x07 and https://example.org");
    for (const link of [linksOf(providersOf(term)[0])[0], linksOf(providersOf(term)[1])[0]]) {
      if (!link) throw new Error("no link found");
      hints.length = 0;
      // A plain hover: no underline, no pointer, no hint.
      expect(link.decorations).toEqual({ underline: false, pointerCursor: false });
      link.hover?.(mouse(PLAIN, 30, 12), link.text);
      expect(link.decorations).toEqual({ underline: false, pointerCursor: false });
      expect(hints.at(-1)).toBeNull();
      // The key goes down over it: underlined, with the hint at the pointer.
      press(key, "keydown");
      expect(link.decorations).toEqual({ underline: true, pointerCursor: true });
      expect(hints.at(-1)).toEqual({ label: openHint(mac), x: 30, y: 12 });
      // And up again.
      press(key, "keyup");
      expect(link.decorations).toEqual({ underline: false, pointerCursor: false });
      expect(hints.at(-1)).toBeNull();
      // Hovered with the key already held; leaving hides the hint.
      link.hover?.(mouse(gesture, 50, 14), link.text);
      expect(hints.at(-1)).toEqual({ label: openHint(mac), x: 50, y: 14 });
      link.leave?.(mouse(gesture), link.text);
      expect(hints.at(-1)).toBeNull();
      press(key, "keyup");
    }
  });

  it("show an OSC 8 link's address on hover", async () => {
    await written(term, "\x1b]8;;https://example.com/pr/12\x07PR #12\x1b]8;;\x07");
    const [link] = linksOf(providersOf(term)[0]);
    link?.hover?.(mouse(PLAIN), link.text);
    expect(element.title).toBe("https://example.com/pr/12");
    link?.leave?.(mouse(PLAIN), link.text);
    expect(element.hasAttribute("title")).toBe(false);
  });
});
