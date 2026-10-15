// @vitest-environment happy-dom
import { Editor } from "@tiptap/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { loadMarkdown, serializeMarkdown } from "../changes";
import { markdownExtensions } from "../extensions";
import { openLink } from "../toolbar/link";
import { applyLink, closeLinkPopover, linkTargetAt, linkTargetOf, linkUi, removeLink } from "./links";
import { checkLinkUrl, LINK_ERRORS, opensInBrowser, shortUrl } from "./url";

const requests = vi.hoisted(() => [] as [string, unknown][]);
vi.mock(import("../../tauri"), async (importOriginal) => ({
  ...(await importOriginal()),
  request: (method: string, params: unknown) => {
    requests.push([method, params]);
    return Promise.resolve(null);
  },
}));

const editors: Editor[] = [];

function editorWith(markdown: string, { mounted = false, editable = true } = {}): Editor {
  const element = mounted ? document.body.appendChild(document.createElement("div")) : null;
  const editor = new Editor({ element, editable, extensions: markdownExtensions() });
  loadMarkdown(editor, markdown);
  editors.push(editor);
  return editor;
}

beforeEach(() => {
  requests.length = 0;
});

afterEach(() => {
  for (const e of editors.splice(0)) e.destroy();
  document.body.replaceChildren();
});

describe("the URL check", () => {
  it.each([
    ["https://example.com/report", "https://example.com/report"],
    ["  http://localhost:4000/a?b=c  ", "http://localhost:4000/a?b=c"],
    ["mailto:team@example.com", "mailto:team@example.com"],
    ["HTTPS://Example.com", "HTTPS://Example.com"],
    ["example.com/report", "https://example.com/report"],
  ])("passes %j as %j", (typed, href) => {
    expect(checkLinkUrl(typed)).toEqual({ ok: true, href });
  });

  it.each([
    ["javascript:alert(1)", LINK_ERRORS.scheme],
    ["file:///etc/passwd", LINK_ERRORS.scheme],
    ["ftp://example.com", LINK_ERRORS.scheme],
    ["data:text/html,hi", LINK_ERRORS.scheme],
    ["otaq://ticket/SHOP-47", LINK_ERRORS.scheme],
    ["", LINK_ERRORS.empty],
    ["   ", LINK_ERRORS.empty],
    ["https://", LINK_ERRORS.invalid],
    ["https://exa mple.com", LINK_ERRORS.invalid],
  ])("refuses %j with the inline error", (typed, error) => {
    expect(checkLinkUrl(typed)).toEqual({ ok: false, error });
  });

  it("opens web links only, and shortens the bubble's URL", () => {
    expect(opensInBrowser("https://example.com")).toBe(true);
    expect(opensInBrowser("http://example.com")).toBe(true);
    expect(opensInBrowser("mailto:a@b.c")).toBe(false);
    expect(opensInBrowser("javascript:alert(1)")).toBe(false);
    expect(shortUrl("https://example.com/report")).toBe("example.com/report");
    expect(shortUrl("https://example.com/")).toBe("example.com");
    expect(shortUrl("mailto:a@b.c")).toBe("a@b.c");
  });
});

describe("what ⌘K edits", () => {
  it("is the link around the caret", () => {
    const editor = editorWith("see [the report](https://example.com/report).");
    editor.commands.setTextSelection(8);
    expect(linkTargetAt(editor.state)).toEqual({
      from: 5,
      to: 15,
      href: "https://example.com/report",
      linked: true,
    });
  });

  it("is the selection, with a selected URL as the address", () => {
    // A URL typed without a space after it isn't linked yet.
    const editor = editorWith("go to ");
    editor.commands.insertContentAt(7, "https://example.com");
    editor.commands.setTextSelection({ from: 7, to: 26 });
    expect(linkTargetAt(editor.state)).toEqual({
      from: 7,
      to: 26,
      href: "https://example.com",
      linked: false,
    });
    editor.commands.setTextSelection({ from: 1, to: 3 });
    expect(linkTargetAt(editor.state)).toEqual({ from: 1, to: 3, href: "", linked: false });
  });

  it("is the caret alone outside a link", () => {
    const editor = editorWith("hello");
    editor.commands.setTextSelection(3);
    expect(linkTargetAt(editor.state)).toEqual({ from: 3, to: 3, href: "", linked: false });
  });
});

