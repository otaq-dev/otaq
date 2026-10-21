// Agents at work (T-0434; ASK-06): asking a tool, then another, gives a tab each; Stop ends one and the other goes
// on; Try again after the limit and after an end; Stop all; ⌘L inside the step; the card's rows as Rust words them.
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { Keymap } from "../../generated/Keymap";
import { AgentsAtWork, FlowTerminals } from "./FlowAtWork";
import {
  atWork,
  ended,
  notAsked,
  notSignedIn,
  recorder,
  stepAtWork,
  stopped,
  working,
} from "./flow_fixtures";
import { actOnRow, askTool, flowStepChanged, focusesAgent, stopAll, watchedTab } from "./logic";

const ignore = () => undefined;

function card(view: FlowAtWorkView, watched: string | null = watchedTab(view, null)): string {
  return renderToStaticMarkup(
    createElement(AgentsAtWork, {
      view,
      watched,
      busy: false,
      onWatch: ignore,
      onAct: ignore,
      onStopAll: ignore,
      onSkip: ignore,
    }),
  );
}

function tabs(view: FlowAtWorkView, watched: string | null = watchedTab(view, null)): string {
  return renderToStaticMarkup(
    createElement(FlowTerminals, { project: "SHOP", view, watched, onWatch: ignore }),
  );
}

/** How many tabs a drawing has. */
const tabCount = (html: string) => html.match(/role="tab"/g)?.length ?? 0;

describe("agents at work", () => {
  it("Ask Claude Code, then Ask OpenCode: a tab each, the first still shown", async () => {
    const { asked, ask } = recorder({ "onboarding.ask": { id: "a-cc", session: "s-cc" } });
    await askTool("SHOP", "claude", ask);
    // The go-ahead moves on to the agents at work once Rust says a tool was asked.
    const one = stepAtWork([working("claude", "a-cc", 0), notAsked("opencode")]);
    expect(flowStepChanged({ at: "go_ahead" }, one)).toEqual({ at: "at_work" });
    expect(tabCount(tabs(one.at_work))).toBe(1);
    const first = card(one.at_work);
    expect(first).toContain("Watching");
    expect(first).toContain("Ask OpenCode");

    // OpenCode is asked from its row while Claude Code works.
    const askOpenCode = one.at_work.rows[1]?.buttons[0];
    expect(askOpenCode?.action).toEqual({ kind: "ask", tool: "opencode" });
    if (askOpenCode?.action.kind === "ask") await actOnRow("SHOP", askOpenCode.action, ask);
    expect(asked).toEqual([
      ["onboarding.ask", { project: "SHOP", tool: "claude" }],
      ["onboarding.ask", { project: "SHOP", tool: "opencode" }],
    ]);
    const two = atWork([working("claude", "a-cc", 2), working("opencode", "a-oc", 1)]);
    const drawn = tabs(two, watchedTab(two, "a-cc"));
    expect(tabCount(drawn)).toBe(2);
    expect(drawn).toContain("Working · 2 min");
    expect(drawn).toContain("Working · 1 min");
    expect(drawn).toMatch(/aria-selected="true"[^>]*>.*?Claude Code/);
    expect(watchedTab(two, null)).toBe("a-cc");
    const both = card(two, "a-cc");
    expect(both.match(/Watching/g)).toHaveLength(1);
    expect(both).toMatch(/>Watch</);
    expect(both.match(/>Stop</g)).toHaveLength(2);
  });

  it("Stop ends one agent (into Recently ended) and the other goes on", async () => {
    const { asked, ask } = recorder();
    await actOnRow("SHOP", { kind: "stop", agent: "a-cc" }, ask);
    expect(asked).toEqual([["pane.end", { project: "SHOP", id: "a-cc", confirm: true }]]);
    const after = atWork([ended("claude", "a-cc"), working("opencode", "a-oc", 2)]);
    const html = card(after, "a-cc");
    expect(html).toContain("Ended without a proposal");
    expect(html).toContain("Try again");
    expect(html).toContain("Working · 2 min");
    expect(after.running).toEqual(["a-oc"]);
    // Its tab stays, read-only; the shown tab doesn't move.
    expect(tabCount(tabs(after, "a-cc"))).toBe(2);
    expect(watchedTab(after, "a-cc")).toBe("a-cc");
    // A tab that left: the first shows.
    expect(watchedTab(atWork([working("opencode", "a-oc", 2)]), "a-cc")).toBe("a-oc");
  });

  it("Try again continues the same session after the limit, and resumes the conversation once it ended", async () => {
    const limit = recorder();
    await actOnRow("SHOP", { kind: "resume_tool", tool: "claude" }, limit.ask);
    expect(limit.asked).toEqual([["limits.resume_tool", { tool: "claude" }]]);
    const gone = recorder();
    await actOnRow("SHOP", { kind: "resume", agent: "a-cc" }, gone.ask);
    expect(gone.asked).toEqual([["pane.resume", { id: "a-cc" }]]);
  });

  it("Stop all ends every agent at work or stopped, then shows the presets", async () => {
    const { asked, ask } = recorder();
    const view = atWork([stopped("claude", "a-cc"), working("opencode", "a-oc", 3)]);
    expect(await stopAll("SHOP", view, ask)).toEqual({ at: "presets" });
    expect(asked).toEqual([
      ["pane.end", { project: "SHOP", id: "a-cc", confirm: true }],
      ["pane.end", { project: "SHOP", id: "a-oc", confirm: true }],
    ]);
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

  it("draws each row as Rust words it: the plan limit, not signed in, not asked", () => {
    const limited = card(atWork([stopped("claude", "a-cc"), working("opencode", "a-oc", 3)]), "a-cc");
    expect(limited).toContain("Plan limit · resets 3:45 pm · picks up where it stopped");
    expect(limited).toMatch(/<button[^>]*disabled[^>]*>.*Try again at 3:45 pm/);
    expect(limited).toContain("Stop all and use a preset");
    expect(limited).toContain("Skip, use the default");
    const signedOut = card(atWork([working("claude", "a-cc", 2), notSignedIn("opencode")]));
    expect(signedOut).toContain("Couldn’t start · not signed in");
    expect(signedOut).toContain("Sign in…");
    expect(signedOut).toMatch(/data-error="true"/);
    expect(tabCount(tabs(atWork([notAsked("claude")])))).toBe(0);
  });
});
