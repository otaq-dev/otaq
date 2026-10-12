import { describe, expect, it } from "vitest";

import { binaryBytes, createSender, decodeBase64, encodeBase64, isReply, type Outgoing } from "./input";

describe("terminal bytes", () => {
  it("round-trips any bytes through base64, also past one slice", () => {
    const bytes = Uint8Array.from({ length: 0x8000 * 2 + 5 }, (_, i) => (i * 7) % 256);
    expect(decodeBase64(encodeBase64(bytes))).toEqual(bytes);
    // Not UTF-8: a lone continuation byte and 0xff.
    expect(decodeBase64(encodeBase64(Uint8Array.of(0x80, 0xff, 0x1b)))).toEqual(
      Uint8Array.of(0x80, 0xff, 0x1b),
    );
    expect(decodeBase64("aGk=")).toEqual(new TextEncoder().encode("hi"));
  });

  it("reads xterm.js's binary data as one byte per character", () => {
    expect(binaryBytes("\x1b[M\xa0\xa1")).toEqual(Uint8Array.of(0x1b, 0x5b, 0x4d, 0xa0, 0xa1));
  });
});

describe("what counts as the terminal answering", () => {
  it("knows the terminal's answers and mouse reports", () => {
    for (const reply of [
      "\x1b[12;40R", // cursor position
      "\x1b[?1;2c", // device attributes
      "\x1b[>0;276;0c",
      "\x1b[0n", // status
      "\x1b[?2004;1$y", // mode report
      "\x1b[8;24;80t", // window size
      "\x1b[I", // focus in
      "\x1b[O",
      "\x1b[M #!", // X10 mouse
      "\x1b[<0;12;5M", // SGR mouse
      "\x1b[<0;12;5m",
      "\x1b]11;rgb:1414/1616/1c1c\x1b\\", // background colour
      "\x1b]10;rgb:d6d6/d9d9/e0e0\x07",
      "\x1bP1$r0m\x1b\\", // DECRQSS
    ]) {
      expect(isReply(reply), JSON.stringify(reply)).toBe(true);
    }
  });

  it("treats keys and pastes as typing", () => {
    for (const typed of [
      "a",
      "hello world",
      "\r",
      "\x7f",
      "\x03",
      "\x1b",
      "\x1b[A", // ↑
      "\x1bOP", // F1
      "\x1b[15~", // F5
      "\x1b[Z", // ⇧Tab
      "\x1b\r", // ⌥↵
      "\x1b[200~pasted\ntext\x1b[201~",
      "über",
    ]) {
      expect(isReply(typed), JSON.stringify(typed)).toBe(false);
    }
  });
});

describe("the ordered sender", () => {
  function harness() {
    const sent: Outgoing[] = [];
    const errors: unknown[] = [];
    let release: (() => void) | undefined;
    let fail = false;
    const sender = createSender(
      (item) => {
        sent.push(item);
        return new Promise<void>((resolve, reject) => {
          release = () => {
            if (fail) reject(new Error("disconnected"));
            else resolve();
          };
        });
      },
      (e) => errors.push(e),
    );
    const settle = async () => {
      release?.();
      await Promise.resolve();
      await Promise.resolve();
    };
    return { sent, errors, sender, settle, failNext: () => (fail = true) };
  }
  const text = (s: string) => new TextEncoder().encode(s);
  const typed = (item: Outgoing | undefined) =>
    item?.kind === "input" ? `${item.reply ? "reply:" : ""}${new TextDecoder().decode(item.data)}` : item;

  it("sends one request at a time, in order, joining what piled up", async () => {
    const h = harness();
    h.sender.push({ kind: "input", data: text("e"), reply: false });
    h.sender.push({ kind: "input", data: text("c"), reply: false });
    h.sender.push({ kind: "input", data: text("h"), reply: false });
    h.sender.push({ kind: "input", data: text("\x1b[1;1R"), reply: true });
    h.sender.push({ kind: "input", data: text("o"), reply: false });
    expect(h.sent.map(typed)).toEqual(["e"]);
    await h.settle();
    expect(h.sent.map(typed)).toEqual(["e", "ch"]);
    await h.settle();
    await h.settle();
    expect(h.sent.map(typed)).toEqual(["e", "ch", "reply:\x1b[1;1R", "o"]);
  });

  it("keeps only the newest of several waiting resizes, in its place among the keys", async () => {
    const h = harness();
    h.sender.push({ kind: "input", data: text("a"), reply: false });
    h.sender.push({ kind: "resize", cols: 80, rows: 24 });
    h.sender.push({ kind: "resize", cols: 100, rows: 30 });
    h.sender.push({ kind: "input", data: text("b"), reply: false });
    for (let i = 0; i < 3; i++) await h.settle();
    expect(h.sent.map(typed)).toEqual(["a", { kind: "resize", cols: 100, rows: 30 }, "b"]);
  });

  it("carries on after a failed send and drops what waits on clear", async () => {
    const h = harness();
    h.sender.push({ kind: "input", data: text("a"), reply: false });
    h.sender.push({ kind: "input", data: text("b"), reply: false });
    h.failNext();
    await h.settle();
    expect(h.errors).toHaveLength(1);
    expect(h.sent.map(typed)).toEqual(["a", "b"]);
    h.sender.push({ kind: "input", data: text("c"), reply: false });
    h.sender.clear();
    await h.settle();
    expect(h.sent.map(typed)).toEqual(["a", "b"]);
    // Idle again: the next push goes out at once.
    h.sender.push({ kind: "input", data: text("d"), reply: false });
    expect(h.sent.map(typed)).toEqual(["a", "b", "d"]);
  });
});