describe("the popover", () => {
  it("opens on ⌘K and the Link button without touching the text", () => {
    const editor = editorWith("see [here](https://a.example)");
    const before = editor.state.doc;
    const updates = vi.fn();
    editor.on("update", updates);
    editor.commands.setTextSelection(6);
    openLink(editor);
    expect(linkUi(editor).get().popover).toEqual({ from: 5, to: 9, href: "https://a.example", linked: true });
    closeLinkPopover(editor);
    expect(linkUi(editor).get().popover).toBeNull();
    expect(editor.state.doc).toBe(before);
    expect(updates).not.toHaveBeenCalled();
  });

  it("doesn't open in read mode", () => {
    const editor = editorWith("see [here](https://a.example)", { editable: false });
    openLink(editor);
    expect(linkUi(editor).get().popover).toBeNull();
  });

  it("Apply links the selection, changes a link and types a URL at the caret", () => {
    const editor = editorWith("see here");
    editor.commands.setTextSelection({ from: 5, to: 9 });
    applyLink(editor, linkTargetAt(editor.state), "https://a.example");
    expect(serializeMarkdown(editor)).toBe("see [here](https://a.example)");

    editor.commands.setTextSelection(6);
    applyLink(editor, linkTargetAt(editor.state), "https://b.example");
    expect(serializeMarkdown(editor)).toBe("see [here](https://b.example)");

    editor.commands.setTextSelection(1);
    applyLink(editor, linkTargetAt(editor.state), "mailto:a@b.c");
    expect(serializeMarkdown(editor)).toBe("[mailto:a@b.c](mailto:a@b.c)see [here](https://b.example)");
    editor.commands.insertContent("…");
    expect(serializeMarkdown(editor)).toBe("[mailto:a@b.c](mailto:a@b.c)…see [here](https://b.example)");
  });

  it("Remove takes the link off and keeps the text", () => {
    const editor = editorWith("see [here](https://a.example) now");
    editor.commands.setTextSelection(7);
    removeLink(editor, linkTargetAt(editor.state));
    expect(serializeMarkdown(editor)).toBe("see here now");
  });

  it("follows edits made while it is open", () => {
    const editor = editorWith("see [here](https://a.example)");
    editor.commands.setTextSelection(6);
    openLink(editor);
    editor.commands.insertContentAt(1, "and ");
    expect(linkUi(editor).get().popover).toMatchObject({ from: 9, to: 13 });
  });
});

describe("clicks and hovers on links", () => {
  const anchor = (editor: Editor) => {
    const a = editor.view.dom.querySelector("a");
    if (!a) throw new Error("no link");
    return a;
  };

  it("read mode: a click opens the link in the system browser, never in the page", () => {
    const editor = editorWith("see [the report](https://example.com/report)", {
      mounted: true,
      editable: false,
    });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    anchor(editor).dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(requests).toEqual([["terminal.open_link", { url: "https://example.com/report" }]]);
  });

  it("read mode: a mailto link isn't followed and doesn't open (Q52)", () => {
    const editor = editorWith("[mail](mailto:a@b.c)", { mounted: true, editable: false });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    anchor(editor).dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(requests).toEqual([]);
  });

  it("edit mode: a click places the caret and opens nothing", () => {
    const editor = editorWith("see [the report](https://example.com/report)", { mounted: true });
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    anchor(editor).dispatchEvent(click);
    expect(click.defaultPrevented).toBe(true);
    expect(requests).toEqual([]);
  });

  it("hovering a link reports it, and its target is the whole link", () => {
    const editor = editorWith("see [the report](https://example.com/report)", { mounted: true });
    const a = anchor(editor);
    a.dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
    expect(linkUi(editor).get().hover).toBe(a);
    expect(linkTargetOf(editor, a)).toEqual({
      from: 5,
      to: 15,
      href: "https://example.com/report",
      linked: true,
    });
    a.dispatchEvent(new MouseEvent("mouseout", { bubbles: true, relatedTarget: editor.view.dom }));
    expect(linkUi(editor).get().hover).toBeNull();
  });
});
