import { beforeEach, describe, expect, it, vi } from "vitest";

import { createStore } from "../../app/store";
import {
  askEach,
  chipPressed,
  dropIndex,
  editStage,
  importChoice,
  keyMove,
  nextTile,
  sendFirst,
} from "./logic";

const ask = vi.fn<(method: string, params?: unknown) => Promise<unknown>>();

beforeEach(() => {
  ask.mockReset();
});

describe("the flow editor's calls", () => {
  it("an edit goes to the draft and hands back the stage to select", async () => {
    ask.mockResolvedValueOnce({ selected: "plan" });
    expect(await editStage("SHOP", { edit: "add", after: "grill" }, ask)).toBe("plan");
    expect(ask).toHaveBeenCalledWith("flow_editor.edit", {
      project: "SHOP",
      edit: { edit: "add", after: "grill" },
    });
  });

  it("nothing starts for empty words; words start the agent the panel then shows", async () => {
    const agents = createStore<Readonly<Record<string, string>>>({});
    expect(await sendFirst("SHOP", "   ", ask, agents)).toBeNull();
    expect(ask).not.toHaveBeenCalled();
    ask.mockResolvedValueOnce({ agent: "pa-1" });
    expect(await sendFirst("SHOP", "What’s Validate?", ask, agents)).toBe("pa-1");
    expect(ask).toHaveBeenCalledWith("flow_editor.send", { project: "SHOP", text: "What’s Validate?" });
    expect(agents.get()).toEqual({ SHOP: "pa-1" });
  });

  it("a chip sends its words, opens the chooser, or asks the one tool", async () => {
    const agents = createStore<Readonly<Record<string, string>>>({});
    const chooser = vi.fn();
    await chipPressed("SHOP", "onboarding", { kind: "choose_tool" }, chooser, ask, agents);
    expect(chooser).toHaveBeenCalledTimes(1);
    expect(ask).not.toHaveBeenCalled();
    ask.mockResolvedValue({ agent: "a-cc" });
    await chipPressed("SHOP", "onboarding", { kind: "ask", tool: "claude" }, chooser, ask, agents);
    expect(ask).toHaveBeenCalledWith("flow_editor.ask", {
      project: "SHOP",
      from: "onboarding",
      tool: "claude",
    });
    await chipPressed(
      "SHOP",
      "project",
      { kind: "send", message: "Why is Security here?" },
      chooser,
      ask,
      agents,
    );
    expect(ask).toHaveBeenLastCalledWith("flow_editor.send", {
      project: "SHOP",
      text: "Why is Security here?",
    });
  });

  it("Both, and compare asks each tool", async () => {
    ask.mockResolvedValue({ agent: "x" });
    await askEach("SHOP", "onboarding", ["claude", "opencode"], ask, createStore({}));
    expect(ask.mock.calls.map((c) => (c[1] as { tool: string }).tool)).toEqual(["claude", "opencode"]);
  });

  it("Import a flow… reads a picked file, or copies another project's flow; a cancelled dialog does nothing", async () => {
    ask.mockResolvedValue(null);
    await importChoice("SHOP", "file", () => Promise.resolve(null), ask);
    expect(ask).not.toHaveBeenCalled();
    await importChoice("SHOP", "file", () => Promise.resolve("/tmp/team.yaml"), ask);
    expect(ask).toHaveBeenCalledWith("flow_editor.import", { project: "SHOP", path: "/tmp/team.yaml" });
    await importChoice("SHOP", "project:BILL", () => Promise.resolve(null), ask);
    expect(ask).toHaveBeenLastCalledWith("flow_editor.pick", {
      project: "SHOP",
      pick: { pick: "project", project: "BILL" },
    });
  });
});

describe("moving a step", () => {
  // Five stage rows, 40 px apart.
  const middles = [20, 60, 100, 140, 180];

  it("a drag lands where the pointer is among the other rows", () => {
    expect(dropIndex(middles, 10, 3)).toBe(0);
    expect(dropIndex(middles, 70, 3)).toBe(2);
    expect(dropIndex(middles, 145, 0)).toBe(3);
    expect(dropIndex(middles, 500, 0)).toBe(4);
    expect(dropIndex(middles, 120, 3)).toBe(3);
  });

  it("Alt+↑ / Alt+↓ move one place, never past the ends", () => {
    expect(keyMove(2, 5, "ArrowUp")).toBe(1);
    expect(keyMove(0, 5, "ArrowUp")).toBeNull();
    expect(keyMove(4, 5, "ArrowDown")).toBeNull();
    expect(keyMove(1, 5, "ArrowDown")).toBe(2);
    expect(keyMove(1, 5, "x")).toBeNull();
  });

  it("the walking ticket wraps", () => {
    expect(nextTile(0, 5)).toBe(1);
    expect(nextTile(4, 5)).toBe(0);
    expect(nextTile(0, 0)).toBe(0);
  });
});
