// Copying from programs (d0117 point 7): a program that copies through the terminal (OSC 52: Claude Code's copy, vim,
// tmux) puts its text on the clipboard. Write only: a program asking for the clipboard (`?`) gets no answer at all,
// and output replayed from an earlier moment never copies again.
import type { IDisposable, Terminal } from "@xterm/xterm";

import { decodeBase64 } from "./input";

/**
 * What one OSC 52 sequence (`Pc;Pd`, the part after `52;`) copies: the text, or `undefined` for nothing (a query, a
 * clear, a payload that isn't base64). Every selection (`c`, `p`, `s`, none) is the one clipboard.
 */
export function copiedText(data: string): string | undefined {
  const split = data.indexOf(";");
  if (split < 0) return undefined;
  const payload = data.slice(split + 1);
  // A query is never answered; an empty payload (clear) leaves the user's clipboard as it is.
  if (payload === "" || payload === "?") return undefined;
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(decodeBase64(payload));
  } catch {
    return undefined;
  }
}

/**
 * Handles OSC 52 for `term`: `write` gets the copied text, unless `replaying()` says the output is a replay. Every
 * OSC 52 counts as handled, so nothing else answers a query. Returns the stop.
 */
export function copyFromPrograms(
  term: Pick<Terminal, "parser">,
  replaying: () => boolean,
  write: (text: string) => void,
): IDisposable {
  return term.parser.registerOscHandler(52, (data) => {
    const text = replaying() ? undefined : copiedText(data);
    if (text !== undefined) write(text);
    return true;
  });
}
