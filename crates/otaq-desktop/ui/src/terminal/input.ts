// What a terminal sends to its session (SVC-06): the bytes typed or pasted, the terminal's own answers to the program,
// and its size. The service tracks the "unsent input" flag from typed bytes only (d0048), so answers are marked as
// replies; and everything goes out one request at a time, so keystrokes and resizes reach the PTY in order.

/** Decodes the service's base64 terminal bytes (they need not be UTF-8). */
export function decodeBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

export function encodeBase64(bytes: Uint8Array): string {
  let binary = "";
  // In slices: `String.fromCharCode` takes its bytes as arguments.
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(binary);
}

/** xterm.js's `onBinary` data: one byte per character (non-UTF-8 mouse reports). */
export function binaryBytes(data: string): Uint8Array {
  return Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff);
}

/* eslint-disable no-control-regex -- terminal answers are escape sequences */
const REPLIES = [
  // Cursor position, device attributes, status, mode and window reports: CSI … R / c / n / $y / t.
  /^\x1b\[[?>=]?[\d;]*(?:R|c|n|\$y|t)$/,
  // Focus in and out.
  /^\x1b\[[IO]$/,
  // Mouse reports: X10 (three bytes after CSI M), SGR and urxvt.
  /^\x1b\[M[\s\S]{3}$/,
  /^\x1b\[<\d+;\d+;\d+[Mm]$/,
  /^\x1b\[\d+;\d+;\d+M$/,
  // OSC answers (colours, clipboard) and DCS answers (settings, terminfo).
  /^\x1b\][\s\S]*(?:\x07|\x1b\\)$/,
  /^\x1bP[\s\S]*\x1b\\$/,
];
/* eslint-enable no-control-regex -- the answers end here */

/**
 * Whether xterm.js produced `data` as the terminal's answer to the program or as a mouse report, not as someone
 * typing: such bytes don't touch the unsent-input flag. xterm.js sends each answer as one piece. Shift+F3 sends the
 * same bytes as a cursor position report; it counts as an answer, which leaves the flag as it is.
 */
export function isReply(data: string): boolean {
  return REPLIES.some((pattern) => pattern.test(data));
}

/** One thing for the session, in the order it happened. */
export type Outgoing =
  { kind: "input"; data: Uint8Array; reply: boolean } | { kind: "resize"; cols: number; rows: number };

export interface Sender {
  push(item: Outgoing): void;
  /** Drops what hasn't gone out yet (the connection dropped; input is disabled meanwhile). */
  clear(): void;
}

/**
 * Sends `Outgoing`s one at a time, in order. What piles up while one request is on its way goes out together: typed
 * bytes of the same kind joined, only the newest of several resizes. `onError` hears failed sends; nothing is retried.
 */
export function createSender(
  send: (item: Outgoing) => Promise<unknown>,
  onError: (error: unknown) => void,
): Sender {
  const waiting: Outgoing[] = [];
  let busy = false;
  const pump = () => {
    const next = waiting.shift();
    if (next === undefined) {
      busy = false;
      return;
    }
    busy = true;
    send(next).then(pump, (error: unknown) => {
      onError(error);
      pump();
    });
  };
  return {
    push(item) {
      const last = waiting.at(-1);
      if (last?.kind === "input" && item.kind === "input" && last.reply === item.reply) {
        const joined = new Uint8Array(last.data.length + item.data.length);
        joined.set(last.data);
        joined.set(item.data, last.data.length);
        last.data = joined;
      } else if (last?.kind === "resize" && item.kind === "resize") {
        waiting[waiting.length - 1] = item;
      } else {
        waiting.push(item);
      }
      if (!busy) pump();
    },
    clear() {
      waiting.length = 0;
    },
  };
}
