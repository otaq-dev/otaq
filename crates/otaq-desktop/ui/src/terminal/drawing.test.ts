// @vitest-environment happy-dom
// The tools' character widths (d0117 point 6), measured with the real xterm.js: where the cursor lands after a string.
import { Terminal } from "@xterm/xterm";
import { describe, expect, it } from "vitest";

import { countWidthsAsUnicode11 } from "./drawing";

// Claude Code's spinner and bullets, then an emoji: 1 + 1 + 1 + 2 columns by Unicode 11.
const SYMBOLS = "✻●⏺😀";

function columnsOf(term: Terminal, text: string): Promise<number> {
  return new Promise((resolve) => {
    term.write(text, () => {
      resolve(term.buffer.active.cursorX);
    });
  });
}

describe("character widths", () => {
  it("are Unicode 11's once it is active: the emoji takes two columns", async () => {
    const term = new Terminal({ allowProposedApi: true });
    countWidthsAsUnicode11(term);
    expect(term.unicode.activeVersion).toBe("11");
    expect(await columnsOf(term, SYMBOLS)).toBe(5);
    term.dispose();
  });

  it("stay Unicode 11's after a reset, which starts every replay", async () => {
    const term = new Terminal({ allowProposedApi: true });
    countWidthsAsUnicode11(term);
    term.reset();
    expect(term.unicode.activeVersion).toBe("11");
    expect(await columnsOf(term, SYMBOLS)).toBe(5);
    term.dispose();
  });

  it("differ from xterm.js's own Unicode 6 widths, which the tools don't use", async () => {
    const term = new Terminal({ allowProposedApi: true });
    expect(term.unicode.activeVersion).toBe("6");
    expect(await columnsOf(term, SYMBOLS)).toBe(4);
    term.dispose();
  });
});
