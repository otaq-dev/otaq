// @vitest-environment happy-dom
// The round-trip gate of d0078 (T-0248): the editor's Markdown setup, headless, loads each sample and saves it without
// an edit. Most samples must come back byte for byte. The 35 that don't carry the accepted output as `saved`: the user
// kept Tiptap with these differences (Q48, d0078 refined), and each is described in tickets/m15-roundtrip.md. Any other
// change, after a package update or a new extension line, fails here.
//
// It runs with a DOM (happy-dom) because the app's WebView has one, and @tiptap/markdown parses HTML in Markdown
// differently without it (it keeps the tags as text instead of reading them).
//
// samples.json holds:
// - "example": the SHOP-47 description of design update P;
// - "agent": 40 bodies written by an agent: every sixth ticket of tickets/v0.1-tickets.md (T-0001, T-0007, …), assembled
//   as `otaq create -f` stores them (otaq-core board/batch.rs: "What and why", then the "## Done when" list, then the
//   notes);
// - "edge": the edge cases T-0248 lists, plus a few found while checking the agent bodies.

import { Editor } from "@tiptap/core";
import { describe, expect, test } from "vitest";

import { markdownExtensions } from "../extensions";
import samples from "./samples.json";

/** Loads `markdown` into a headless editor and saves it back, with no edit in between. */
function roundTrip(markdown: string): string {
  const editor = new Editor({
    element: null,
    extensions: markdownExtensions(),
    content: markdown,
    contentType: "markdown",
  });
  try {
    return editor.getMarkdown();
  } finally {
    editor.destroy();
  }
}

/**
 * The fence-in-fence samples (M3 in tickets/m15-roundtrip.md): today the code block always writes a ``` fence, so a
 * block containing a ``` line comes back broken, and `saved` records that broken text. T-0254 fixes the fence in
 * markdown/code/ and changes these two expectations to the input (Q48).
 */
const brokenFences = ["fence-containing-fence", "four-backtick-fence"];

describe("the description round-trips as accepted (d0078, Q48)", () => {
  test.each(samples.map((sample) => [`${sample.kind} · ${sample.name}`, sample] as const))(
    "%s",
    (_name, sample) => {
      expect(roundTrip(sample.markdown)).toBe(sample.saved ?? sample.markdown);
    },
  );

  test("the broken fences are still recorded as known (T-0254 fixes them)", () => {
    for (const name of brokenFences) {
      const sample = samples.find((s) => s.name === name);
      expect(sample?.saved).toBeDefined();
      expect(sample?.saved).not.toBe(sample?.markdown);
    }
  });
});
