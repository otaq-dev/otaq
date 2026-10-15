// @vitest-environment happy-dom
// The paste helpers on their own (T-0255): what pasted HTML is cut down to, which plain text reads as Markdown, the
// images taken out of Markdown, and the note's wording.

import { describe, expect, test } from "vitest";

import { droppedNote } from "./dropped";
import { cleanPastedHtml } from "./html";
import { looksLikeMarkdown, stripMarkdownImages } from "./markdown";

describe("cleanPastedHtml", () => {
  test("takes images and embeds out, with the paragraphs they leave empty, and counts them", () => {
    const { html, dropped } = cleanPastedHtml(
      '<p>Text</p><p><span><img src="a.png"></span></p><picture><source srcset="b.webp"><img src="b.png"></picture>' +
        '<object data="c.swf"><embed src="c.swf"></object><svg><path d="M0 0"/></svg>',
    );
    expect(html).toBe("<p>Text</p>");
    expect(dropped).toEqual({ images: 2, embeds: 1 });
  });

  test("splits merged cells into a plain grid with one header row and no alignment", () => {
    const { html } = cleanPastedHtml(
      '<table><tr><td colspan="2" align="center">A</td></tr><tr><th>B</th><td style="text-align:right">C</td></tr></table>',
    );
    // (A browser adds a `tbody` around the rows; the test DOM doesn't.)
    expect(html.replace(/<\/?tbody>/g, "")).toBe(
      "<table><tr><th>A</th><th></th></tr><tr><td>B</td><td>C</td></tr></table>",
    );
  });

  test("leaves a table copied out of a description editor as it is", () => {
    const copied = '<table data-pm-slice="1 1 []"><tbody><tr><td><p>row 2</p></td></tr></tbody></table>';
    expect(cleanPastedHtml(copied).html).toBe(copied);
  });

  test("a checkbox at the start of a list item makes a task item", () => {
    const { html } = cleanPastedHtml(
      '<ul><li><input type="checkbox" checked> Done</li><li>Plain <input type="checkbox"></li></ul>',
    );
    expect(html).toBe(
      '<ul data-type="taskList"><li data-type="taskItem" data-checked="true"> Done</li><li>Plain <input type="checkbox"></li></ul>',
    );
  });
});

describe("looksLikeMarkdown", () => {
  test.each([
    ["## Why", true],
    ["- [ ] Arrow keys move the highlight", true],
    ["* one\n* two", true],
    ["1. first\n2. second", true],
    ["```ts\nconst a = 1;\n```", true],
    ["> quoted", true],
    ["---", true],
    ["Users mistype **12%** of addresses", true],
    ["see [the report](https://example.com/report)", true],
    ["run `npm ci` first", true],
    ["| Field | Source |\n|---|:---:|\n| Street | Places |", true],
    ["Just a sentence.", false],
    ["2 * 3 * 4 = 24", false],
    ["snake_case_name and #hashtag", false],
    ["Address: 12 Main St.\nPostcode 34000", false],
  ])("%j → %s", (text, expected) => {
    expect(looksLikeMarkdown(text)).toBe(expected);
  });
});

describe("stripMarkdownImages", () => {
  test("takes images and embeds out of text, not out of code", () => {
    const { markdown, dropped } = stripMarkdownImages(
      'Logo: ![logo](logo.png "Logo") and ![ref][1]\n![alone](a.png)\n<iframe src="x"></iframe>\n```\n![kept](k.png)\n```\nInline `![kept](k.png)`',
    );
    expect(markdown).toBe("Logo:  and\n```\n![kept](k.png)\n```\nInline `![kept](k.png)`");
    expect(dropped).toEqual({ images: 3, embeds: 1 });
  });
});

describe("droppedNote", () => {
  test.each([
    [{ images: 0, embeds: 0 }, null],
    [{ images: 1, embeds: 0 }, "1 image left out: descriptions are Markdown only"],
    [{ images: 3, embeds: 0 }, "3 images left out: descriptions are Markdown only"],
    [{ images: 0, embeds: 1 }, "1 embed left out: descriptions are Markdown only"],
    [{ images: 1, embeds: 2 }, "1 image and 2 embeds left out: descriptions are Markdown only"],
  ])("%j", (dropped, expected) => {
    expect(droppedNote(dropped)).toBe(expected);
  });
});
