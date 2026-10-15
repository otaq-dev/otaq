// @vitest-environment happy-dom
// What the editor reports (T-0249): an untouched description is never re-saved. The tracker is tested on its own, then
// with a headless editor over every round-trip sample: loading reads them as the round-trip gate does, a click (which makes the editor add its trailing paragraph)
// changes nothing, and typing then undoing reports the original string.

import { Editor, type EditorEvents } from "@tiptap/core";
import { describe, expect, test } from "vitest";

import {
  ChangeTracker,
  loadMarkdown,
  serializeMarkdown,
  withoutTrailingEmptyParagraphs,
  wrapLooseInline,
} from "./changes";
import { markdownExtensions } from "./extensions";
import samples from "./roundtrip/samples.json";

describe("ChangeTracker", () => {
  test("reports a changed text once, and the original string when it comes back", () => {
    const tracker = new ChangeTracker("* a", "- a");
    expect(tracker.value).toBe("* a");
    expect(tracker.report("- a")).toBeNull();
    expect(tracker.report("- ab")).toBe("- ab");
    expect(tracker.report("- ab")).toBeNull();
    expect(tracker.report("- a")).toBe("* a");
    expect(tracker.value).toBe("* a");
  });

  test("a reload starts over from the new text", () => {
    const tracker = new ChangeTracker("a", "a");
    tracker.report("ab");
    tracker.reload("* b", "- b");
    expect(tracker.value).toBe("* b");
    expect(tracker.report("- b")).toBeNull();
  });
});

test("only empty paragraphs at the end are left out", () => {
  const text = { type: "text", text: "x" };
  const doc = {
    type: "doc",
    content: [
      { type: "paragraph" },
      { type: "paragraph", content: [text] },
      { type: "paragraph" },
      { type: "paragraph", content: [] },
    ],
  };
  expect(withoutTrailingEmptyParagraphs(doc).content).toEqual([
    { type: "paragraph" },
    { type: "paragraph", content: [text] },
  ]);
  expect(withoutTrailingEmptyParagraphs({ type: "doc" }).content).toEqual([]);
});

test("loose inline content is wrapped in a paragraph", () => {
  const editor = new Editor({ element: null, extensions: markdownExtensions() });
  try {
    const text = { type: "text", text: "logo" };
    const list = { type: "bulletList", content: [{ type: "listItem", content: [text] }] };
    expect(wrapLooseInline({ type: "doc", content: [text, list] }, editor.schema)).toEqual({
      type: "doc",
      content: [
        { type: "paragraph", content: [text] },
        {
          type: "bulletList",
          content: [{ type: "listItem", content: [{ type: "paragraph", content: [text] }] }],
        },
      ],
    });
  } finally {
    editor.destroy();
  }
});

/** A headless editor on `markdown`, with a tracker and the list of what it reported, wired as the hook wires them. */
function track(markdown: string) {
  const element = document.createElement("div");
  document.body.append(element);
  const editor = new Editor({ element, extensions: markdownExtensions() });
  loadMarkdown(editor, markdown);
  const tracker = new ChangeTracker(markdown, serializeMarkdown(editor));
  const reported: string[] = [];
  editor.on("update", ({ editor: changed, transaction }: EditorEvents["update"]) => {
    if (!transaction.docChanged) return;
    const next = tracker.report(serializeMarkdown(changed));
    if (next !== null) reported.push(next);
  });
  return { editor, tracker, reported };
}

/**
 * Samples whose accepted output ends in an empty paragraph (the dropped HTML block of M1, tickets/m15-roundtrip.md):
 * `serializeMarkdown` leaves it out, so they save without the blank lines at the end.
 */
const endsEmpty = ["html-block"];

test.each(samples.map((sample) => [`${sample.kind} · ${sample.name}`, sample] as const))(
  "loading saves as the round-trip gate accepted: %s",
  (_name, sample) => {
    const { editor } = track(sample.markdown);
    try {
      const accepted = sample.saved ?? sample.markdown;
      expect(serializeMarkdown(editor)).toBe(
        endsEmpty.includes(sample.name) ? accepted.replace(/\n+$/, "") : accepted,
      );
    } finally {
      editor.destroy();
    }
  },
);

describe("an untouched description is never re-saved", () => {
  test.each(samples.map((sample) => [`${sample.kind} · ${sample.name}`, sample.markdown] as const))(
    "%s",
    (_name, markdown) => {
      const { editor, tracker, reported } = track(markdown);
      try {
        // Clicks: the first transaction lets the editor append its trailing paragraph.
        editor.commands.focus("end");
        editor.commands.setTextSelection(1);
        editor.commands.selectAll();
        expect(reported).toEqual([]);
        expect(tracker.value).toBe(markdown);
      } finally {
        editor.destroy();
      }
    },
  );

  test("typing then undoing reports the original string, byte for byte", () => {
    const original = samples.find((s) => s.kind === "example")?.markdown ?? "";
    const { editor, tracker, reported } = track(original);
    try {
      editor.chain().focus("end").insertContent("x").run();
      expect(reported).toHaveLength(1);
      expect(reported[0]).not.toBe(original);
      editor.commands.undo();
      expect(reported).toHaveLength(2);
      expect(reported[1]).toBe(original);
      expect(tracker.value).toBe(original);
    } finally {
      editor.destroy();
    }
  });

  test("an edit reports Markdown, without the editor's trailing paragraph", () => {
    const { editor, reported } = track("| a |\n|---|\n| b |");
    try {
      editor.commands.focus("end");
      editor.commands.insertContentAt(0, "Intro");
      expect(reported).toHaveLength(1);
      expect(reported[0]).toMatch(/^Intro\n+\| a +\|\n/);
      expect(reported[0]).toMatch(/\| b +\|\n$/);
    } finally {
      editor.destroy();
    }
  });
});
