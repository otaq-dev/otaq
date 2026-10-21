// The agent at work (T-0430; ONB-05): the start's check for a tool that isn't signed in, Stop and use a preset, Try
// again on each failure, ⌘L inside the step, and the four cards as Rust words them.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { Keymap } from "../../generated/Keymap";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import { FlowAtWork } from "./FlowAtWork";
import { flowAgent, flowStarted, focusesAgent, retryFlowAgent, startFlowAgent, stopFlowAgent } from "./logic";

const STEP = { project: "SHOP" } as OnboardingFlowStep;

const CARD: FlowAtWorkView = {
  kind: "working",
  title: "Reading how you work",
  text: "This can take a few minutes. Watch the agent in its terminal, or come back later: the proposal waits for you.",
  elapsed: "Working · 2 min",
  problem: null,
  stop: "Stop and use a preset",
  skip: null,
  use_preset: null,
  retry: null,
  pill: null,
  foot: null,
};

const FAILURE = {
  ...CARD,
  elapsed: null,
  stop: null,
  skip: "Skip, use the default",
  use_preset: "Use a preset instead",
};

const STOPPED: FlowAtWorkView = {
  ...FAILURE,
  kind: "stopped",
  title: "The agent stopped",
  text: "Claude Code reached its plan limit before the proposal was ready. Try again at 3:45 pm · it picks up where it stopped.",
  problem: "Claude Code limit reached · resets at 3:45 pm (in 1 h 12 min).",
  retry: {
    label: "Try again at 3:45 pm",
    icon: "clock",
    enabled: false,
    action: { kind: "resume_tool", tool: "claude" },
  },
  pill: { palette: "queued", icon: "circle-pause", label: "Stopped" },
};

const NOT_SIGNED_IN: FlowAtWorkView = {
  ...FAILURE,
  kind: "not_signed_in",
  title: "The agent couldn’t start",
  text: "Codex isn’t signed in on this computer.",
  problem: "Codex: not signed in.",
  retry: { label: "Sign in…", icon: "log-in", enabled: true, action: { kind: "sign_in", tool: "codex" } },
};

const ENDED: FlowAtWorkView = {
  ...FAILURE,
  kind: "ended",
  title: "The agent ended without a proposal",
  text: "Its conversation ended before it proposed a flow. Try again to pick up where it stopped.",
  retry: { label: "Try again", icon: "rotate-ccw", enabled: true, action: { kind: "resume", agent: "pa-1" } },
  foot: "Read-only · the session ended",
};

function recorder(answers: Record<string, unknown> = {}) {
  const asked: [string, unknown][] = [];
  const ask = (method: string, params?: unknown) => {
    asked.push([method, params]);
    return Promise.resolve(answers[method] ?? null);
  };
  return { asked, ask };
}

function draw(view: FlowAtWorkView): string {
  const ignore = () => undefined;
  return renderToStaticMarkup(
    createElement(FlowAtWork, {
      view,
      busy: false,
      onStop: ignore,
      onSkip: ignore,
      onPresets: ignore,
      onRetry: ignore,
    }),
  );
}

describe("the agent at work", () => {
  it("doesn't start while the Project agent's tool isn't signed in: the card says so", async () => {
    const { asked, ask } = recorder({ "onboarding.at_work": NOT_SIGNED_IN });
    const started = await startFlowAgent(STEP, ask);
    expect(started).toEqual({ kind: "cannot_start", card: NOT_SIGNED_IN });
    expect(asked).toEqual([["onboarding.at_work", { project: "SHOP" }]]);
    const state = flowStarted(started);
    expect(state).toEqual({ at: "cannot_start", card: NOT_SIGNED_IN });
    expect(flowAgent(state)).toBeNull();
    expect(flowStarted({ kind: "started", agent: "pa-1" })).toEqual({ at: "working", agent: "pa-1" });
  });

  it("Stop and use a preset ends the agent (into Recently ended) and shows the presets", async () => {
    const { asked, ask } = recorder();
    expect(await stopFlowAgent("SHOP", "pa-1", ask)).toEqual({ at: "presets", agent: null });
    expect(asked).toEqual([["pane.end", { project: "SHOP", id: "pa-1", confirm: true }]]);
  });

  it("Try again continues the same session after the limit, and resumes the conversation once it ended", async () => {
    const limit = recorder();
    await retryFlowAgent({ kind: "resume_tool", tool: "claude" }, limit.ask);
    expect(limit.asked).toEqual([["limits.resume_tool", { tool: "claude" }]]);
    const ended = recorder();
    await retryFlowAgent({ kind: "resume", agent: "pa-1" }, ended.ask);
    expect(ended.asked).toEqual([["pane.resume", { id: "pa-1" }]]);
  });

  it("⌘L inside the step is the user's Focus the pane", () => {
    const map = {
      platform: "mac",
      log: false,
      letters: true,
      actions: [],
      sequences: [],
      terminal_keys: [],
      terminal_app_modifiers: [],
      bindings: [
        {
          action: { action: "focus_pane" },
          slot: 0,
          text: "⌘L",
          key: {
            kind: "stroke",
            stroke: {
              key: "l",
              code: "KeyL",
              modifiers: { control: false, alt: false, shift: false, platform: true, function: false },
            },
          },
          place: "everywhere",
          custom: false,
        },
      ],
    } satisfies Keymap;
    const key = (k: string, meta: boolean) => ({
      key: k,
      code: `Key${k.toUpperCase()}`,
      metaKey: meta,
      ctrlKey: false,
      altKey: false,
      shiftKey: false,
    });
    expect(focusesAgent(map, key("l", true), false)).toBe(true);
    expect(focusesAgent(map, key("l", true), true)).toBe(true);
    expect(focusesAgent(map, key("l", false), false)).toBe(false);
    expect(focusesAgent(map, key("k", true), false)).toBe(false);
    expect(focusesAgent(undefined, key("l", true), false)).toBe(false);
  });

  it("draws the four cards as Rust words them, with no per-source rows", () => {
    const working = draw(CARD);
    expect(working).toContain("Reading how you work");
    expect(working).toContain("Working · 2 min");
    expect(working).toContain("Stop and use a preset");
    expect(working).not.toContain("flow-at-work-problem");

    const stopped = draw(STOPPED);
    expect(stopped).toContain("Claude Code limit reached · resets at 3:45 pm (in 1 h 12 min).");
    expect(stopped).toMatch(/<button[^>]*disabled[^>]*>.*Try again at 3:45 pm/);
    expect(stopped).toContain("Skip, use the default");
    expect(stopped).toContain("Use a preset instead");

    expect(draw(NOT_SIGNED_IN)).toContain("Sign in…");
    const ended = draw(ENDED);
    expect(ended).toContain("The agent ended without a proposal");
    expect(ended).not.toMatch(/<button[^>]*disabled/);
  });
});
