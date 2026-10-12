import { beforeEach, describe, expect, it, vi } from "vitest";

import { attachTerminal, request, subscribeEvents } from "./tauri";

// The door's calls must match the Rust commands' names and argument names in `src/door.rs`.
const tauri = vi.hoisted(() => {
  class Channel {
    onmessage: (message: unknown) => void = () => undefined;
  }
  return { invoke: vi.fn<(command: string, args: Record<string, unknown>) => Promise<unknown>>(), Channel };
});
vi.mock("@tauri-apps/api/core", () => tauri);

beforeEach(() => {
  tauri.invoke.mockReset();
  tauri.invoke.mockResolvedValue(null);
});

describe("the door", () => {
  it("sends a request as a method name and params, and returns the reply", async () => {
    tauri.invoke.mockResolvedValue({ ok: true });
    await expect(request("board.view", { project: "SHOP" })).resolves.toEqual({ ok: true });
    expect(tauri.invoke).toHaveBeenCalledWith("request", {
      method: "board.view",
      params: { project: "SHOP" },
    });
  });

  it("sends null params when a request has none", async () => {
    await request("service.state");
    expect(tauri.invoke).toHaveBeenCalledWith("request", { method: "service.state", params: null });
  });

  it("passes events and terminal output from their channels on", async () => {
    const events: unknown[] = [];
    const chunks: unknown[] = [];
    await subscribeEvents((event) => events.push(event));
    await attachTerminal("s1", (chunk) => chunks.push(chunk));
    const [eventCall, terminalCall] = tauri.invoke.mock.calls;
    expect(eventCall?.[0]).toBe("subscribe_events");
    expect(terminalCall?.[0]).toBe("attach_terminal");
    expect(terminalCall?.[1].session).toBe("s1");
    (eventCall?.[1].channel as InstanceType<typeof tauri.Channel>).onmessage({ kind: "resync" });
    (terminalCall?.[1].channel as InstanceType<typeof tauri.Channel>).onmessage("output");
    expect(events).toEqual([{ kind: "resync" }]);
    expect(chunks).toEqual(["output"]);
  });
});
