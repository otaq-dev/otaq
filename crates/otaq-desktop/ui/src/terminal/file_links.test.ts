// File-path links (d0117 point 8): lines read from the buffer with their cells, one ask per screen of new lines, the
// answers kept briefly, and links mapped back to their cells.
import type { IBuffer, IBufferCell, IBufferLine, ILink } from "@xterm/xterm";
import { describe, expect, it } from "vitest";

import type { FileLink } from "../generated/FileLink";
import { FRESH_MS, filePathLinks, logicalLine } from "./file_links";

/** A buffer of `rows`, each a string of cells (a wide character takes its cell and an empty one after it). */
function bufferOf(rows: { text: string; wrapped?: boolean }[], viewportY = 0): IBuffer {
  const lines: IBufferLine[] = rows.map(({ text, wrapped = false }) => {
    const cells: { chars: string; width: number }[] = [];
    for (const ch of text) {
      const wide = ch === "📦";
      cells.push({ chars: ch, width: wide ? 2 : 1 });
      if (wide) cells.push({ chars: "", width: 0 });
    }
    return {
      isWrapped: wrapped,
      length: cells.length,
      getCell: (x: number) => {
        const c = cells[x];
        return c ? ({ getChars: () => c.chars, getWidth: () => c.width } as IBufferCell) : undefined;
      },
    } as IBufferLine;
  });
  return { viewportY, getLine: (y: number) => lines[y] } as unknown as IBuffer;
}

describe("logicalLine", () => {
  it("joins wrapped rows and maps each character to its cell, wide ones included", () => {
    const buffer = bufferOf([{ text: "📦 src/a" }, { text: ".ts:4", wrapped: true }, { text: "next" }]);
    const line = logicalLine(buffer, 1, 80);
    expect(line?.text).toBe("📦 src/a.ts:4");
    expect(line?.first).toBe(0);
    expect(line?.last).toBe(1);
    // "📦" is two UTF-16 units in cell 1; the space is in cell 3.
    expect(line?.cells.slice(0, 4)).toEqual([
      { x: 1, y: 1 },
      { x: 1, y: 1 },
      { x: 3, y: 1 },
      { x: 4, y: 1 },
    ]);
    expect(line?.cells.at(-1)).toEqual({ x: 5, y: 2 });
  });
});

describe("filePathLinks", () => {
  const link = (start: number, end: number, path: string, line: number | null): FileLink => ({
    start,
    end,
    path,
    line,
  });

  function setup(rows: { text: string; wrapped?: boolean }[]) {
    const asked: string[][] = [];
    const opened: [string, number | null][] = [];
    let time = 1000;
    const answers = new Map<string, FileLink[]>([
      ["Edited src/a.ts:42", [link(7, 18, "src/a.ts", 42)]],
      ["and src/b.ts", [link(4, 12, "src/b.ts", null)]],
    ]);
    const provider = filePathLinks({
      buffer: () => bufferOf(rows),
      rows: () => rows.length,
      cols: () => 80,
      find: (lines) => {
        asked.push(lines);
        return Promise.resolve(lines.map((l) => answers.get(l) ?? []));
      },
      open: (path, line) => opened.push([path, line]),
      now: () => time,
    });
    const linksAt = (row: number) =>
      new Promise<ILink[] | undefined>((resolve) => {
        provider.provideLinks(row, resolve);
      });
    return {
      asked,
      opened,
      linksAt,
      later: (ms: number) => {
        time += ms;
      },
    };
  }

  it("asks once for the whole screen, then answers from what it kept", async () => {
    const t = setup([{ text: "Edited src/a.ts:42" }, { text: "" }, { text: "and src/b.ts" }]);
    const first = await t.linksAt(1);
    expect(t.asked).toEqual([["Edited src/a.ts:42", "and src/b.ts"]]);
    expect(first?.map((l) => [l.text, l.range])).toEqual([
      ["src/a.ts:42", { start: { x: 8, y: 1 }, end: { x: 18, y: 1 } }],
    ]);
    expect((await t.linksAt(3))?.map((l) => l.text)).toEqual(["src/b.ts"]);
    expect(await t.linksAt(2)).toBeUndefined();
    expect(t.asked).toHaveLength(1);
    t.later(FRESH_MS);
    await t.linksAt(3);
    expect(t.asked).toHaveLength(2);
  });

  it("opens the file at its line when activated (the gate lets only ⌘-click through)", async () => {
    const t = setup([{ text: "Edited src/a.ts:42" }]);
    const [found] = (await t.linksAt(1)) ?? [];
    found?.activate({ metaKey: true } as MouseEvent, found.text);
    expect(t.opened).toEqual([["src/a.ts", 42]]);
  });
});
