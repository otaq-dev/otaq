// File paths in a terminal's output as links (d0117 point 8). Which text is a path, and whether it names an existing
// file inside the session's folder, is Rust's (`file_links.find`, `otaq-view`'s `file_paths.rs`); this reads the
// lines xterm.js shows, asks for the whole screen at once when a line under the mouse isn't known yet, keeps the
// answers briefly, and maps each link back to its cells. The links open with ⌘-click through `links.ts`'s gate.
import type { IBuffer, ILink, ILinkProvider } from "@xterm/xterm";

import type { FileLink } from "../generated/FileLink";

/** How long a line's answer is kept: the files in a folder change while an agent works. */
export const FRESH_MS = 3000;

/** A line as written, its wrapped rows joined, with each UTF-16 unit's cell (1-based, as xterm.js's links count). */
export interface LogicalLine {
  text: string;
  cells: { x: number; y: number }[];
  /** Its first and last row (0-based). */
  first: number;
  last: number;
}

/** The line holding row `y` (0-based), wrapped rows joined; trailing blanks dropped. */
export function logicalLine(buffer: IBuffer, y: number, cols: number): LogicalLine | undefined {
  if (!buffer.getLine(y)) return undefined;
  let first = y;
  while (first > 0 && buffer.getLine(first)?.isWrapped) first--;
  let last = y;
  while (buffer.getLine(last + 1)?.isWrapped) last++;
  let text = "";
  const cells: { x: number; y: number }[] = [];
  for (let row = first; row <= last; row++) {
    const line = buffer.getLine(row);
    if (!line) break;
    for (let x = 0; x < Math.min(cols, line.length); x++) {
      const cell = line.getCell(x);
      if (!cell) continue;
      // The second half of a wide character has no text of its own.
      if (cell.getWidth() === 0) continue;
      const chars = cell.getChars() || " ";
      text += chars;
      for (let i = 0; i < chars.length; i++) cells.push({ x: x + 1, y: row + 1 });
    }
  }
  const trimmed = text.trimEnd();
  return { text: trimmed, cells: cells.slice(0, trimmed.length), first, last };
}

interface Known {
  at: number;
  links: Promise<FileLink[]>;
}

export interface FilePathLinksOptions {
  /** The terminal's active buffer and size, read on each hover. */
  buffer: () => IBuffer;
  rows: () => number;
  cols: () => number;
  /** Asks Rust for the links in `lines`, one answer per line. */
  find: (lines: string[]) => Promise<FileLink[][]>;
  /** Opens a link's file (gated: only on ⌘-click). */
  open: (path: string, line: number | null) => void;
  now?: () => number;
}

/** The provider of file-path links: one `find` per screen of lines not known yet, never one per hover. */
export function filePathLinks({
  buffer,
  rows,
  cols,
  find,
  open,
  now = Date.now,
}: FilePathLinksOptions): ILinkProvider {
  const known = new Map<string, Known>();

  const fresh = (text: string, time: number) => {
    const k = known.get(text);
    return k && time - k.at < FRESH_MS ? k : undefined;
  };

  /** The links of `text`, asking for every line of the screen not known yet along with it. */
  const linksOf = (text: string): Promise<FileLink[]> => {
    const time = now();
    const hit = fresh(text, time);
    if (hit) return hit.links;
    for (const [t, k] of known) if (time - k.at >= FRESH_MS) known.delete(t);
    const buf = buffer();
    const wanted = new Set([text]);
    for (let y = buf.viewportY; y < buf.viewportY + rows(); y++) {
      const line = logicalLine(buf, y, cols());
      if (line && line.text.trim() !== "" && !fresh(line.text, time)) wanted.add(line.text);
      if (line) y = Math.max(y, line.last);
    }
    const lines = [...wanted];
    const answer = find(lines);
    lines.forEach((t, i) => {
      const links = answer.then((all) => all[i] ?? []);
      // A failed ask is forgotten, so the next hover asks again.
      links.catch(() => {
        if (known.get(t)?.links === links) known.delete(t);
      });
      known.set(t, { at: time, links });
    });
    return known.get(text)?.links ?? Promise.resolve([]);
  };

  return {
    provideLinks: (row, callback) => {
      const line = logicalLine(buffer(), row - 1, cols());
      if (!line || line.text.trim() === "") {
        callback(undefined);
        return;
      }
      linksOf(line.text).then(
        (links) => {
          const out: ILink[] = [];
          for (const link of links) {
            const start = line.cells[link.start];
            const end = line.cells[link.end - 1];
            if (!start || !end || row < start.y || row > end.y) continue;
            out.push({
              range: { start, end },
              text: line.text.slice(link.start, link.end),
              activate: () => {
                open(link.path, link.line);
              },
            });
          }
          callback(out.length > 0 ? out : undefined);
        },
        () => {
          callback(undefined);
        },
      );
    },
  };
}
