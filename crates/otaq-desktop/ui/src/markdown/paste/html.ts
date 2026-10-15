// Pasted HTML (a web page, Google Docs, a Claude reply), cut down to what a Markdown description can hold before the
// editor reads it (T-0255, d0078 D6). Most of the cutting is the schema's: it has no colour, font, highlight,
// underline or alignment, so those are never read. What's left to do here is what the schema would read wrongly:
// - images and embeds are taken out and counted, for the note under the content;
// - merged table cells are split (Markdown has none), the first row becomes the header row (Markdown's tables have
//   one; Google Docs and many pages draw none), and cell alignment is dropped;
// - GitHub-style task lists (a checkbox at the start of a list item) become task items.

import type { Dropped } from "./dropped";

const IMAGES = "img";
const EMBEDS = "video, audio, iframe, embed, object";
/** Taken out without counting: drawings and icons that come along with a page's text, not pictures in it. */
const DECORATION = "svg, canvas, picture > source";

export interface CleanedHtml {
  html: string;
  dropped: Dropped;
}

/** The pasted HTML without images and embeds, with simple tables; and how many images and embeds were left out. */
export function cleanPastedHtml(html: string): CleanedHtml {
  // A template's content is inert: nothing in it loads (an image or a frame) while it's cleaned.
  const template = document.createElement("template");
  template.innerHTML = html;
  const doc = template.content;
  const dropped = {
    images: removeAll(doc, IMAGES),
    // An `object` or `embed` inside another embed (a fallback) is the same embed.
    embeds: [...doc.querySelectorAll(EMBEDS)].filter((el) => el.parentElement?.closest(EMBEDS) == null)
      .length,
  };
  removeAll(doc, EMBEDS);
  removeAll(doc, DECORATION);
  // Copied from a description editor (ProseMirror marks its own HTML): its tables are already simple, and a few rows
  // copied out of one must stay body rows when pasted into another table.
  if (doc.querySelector("[data-pm-slice]") === null) {
    for (const table of doc.querySelectorAll("table")) simplifyTable(table);
  }
  markTaskLists(doc);
  return { html: template.innerHTML, dropped };
}

/** Takes out what `selector` finds, and the paragraphs, links and spans left empty by it; how many it found. */
function removeAll(doc: ParentNode, selector: string): number {
  const found = [...doc.querySelectorAll(selector)];
  for (const el of found) {
    let emptied = el.parentElement;
    el.remove();
    while (emptied !== null && isEmptyWrapper(emptied)) {
      const parent = emptied.parentElement;
      emptied.remove();
      emptied = parent;
    }
  }
  return found.length;
}

/** An element with nothing left in it that isn't part of a list or table's structure. */
function isEmptyWrapper(el: Element): boolean {
  const structural = ["li", "td", "th", "tr", "table", "pre", "code"];
  if (structural.includes(el.tagName.toLowerCase())) return false;
  return el.textContent.trim() === "" && el.querySelector("*:not(br, span, b, i, a, p, div)") === null;
}

/**
 * Rebuilds a table as a plain grid: a cell spanning several columns or rows keeps its text in its first place and
 * empty cells fill the rest; short rows are filled out to the widest. The first row is the header, the others body
 * rows; no cell keeps an alignment.
 */
function simplifyTable(table: HTMLTableElement): void {
  const doc = table.ownerDocument;
  const rows = [...table.rows];
  const grid: (HTMLTableCellElement | null)[][] = rows.map(() => []);
  rows.forEach((row, r) => {
    let c = 0;
    for (const cell of [...row.cells]) {
      while (grid[r]?.[c] !== undefined) c++;
      const colSpan = Math.max(1, cell.colSpan);
      const rowSpan = Math.max(1, Math.min(cell.rowSpan || 1, rows.length - r));
      for (let dr = 0; dr < rowSpan; dr++) {
        for (let dc = 0; dc < colSpan; dc++) {
          const line = grid[r + dr];
          if (line) line[c + dc] = dr === 0 && dc === 0 ? cell : null;
        }
      }
      c += colSpan;
    }
  });
  const width = Math.max(0, ...grid.map((line) => line.length));
  rows.forEach((row, r) => {
    const tag = r === 0 ? "th" : "td";
    const cells = Array.from({ length: width }, (_, c) => grid[r]?.[c] ?? null).map((cell) =>
      cell === null ? doc.createElement(tag) : asCell(cell, tag),
    );
    row.replaceChildren(...cells);
  });
}

/** `cell` as a `th` or `td` holding the same content, with no span or alignment. */
function asCell(cell: HTMLTableCellElement, tag: "th" | "td"): HTMLTableCellElement {
  const out = cell.tagName.toLowerCase() === tag ? cell : cell.ownerDocument.createElement(tag);
  if (out !== cell) out.replaceChildren(...cell.childNodes);
  for (const name of ["colspan", "rowspan", "align", "style", "valign"]) out.removeAttribute(name);
  // A header cell is drawn bold already; bold inside it (Google Docs sets it) would save as `**Field**`.
  if (tag === "th") {
    for (const bold of out.querySelectorAll("b, strong")) bold.replaceWith(...bold.childNodes);
    for (const span of out.querySelectorAll<HTMLElement>("[style]")) span.style.removeProperty("font-weight");
  }
  return out;
}

/** A list item starting with a checkbox (GitHub, many Markdown renderers) as a task item, its list as a task list. */
function markTaskLists(doc: ParentNode): void {
  for (const box of doc.querySelectorAll<HTMLInputElement>('li input[type="checkbox"]')) {
    const item = box.closest("li");
    const list = item?.parentElement;
    if (!item || list?.tagName.toLowerCase() !== "ul" || !startsWith(item, box)) continue;
    item.setAttribute("data-type", "taskItem");
    item.setAttribute("data-checked", String(box.checked || box.hasAttribute("checked")));
    list.setAttribute("data-type", "taskList");
    box.remove();
  }
}

/** Whether `inner` comes before any text in `outer`. */
function startsWith(outer: Element, inner: Element): boolean {
  const walker = outer.ownerDocument.createTreeWalker(outer, NodeFilter.SHOW_ALL);
  for (let node = walker.nextNode(); node !== null; node = walker.nextNode()) {
    if (node === inner) return true;
    if (node.nodeType === Node.TEXT_NODE && (node.textContent ?? "").trim() !== "") return false;
  }
  return false;
}
