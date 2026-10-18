import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { createRegistries, registered, registerScreens, RegistriesContext } from "../../app/registry";
import type { TerminalPhase } from "../../generated/TerminalPhase";
import type { TerminalTabStrip } from "../../generated/TerminalTabStrip";
import type { TerminalTabView } from "../../generated/TerminalTabView";
import { INITIAL_STATE, type TerminalState } from "../../terminal/state";
import { terminalTab } from ".";
import type { TerminalTabSide } from "../../generated/TerminalTabSide";
import {
  answers,
  bodyOf,
  isNarrow,
  scrolledLabel,
  sessionOf,
  shownPhase,
  shownSide,
  stripFor,
  toneColor,
} from "./logic";
import { TerminalTabBody } from "./TerminalTab";

const PHASES: TerminalPhase[] = ["no_session", "attaching", "live", "reconnecting", "ended"];

/** Strips worded like Rust's, enough to tell them apart. */
function strips(interrupted = false): TerminalTabStrip[] {
  return PHASES.flatMap((phase) =>
    [false, true].map((focused) => ({
      phase,
      focused,
      state: interrupted
        ? { label: "Interrupted", tone: "error" as const }
        : { label: `${phase}${focused ? " focused" : ""}`, tone: "working" as const },
      hint:
        phase === "live"
          ? {
              icon: "keyboard" as const,
              text: focused ? "Typing goes to the agent" : "Click the terminal to type to the agent",
              tone: focused ? ("focus" as const) : ("muted" as const),
            }
          : null,
    })),
  );
}

const VIEW: TerminalTabView = {
  key: "SHOP-19",
  session: "s1",
  label: "SHOP-19 · agent terminal",
  no_session: "SHOP-19 has no agent session yet. It starts when the ticket goes to Dev.",
  strips: strips(),
  path: "…/wt/shop-web/SHOP-19",
  port: "agent :4019",
  attach_command: "otaq attach SHOP-19",
  attach_short: "attach",
  copy_attach: "Copy attach command",
  waiting: null,
  instructions: { label: "Instructions: shop-web", project: "SHOP" },
  leave: { text: "leave terminal", keys: "⇧⌘`" },
  smaller: "Smaller text",
  bigger: "Larger text",
  find: { text: "Find in scrollback", keys: "⌘F" },
  background: null,
  narrow_below: 560,
  ended: null,
  recovery: null,
  agent_label: "Agent",
  agent_dot: "working",
  validator: null,
  lands_on: "agent",
};

/** After a round: the validator's own strips, worded so the tests tell them from the agent's. */
const VALIDATING: TerminalTabView = {
  ...VIEW,
  agent_dot: null,
  lands_on: "validator",
  validator: {
    session: "v1",
    label: "Validator · round 1",
    terminal_label: "SHOP-19 · validator terminal",
    dot: "working",
    read_only: false,
    strips: strips().map((s) => ({ ...s, state: { ...s.state, label: `validator ${s.state.label}` } })),
    attach_command: "otaq attach SHOP-19 --validator",
  },
};

const LIVE: TerminalState = { ...INITIAL_STATE, phase: "live" };

function render(
  view: TerminalTabView,
  state: TerminalState = LIVE,
  narrow = false,
  side: TerminalTabSide = "agent",
) {
  return renderToStaticMarkup(
    createElement(
      RegistriesContext,
      { value: createRegistries() },
      createElement(TerminalTabBody, { view, side, state, narrow }),
    ),
  );
}

describe("the Agent / Validator switch", () => {
  it("shows only once a validation ran", () => {
    expect(render(VIEW)).not.toContain("Validator");
    const html = render(VALIDATING, LIVE, false, "validator");
    expect(html).toContain("Agent");
    expect(html).toContain("Validator · round 1");
    expect(html).toMatch(/aria-selected="true"[^>]*>(<span[^>]*><\/span>)?Validator · round 1/);
  });

  it("lands on the side Rust names, keeps the user's pick until that changes", () => {
    expect(shownSide(VIEW, { ticket: "SHOP-19", landsOn: "agent", focus: 0, side: "validator" })).toBe(
      "agent",
    );
    expect(shownSide(VALIDATING, null)).toBe("validator");
    const picked = { ticket: "SHOP-19", landsOn: "validator" as const, focus: 0, side: "agent" as const };
    expect(shownSide(VALIDATING, picked)).toBe("agent");
    expect(shownSide({ ...VALIDATING, lands_on: "agent" }, picked)).toBe("agent");
    expect(shownSide(VALIDATING, { ...picked, landsOn: "agent" })).toBe("validator"); // a new round
    expect(shownSide(VALIDATING, { ...picked, ticket: "SHOP-20" })).toBe("validator");
    // ⌘J on "Needs you · validator": a newer focus request lands on the validator, focused.
    expect(shownSide(VALIDATING, picked, 3)).toBe("validator");
    // The panel's "Open terminal · Validator" asks for the validator whatever the tab would land on.
    expect(shownSide({ ...VALIDATING, lands_on: "agent" }, null, 4, "validator")).toBe("validator");
    expect(shownSide(VIEW, null, 4, "validator")).toBe("agent");
  });

  it("attaches the validator's session with its own strip and attach command", () => {
    expect(sessionOf(VALIDATING, "validator")).toBe("v1");
    expect(sessionOf(VALIDATING, "agent")).toBe("s1");
    expect(stripFor(VALIDATING, "live", false, "validator")?.state.label).toBe("validator live");
    expect(bodyOf({ ...VALIDATING, recovery: { kind: "loading", text: "…" } }, "validator")).toBe("terminal");
    const html = render(VALIDATING, LIVE, false, "validator");
    expect(html).toContain("otaq attach SHOP-19 --validator");
    expect(html).not.toContain("…/wt/shop-web/SHOP-19");
    expect(html).not.toContain("Instructions: shop-web");
  });
});

