// @vitest-environment happy-dom
// Copy and paste in the description editor (T-0255): pasted HTML, Markdown text and plain text on a mounted editor
// (ProseMirror's own paste path, from a real paste event), the note's count, and the Markdown a copy puts on the
// clipboard.

import { Editor } from "@tiptap/core";
import { TextSelection } from "@tiptap/pm/state";
import { afterEach, describe, expect, test, vi } from "vitest";

import { loadMarkdown, serializeMarkdown } from "../changes";
import { markdownExtensions } from "../extensions";
import { droppedNote } from "./dropped";
import { droppedByPaste, pastePlainText } from "./paste";

let editor: Editor | null = null;

afterEach(() => {
  editor?.destroy();
  editor = null;
  vi.restoreAllMocks();
});

/** A mounted editor holding `markdown`, with the caret at its end. */
function mounted(markdown = ""): Editor {
  editor = new Editor({ element: document.createElement("div"), extensions: markdownExtensions() });
  loadMarkdown(editor, markdown);
  editor.view.dispatch(editor.state.tr.setSelection(TextSelection.atEnd(editor.state.doc)));
  return editor;
}

/** Pastes what a clipboard holds, as the webview does: a `paste` event on the text. */
function paste(e: Editor, data: Record<string, string>, files: File[] = []): void {
  const clipboard = new DataTransfer();
  for (const [type, value] of Object.entries(data)) clipboard.setData(type, value);
  for (const file of files) clipboard.items.add(file);
  e.view.dom.dispatchEvent(
    new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true, cancelable: true }),
  );
}

function note(e: Editor): string | null {
  const dropped = droppedByPaste(e.state);
  return dropped === null ? null : droppedNote(dropped);
}

/** The "Address autocomplete — notes" part of a Google Doc (*6 · Copy and paste*), as Google Docs puts it on the
 * clipboard: a `b` wrapper, styles on spans, a coloured and highlighted bold, an image, a bullet in another font and
 * colour, a link, and a table drawn without a header row. */
const GOOGLE_DOC = `<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-5f1c2b7e-7fff-1a2b-3c4d-5e6f7a8b9c0d"><h1 dir="ltr" style="line-height:1.38;margin-top:20pt;margin-bottom:6pt;"><span style="font-size:20pt;font-family:'Playfair Display',serif;color:#b4235a;font-weight:400;">Address autocomplete — notes</span></h1><p dir="ltr" style="line-height:1.38;text-align:center;margin-top:0pt;margin-bottom:0pt;"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#202124;">Failed deliveries are up </span><span style="font-size:11pt;font-family:Arial,sans-serif;color:#d93025;background-color:#fce8e6;font-weight:700;">12%</span><span style="font-size:11pt;font-family:Arial,sans-serif;color:#202124;"> this quarter.</span></p><p dir="ltr"><span style="font-size:11pt;"><span style="border:none;display:inline-block;overflow:hidden;width:240px;height:120px;"><img src="https://lh7-rt.googleusercontent.com/docsz/chart.png" width="240" height="120" alt="chart.png"></span></span></p><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;"><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#202124;" aria-level="1"><p dir="ltr" role="presentation"><span style="font-size:11pt;">Places API fills the street</span></p></li><li dir="ltr" style="list-style-type:disc;" aria-level="1"><p dir="ltr" role="presentation"><span style="font-size:12pt;font-family:'Comic Neue',cursive;color:#188038;text-decoration:underline;">Postcode stays editable</span></p></li></ul><p dir="ltr"><span style="font-size:11pt;color:#202124;">Source: </span><a href="https://example.com/report" style="text-decoration:none;"><span style="font-size:11pt;color:#1a73e8;text-decoration:underline;">Support report Q3</span></a></p><div dir="ltr" style="margin-left:0pt;" align="left"><table style="border:none;border-collapse:collapse;"><colgroup><col width="120"><col width="160"></colgroup><tbody><tr style="height:0pt"><td style="background-color:#4c57d6;padding:5pt;"><p dir="ltr"><span style="font-size:10pt;color:#ffffff;font-weight:700;">Field</span></p></td><td style="background-color:#4c57d6;padding:5pt;"><p dir="ltr" style="text-align:center"><span style="font-size:10pt;color:#ffffff;font-weight:700;">Source</span></p></td></tr><tr style="height:0pt"><td style="padding:5pt;"><p dir="ltr"><span style="font-size:10pt;">Street</span></p></td><td style="padding:5pt;" align="right"><p dir="ltr"><span style="font-size:10pt;">Places API</span></p></td></tr></tbody></table></div></b>`;

