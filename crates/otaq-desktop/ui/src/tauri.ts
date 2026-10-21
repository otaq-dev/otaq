// The one TypeScript file that names Tauri (UIT-02). Everything else imports these functions, so a Tauri upgrade
// touches only this file and the Rust side's `src/door.rs`. Screens don't edit it: they call `request` with their own
// method names and use the generated types for what goes in and comes back.
import { Channel, invoke } from "@tauri-apps/api/core";

import type { TerminalFeed } from "./generated/TerminalFeed";
import type { WindowAction } from "./generated/WindowAction";

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

/** An action on the page's window: a caption button's (minimize, maximize or restore, close), or bringing it forward. */
export function windowAction(action: WindowAction): Promise<void> {
  return invoke("window", { action });
}

/** The system folder picker titled `title`: the folder chosen, or `null` when cancelled (Add project). */
export function pickFolder(title: string): Promise<string | null> {
  return invoke<string | null>("pick_folder", { title });
}

/** The system file picker for a flow file (Import flow…): the YAML file chosen, or `null` when cancelled. */
export function pickFlowFile(title: string): Promise<string | null> {
  return invoke<string | null>("pick_flow_file", { title });
}

/** The system save dialog for a flow file (Export flow…), offering `fileName`: where to save, or `null` when cancelled. */
export function saveFlowFile(title: string, fileName: string): Promise<string | null> {
  return invoke<string | null>("save_flow_file", { title, fileName });
}

/** The system save dialog for a report (the report view's Download), offering `fileName`: where to save, or `null`. */
export function saveReportFile(title: string, fileName: string): Promise<string | null> {
  return invoke<string | null>("save_report_file", { title, fileName });
}

/**
 * Spread on an element to make it the window's drag region, children included: pressing it moves the window, a double
 * click maximizes (zooms on macOS). Buttons, inputs and links inside keep their clicks.
 */
export const DRAG_REGION = { "data-tauri-drag-region": "deep" } as const;