describe("the Terminal tab's state switching", () => {
  it("shows the terminal, the recovery view or the no-session line", () => {
    expect(bodyOf(VIEW)).toBe("terminal");
    expect(bodyOf({ ...VIEW, session: null })).toBe("no_session");
    expect(bodyOf({ ...VIEW, recovery: { kind: "loading", text: "Reading…" } })).toBe("recovery");
    expect(shownPhase({ ...VIEW, session: null }, "live")).toBe("no_session");
    expect(shownPhase(VIEW, "reconnecting")).toBe("reconnecting");
  });

  it("picks the strip of the terminal's phase and focus", () => {
    expect(stripFor(VIEW, "live", false)?.state.label).toBe("live");
    expect(stripFor(VIEW, "live", true)?.hint?.text).toBe("Typing goes to the agent");
    expect(stripFor(VIEW, "ended", false)?.hint).toBeNull();
    expect(stripFor({ ...VIEW, session: null }, "live", false)?.state.label).toBe("no_session");
    expect(toneColor("error")).toBe("var(--st-error)");
  });

  it("is narrow under the panel's icon-tab width", () => {
    expect(isNarrow(480, VIEW)).toBe(true);
    expect(isNarrow(660, VIEW)).toBe(false);
    expect(isNarrow(0, VIEW)).toBe(false);
  });

  it("answers each focus request for its ticket once", () => {
    expect(answers({ ticket: "SHOP-19", seq: 3 }, "SHOP-19", 2)).toBe(true);
    expect(answers({ ticket: "SHOP-19", seq: 3 }, "SHOP-19", 3)).toBe(false);
    expect(answers({ ticket: "SHOP-22", seq: 4 }, "SHOP-19", 3)).toBe(false);
    expect(answers(null, "SHOP-19", 0)).toBe(false);
  });

  it("counts scrolled-up lines", () => {
    expect(scrolledLabel(1)).toBe("Scrolled up 1 line");
    expect(scrolledLabel(1320)).toBe("Scrolled up 1,320 lines");
  });
});

describe("the Terminal tab's markup", () => {
  it("registers the panel's Terminal tab", () => {
    expect(registered(registerScreens({ terminalTab })).panelTabs).toEqual(["terminal"]);
  });

  it("draws the strip and the hint of a live terminal", () => {
    const html = render(VIEW);
    expect(html).toContain("…/wt/shop-web/SHOP-19");
    expect(html).toContain("agent :4019");
    expect(html).toContain("otaq attach SHOP-19");
    expect(html).toContain("Click the terminal to type to the agent");
    expect(html).toContain("Instructions: shop-web");
    expect(html).not.toContain("leave terminal");
  });

  it("says how to leave a focused terminal and draws its focus ring", () => {
    const html = render(VIEW, { ...LIVE, focused: true });
    expect(html).toContain("Typing goes to the agent");
    expect(html).toContain("leave terminal");
    expect(html).toContain("data-focused");
    expect(html).not.toContain("Instructions: shop-web");
  });

  it("drops the path and port when narrow", () => {
    const html = render(VIEW, LIVE, true);
    expect(html).not.toContain("…/wt/shop-web/SHOP-19");
    expect(html).toContain(">attach<");
  });

  it("offers Deliver now while messages wait", () => {
    const html = render({ ...VIEW, waiting: { label: "1 message waiting", deliver: "Deliver now" } });
    expect(html).toContain("1 message waiting");
    expect(html).toContain("Deliver now");
    expect(html).not.toContain("otaq attach SHOP-19");
  });

  it("marks an ended session and shows how far it is scrolled", () => {
    const ended = {
      ...VIEW,
      ended: { title: "Session ended · 11:02 · 41 min", text: "The agent’s session finished." },
    };
    expect(render(ended, { ...LIVE, phase: "ended" })).toContain("Session ended · 11:02 · 41 min");
    expect(render(ended, LIVE)).not.toContain("Session ended");
    expect(render(VIEW, { ...LIVE, scrolledUp: 1320 })).toContain("Scrolled up 1,320 lines");
  });

  it("shows the no-session line and Before the restart instead of a terminal", () => {
    expect(render({ ...VIEW, session: null })).toContain("has no agent session yet");
    const html = render({
      ...VIEW,
      strips: strips(true),
      recovery: {
        kind: "before_restart",
        title: "Before the restart",
        source: "from Claude’s saved conversation · read-only",
        session: "session 3be94c10",
        lines: [{ time: "08:09", who: "Bash", text: "pnpm build", action: true }],
        ended: "Session ended by a restart at 08:24.",
        resume: "Resume",
      },
    });
    expect(html).toContain("Interrupted");
    expect(html).toContain("Before the restart");
    expect(html).toContain("pnpm build");
    expect(html).toContain("Resume");
    expect(html).not.toContain("terminal-well");
  });
});