describe("pasting HTML", () => {
  test("a Google Doc keeps headings, lists, bold, links and the table, and leaves out the rest", () => {
    const e = mounted();
    paste(e, { "text/html": GOOGLE_DOC, "text/plain": "Address autocomplete — notes …" });
    expect(serializeMarkdown(e)).toBe(`# Address autocomplete — notes

Failed deliveries are up **12%** this quarter.

- Places API fills the street
- Postcode stays editable

Source: [Support report Q3](https://example.com/report)


| Field  | Source     |
| ------ | ---------- |
| Street | Places API |
`);
    expect(note(e)).toBe("1 image left out: descriptions are Markdown only");
  });

  test("merged cells are split, and a table without a header row gets its first row as the header", () => {
    const e = mounted();
    const html =
      "<table><tr><td colspan='2'>Address</td><td>Note</td></tr>" +
      "<tr><td rowspan='2'>Street</td><td>Places</td><td>auto</td></tr>" +
      "<tr><td>Typed</td><td>fallback</td></tr></table>";
    paste(e, { "text/html": html });
    expect(serializeMarkdown(e)).toBe(`
| Address |        | Note     |
| ------- | ------ | -------- |
| Street  | Places | auto     |
|         | Typed  | fallback |
`);
  });

  test("underline, highlights, colours and alignment are dropped; strike, italic and code stay", () => {
    const e = mounted();
    paste(e, {
      "text/html":
        '<p style="text-align:right"><u>under</u> <mark>marked</mark> <span style="color:red;font-family:Courier">red</span> ' +
        "<s>gone</s> <em>soft</em> <code>npm ci</code></p>",
    });
    expect(serializeMarkdown(e)).toBe("under marked red ~~gone~~ *soft* `npm ci`");
    expect(note(e)).toBeNull();
  });

  test("a code block keeps its language; a GitHub task list stays a task list", () => {
    const e = mounted();
    paste(e, {
      "text/html":
        '<pre><code class="language-ts">const a = 1;\nconst b = 2;</code></pre>' +
        '<ul class="contains-task-list"><li class="task-list-item"><input type="checkbox" checked disabled> Done</li>' +
        '<li class="task-list-item"><input type="checkbox" disabled> Not yet</li></ul>',
    });
    expect(serializeMarkdown(e)).toBe("```ts\nconst a = 1;\nconst b = 2;\n```\n\n- [x] Done\n- [ ] Not yet");
  });

  test("videos and frames are counted as embeds, next to images", () => {
    const e = mounted();
    paste(e, {
      "text/html":
        '<p>Demo</p><iframe src="https://www.youtube.com/embed/x"></iframe><video src="a.mp4"></video>' +
        '<p><img src="a.png"><img src="b.png"></p>',
    });
    expect(serializeMarkdown(e)).toBe("Demo");
    expect(note(e)).toBe("2 images and 2 embeds left out: descriptions are Markdown only");
  });
});

describe("pasting plain text", () => {
  test("text that looks like Markdown becomes formatting", () => {
    const e = mounted();
    const reply = `## Why
Users mistype addresses; **12%** of failed deliveries trace back to it (see [the report](https://example.com/report)).

- [x] Suggestions appear after 3 characters
- [ ] Arrow keys move the highlight

| Field | Source |
|---|---|
| Street | Places API |

\`\`\`ts
const results = await places.autocomplete(query);
\`\`\`
`;
    paste(e, { "text/plain": reply });
    const doc = e.getJSON();
    expect(doc.content.map((node) => node.type)).toEqual([
      "heading",
      "paragraph",
      "taskList",
      "table",
      "codeBlock",
      "paragraph",
    ]);
    expect(serializeMarkdown(e)).toContain("**12%** of failed deliveries");
    expect(serializeMarkdown(e)).toContain("[the report](https://example.com/report)");
  });

  test("one line of Markdown joins the paragraph at the caret", () => {
    const e = mounted("Before: ");
    paste(e, { "text/plain": "a **bold** word" });
    expect(serializeMarkdown(e)).toBe("Before: a **bold** word");
  });

  test("a URL pasted over selected words links them (Tiptap's link-on-paste, kept with the paste rules off)", () => {
    const e = mounted("See the report here");
    const from = e.state.doc.textContent.indexOf("the report") + 1;
    e.view.dispatch(
      e.state.tr.setSelection(TextSelection.create(e.state.doc, from, from + "the report".length)),
    );
    paste(e, { "text/plain": "https://example.com/report" });
    expect(serializeMarkdown(e)).toBe("See [the report](https://example.com/report) here");
  });

  test("text that doesn't look like Markdown is pasted as it is, a paragraph per line", () => {
    const e = mounted();
    paste(e, { "text/plain": "2 * 3 * 4 = 24\nsnake_case_name stays" });
    expect(e.getJSON().content.map((node) => node.type)).toEqual(["paragraph", "paragraph"]);
    expect(e.getText()).toContain("2 * 3 * 4 = 24");
  });

  test("Markdown pasted into a code block is code", () => {
    const e = mounted("```\nx\n```");
    e.view.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 2)));
    paste(e, { "text/plain": "## not a heading" });
    expect(e.getJSON().content[0]?.type).toBe("codeBlock");
    expect(e.state.doc.firstChild?.textContent).toBe("x## not a heading");
  });

  test("images in Markdown are left out and counted, but not in code", () => {
    const e = mounted();
    paste(e, {
      "text/plain": "## Shots\n\n![chart](chart.png)\n\nSee `![not](an image)` here <img src=x>\n",
    });
    expect(serializeMarkdown(e)).toBe("## Shots\n\nSee `![not](an image)` here");
    expect(note(e)).toBe("2 images left out: descriptions are Markdown only");
  });
});

