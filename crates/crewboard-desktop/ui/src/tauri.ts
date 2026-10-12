// The one TypeScript file that names Tauri (UIT-02). Everything else imports these functions, so a Tauri upgrade
// touches only this file and the Rust side's `src/door.rs`. Screens don't edit it: they call `request` with their own
// method names and use the generated types for what goes in and comes back.
import { Channel, invoke } from "@tauri-apps/api/core";

import type { TerminalFeed } from "./generated/TerminalFeed";

/**
 * The one request call: a method name and JSON params in, the JSON reply out. Rejects with the Rust side's
 * `RequestError` (see `asRequestError` in `service.ts`).
 */
export function request(method: string, params: unknown = null): Promise<unknown> {
  return invoke("request", { method, params });
}

/** Subscribes to the app's event stream; `onEvent` gets every event from now on. */
export async function subscribeEvents(onEvent: (event: unknown) => void): Promise<void> {
  const channel = new Channel<unknown>();
  channel.onmessage = onEvent;
  await invoke("subscribe_events", { channel });
}

/**
 * Attaches a terminal to a session over its own channel; `onFeed` gets its stored output, then live output and phase
 * changes, until `terminal.detach` with the number this resolves to.
 */
export function attachTerminal(session: string, onFeed: (feed: TerminalFeed) => void): Promise<number> {
  const channel = new Channel<TerminalFeed>();
  channel.onmessage = onFeed;
  return invoke<number>("attach_terminal", { session, channel });
}
