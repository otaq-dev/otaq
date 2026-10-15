// Pasted plain text that looks like Markdown (a reply copied from Claude Code, a GitHub issue body) is read as Markdown
// and becomes formatting (T-0255, d0078). Text that doesn't look like it is pasted as it is, line by line, so a log or
// an address with a stray `*` isn't turned into something else.

import type { Dropped } from "./dropped";

/** One sign is enough: a heading, a list or task item, a fence, a quote, a rule, bold, strike, a link, code, a table. */
const BLOCK_SIGNS = [
  /^ {0,3}#{1,6}[ \t]+\S/m, // ## Heading
  /^ {0,3}[-*+][ \t]+\S/m, // - item (and - [ ] task)
  /^ {0,3}\d{1,9}[.)][ \t]+\S/m, // 1. item
  /^ {0,3}(`{3,}|~{3,})/m, // ``` fence
  /^ {0,3}>[ \t]?\S/m, // > quote
  /^ {0,3}([-*_])([ \t]*\1){2,}[ \t]*$/m, // ---
];
const INLINE_SIGNS = [
  /\*\*[^*\n]+\*\*/, // **bold**
  /__[^_\n]+__/, // __bold__
  /~~[^~\n]+~~/, // ~~strike~~
  /\[[^\]\n]+\]\([^)\s]+(?:\s+"[^"\n]*")?\)/, // [text](url)
  /`[^`\n]+`/, // `code`
];
/** A table: a row of cells, then the line under the header (`|---|:---:|`). */
const TABLE_SIGN = /^.*\|.*\n {0,3}\|?[ \t]*:?-+:?[ \t]*(\|[ \t]*:?-+:?[ \t]*)+\|?[ \t]*$/m;

/** Whether pasted plain text reads as Markdown: it has at least one Markdown sign. */
export function looksLikeMarkdown(text: string): boolean {
  const unix = text.replace(/\r\n?/g, "\n");
  return [...BLOCK_SIGNS, ...INLINE_SIGNS, TABLE_SIGN].some((sign) => sign.test(unix));
}

const FENCE = /^ {0,3}(`{3,}|~{3,})/;
/** `![alt](src "title")` and `![alt][ref]`. */
const IMAGE = /!\[(?:\\.|[^\\\]\n])*\](?:\((?:\\.|[^\\()\n]|\([^()\n]*\))*\)|\[[^\]\n]*\])/g;
const HTML_IMAGE = /<img\b[^>]*>/gi;
const HTML_EMBED = /<(iframe|video|audio|embed|object)\b[^>]*>(?:[\s\S]*?<\/\1\s*>)?/gi;
/** Inline code spans, which are left alone: a backtick run up to the same run. */
const CODE_SPAN = /(`+)[\s\S]*?\1/g;

export interface StrippedMarkdown {
  markdown: string;
  dropped: Dropped;
}

/**
 * The Markdown without its images (`![…](…)`, `<img>`) and embeds (`<iframe>`, `<video>`…), and how many were taken
 * out. Code blocks and inline code are left as they are: an image written there is text.
 */
export function stripMarkdownImages(markdown: string): StrippedMarkdown {
  const dropped = { images: 0, embeds: 0 };
  let fence: string | null = null;
  const lines = markdown.split(/(\r\n?|\n)/);
  for (let i = 0; i < lines.length; i += 2) {
    const line = lines[i] ?? "";
    const opener = FENCE.exec(line)?.[1];
    if (fence !== null) {
      if (opener !== undefined && opener[0] === fence[0] && opener.length >= fence.length) fence = null;
      continue;
    }
    if (opener !== undefined) {
      fence = opener;
      continue;
    }
    const stripped = outsideCode(line, (text) =>
      text
        .replace(IMAGE, () => {
          dropped.images++;
          return "";
        })
        .replace(HTML_IMAGE, () => {
          dropped.images++;
          return "";
        })
        .replace(HTML_EMBED, () => {
          dropped.embeds++;
          return "";
        }),
    );
    // A line that held only an image goes with its line break, so no empty paragraph is left in its place.
    if (stripped !== line && stripped.trim() === "") {
      lines[i] = "";
      lines[i + 1] = "";
    } else {
      lines[i] = stripped === line ? line : stripped.replace(/[ \t]+$/, "");
    }
  }
  return { markdown: lines.join(""), dropped };
}

/** `line` with `change` applied to the parts outside inline code spans. */
function outsideCode(line: string, change: (text: string) => string): string {
  let out = "";
  let from = 0;
  for (const span of line.matchAll(CODE_SPAN)) {
    out += change(line.slice(from, span.index)) + span[0];
    from = span.index + span[0].length;
  }
  return out + change(line.slice(from));
}
