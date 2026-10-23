// @vitest-environment happy-dom
// Copying from programs (d0117 point 7, T-0484), through the real xterm.js parser: an OSC 52 write copies, a query gets
// no answer, a replay never copies.
import { Terminal } from "@xterm/xterm";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { copiedText, copyFromPrograms } from "./clipboard";
import { encodeBase64 } from "./input";

const b64 = (text: string) => encodeBase64(new TextEncoder().encode(text));

describe("an OSC 52 sequence", () => {
  it("copies its base64 text, UTF-8, whichever selection it names", () => {
    expect(copiedText(`c;${b64("hello")}`)).toBe("hello");
    expect(copiedText(`;${b64("from tmux")}`)).toBe("from tmux");
    expect(copiedText(`p;${b64("ünïcødé ✻")}`)).toBe("ünïcødé ✻");
  });

  it("copies nothing for a query, an empty payload or one that isn't base64 UTF-8", () => {
    expect(copiedText("c;?")).toBeUndefined();
    expect(copiedText("c;")).toBeUndefined();
    expect(copiedText("c;not base64!")).toBeUndefined();
    expect(copiedText("c")).toBeUndefined();
    expect(copiedText(`c;${btoa("\xff\xfe")}`)).toBeUndefined();
  });
});

describe("copying from programs", () => {
  let term: Terminal;
  let copied: string[];
  let sent: string[];
  let replaying: boolean;

  beforeEach(() => {
    term = new Terminal({ allowProposedApi: true });
    copied = [];
    sent = [];
    replaying = false;
    copyFromPrograms(
      term,
      () => replaying,
      (text) => copied.push(text),
    );
    term.onData((data) => sent.push(data));
  });

  afterEach(() => {
    term.dispose();
  });

  const write = (text: string) =>
    new Promise<void>((resolve) => {
      term.write(text, resolve);
    });

  it("puts a program's copy on the clipboard (BEL and ST endings)", async () => {
    await write(`\x1b]52;c;${b64("npm run build")}\x07`);
    await write(`\x1b]52;c;${b64("second")}\x1b\\`);
    expect(copied).toEqual(["npm run build", "second"]);
  });

  it("answers a query with nothing: no clipboard text goes to the program", async () => {
    await write("\x1b]52;c;?\x07");
    await write("\x1b]52;;?\x1b\\");
    expect(copied).toEqual([]);
    expect(sent).toEqual([]);
  });

  it("never copies from replayed output", async () => {
    replaying = true;
    await write(`\x1b]52;c;${b64("old copy")}\x07`);
    replaying = false;
    await write(`\x1b]52;c;${b64("new copy")}\x07`);
    expect(copied).toEqual(["new copy"]);
  });
});