describe("⌘⇧V / Ctrl+Shift+V", () => {
  test("pastes plain text: no formatting, no Markdown reading", () => {
    const e = mounted();
    pastePlainText(e, "## Why\n**12%** of [them](https://x.example)");
    expect(e.getJSON().content.map((node) => node.type)).toEqual(["paragraph", "paragraph"]);
    expect(e.getText()).toContain("## Why");
    expect(e.getText()).toContain("**12%** of [them](https://x.example)");
    expect(JSON.stringify(e.getJSON())).not.toContain('"marks"');
  });

  test("the key reads the clipboard's text and pastes it plain", async () => {
    const readText = vi.spyOn(navigator.clipboard, "readText").mockResolvedValue("**not *bold***");
    const e = mounted();
    e.view.focus();
    const key = new KeyboardEvent("keydown", {
      key: "V",
      code: "KeyV",
      keyCode: 86,
      shiftKey: true,
      ctrlKey: !/Mac/.test(navigator.platform),
      metaKey: /Mac/.test(navigator.platform),
      bubbles: true,
      cancelable: true,
    });
    e.view.dom.dispatchEvent(key);
    expect(key.defaultPrevented).toBe(true);
    await vi.waitFor(() => {
      expect(e.getText()).toBe("**not *bold***");
    });
    expect(readText).toHaveBeenCalledOnce();
  });
});

describe("the note", () => {
  test("an image alone on the clipboard pastes nothing, changes nothing, and is counted", () => {
    const e = mounted("Untouched");
    const before = e.state.doc;
    paste(e, {}, [new File(["png"], "shot.png", { type: "image/png" })]);
    expect(e.state.doc).toBe(before);
    expect(note(e)).toBe("1 image left out: descriptions are Markdown only");
  });

  test("stays while the caret moves and goes at the next edit", () => {
    const e = mounted("Text");
    paste(e, { "text/html": '<p>More</p><img src="a.png">' });
    expect(note(e)).not.toBeNull();
    e.view.dispatch(e.state.tr.setSelection(TextSelection.create(e.state.doc, 1)));
    expect(note(e)).not.toBeNull();
    e.view.dispatch(e.state.tr.insertText("x"));
    expect(note(e)).toBeNull();
  });

  test("a paste that leaves nothing out shows no note, and replaces an earlier one", () => {
    const e = mounted();
    paste(e, { "text/html": '<img src="a.png">' });
    expect(note(e)).not.toBeNull();
    paste(e, { "text/html": "<p>plain</p>" });
    expect(note(e)).toBeNull();
  });

  test("loading a description shows none and changes nothing", () => {
    const e = mounted("![logo](logo.png) and text");
    expect(note(e)).toBeNull();
  });
});

describe("copying", () => {
  /** What a copy of `from`–`to` puts on the clipboard as `text/plain` and `text/html`. */
  function copied(e: Editor, from: number, to: number): { text: string; html: string } {
    const { dom, text } = e.view.serializeForClipboard(e.state.doc.slice(from, to, true));
    return { text, html: dom.innerHTML };
  }

  /** The document position of the start of `text`. */
  function at(e: Editor, text: string): number {
    let pos = -1;
    e.state.doc.descendants((node, offset) => {
      const index = node.isText ? (node.text ?? "").indexOf(text) : -1;
      if (pos < 0 && index >= 0) pos = offset + index;
    });
    return pos;
  }

  const EXAMPLE = `## Why
Users mistype addresses; **12%** of failed deliveries trace back to it.

- [x] Suggestions appear after 3 characters
- [ ] Arrow keys move the highlight

\`\`\`ts
const results = await places.autocomplete(query);
\`\`\``;

  test("the whole description copies as the Markdown a save writes, and as HTML", () => {
    const e = mounted(EXAMPLE);
    const { text, html } = copied(e, 0, e.state.doc.content.size);
    expect(text).toBe(serializeMarkdown(e));
    expect(html).toContain("<strong>12%</strong>");
    expect(html).toContain("<h2");
  });

  test("words inside a paragraph keep their formatting, without the block around them", () => {
    const e = mounted(EXAMPLE);
    const from = at(e, "12%");
    expect(copied(e, from, at(e, " trace")).text).toBe("**12%** of failed deliveries");
  });

  test("words inside a task item come without the box; two items come as a list", () => {
    const e = mounted(EXAMPLE);
    expect(copied(e, at(e, "Suggestions"), at(e, " after")).text).toBe("Suggestions appear");
    expect(copied(e, at(e, "Suggestions"), at(e, " the highlight")).text).toBe(
      "- [x] Suggestions appear after 3 characters\n- [ ] Arrow keys move",
    );
  });

  test("code inside a code block copies as the code itself", () => {
    const e = mounted(EXAMPLE);
    expect(copied(e, at(e, "await"), at(e, "(query)")).text).toBe("await places.autocomplete");
  });
});
