// The Flow step's views as Rust words them for the example's shop-web (`s14-12`, `s14-14`), for the tests: the
// go-ahead with Claude Code and OpenCode connected, and the agents at work as each tool is asked, stops or ends.
import type { FlowAtWorkRow } from "../../generated/FlowAtWorkRow";
import type { FlowAtWorkTab } from "../../generated/FlowAtWorkTab";
import type { FlowAtWorkView } from "../../generated/FlowAtWorkView";
import type { OnboardingFlowStep } from "../../generated/OnboardingFlowStep";
import type { ToolAvatarView } from "../../generated/ToolAvatarView";

export type FixtureTool = "claude" | "opencode";

const AVATARS: Record<FixtureTool, ToolAvatarView> = {
  claude: {
    tool: "claude_code",
    letters: "CC",
    background: "tool-cc",
    foreground: "tool-cc-fg",
    hover: "Claude Code",
  },
  opencode: {
    tool: "open_code",
    letters: "OC",
    background: "tool-oc",
    foreground: "tool-oc-fg",
    hover: "OpenCode",
  },
};

const NAMES: Record<FixtureTool, string> = { claude: "Claude Code", opencode: "OpenCode" };

/** A tool's row, not asked yet. */
export function notAsked(tool: FixtureTool): FlowAtWorkRow {
  return {
    tool,
    avatar: AVATARS[tool],
    name: NAMES[tool],
    state: "not_asked",
    status: "Not asked",
    error: false,
    agent: null,
    proposal: null,
    buttons: [
      {
        label: `Ask ${NAMES[tool]}`,
        icon: "sparkles",
        enabled: true,
        quiet: false,
        action: { kind: "ask", tool },
      },
    ],
  };
}

/** A tool's row while its flow agent `agent` works. */
export function working(tool: FixtureTool, agent: string, minutes: number): FlowAtWorkRow {
  return {
    ...notAsked(tool),
    state: "working",
    status: `Working · ${String(minutes)} min`,
    agent,
    buttons: [
      { label: "Stop", icon: "circle-stop", enabled: true, quiet: true, action: { kind: "stop", agent } },
    ],
  };
}

/** A tool's row after its flow agent ended without a proposal. */
export function ended(tool: FixtureTool, agent: string): FlowAtWorkRow {
  return {
    ...notAsked(tool),
    state: "ended",
    status: "Ended without a proposal",
    error: true,
    agent,
    buttons: [
      {
        label: "Try again",
        icon: "rotate-ccw",
        enabled: true,
        quiet: false,
        action: { kind: "resume", agent },
      },
    ],
  };
}

/** A tool's row at its plan limit, Try again off until 3:45 pm. */
export function stopped(tool: FixtureTool, agent: string): FlowAtWorkRow {
  return {
    ...notAsked(tool),
    state: "plan_limit",
    status: "Plan limit · resets 3:45 pm · picks up where it stopped",
    error: true,
    agent,
    buttons: [
      {
        label: "Try again at 3:45 pm",
        icon: "clock",
        enabled: false,
        quiet: false,
        action: { kind: "resume_tool", tool },
      },
    ],
  };
}

/** A tool found but not signed in. */
export function notSignedIn(tool: FixtureTool): FlowAtWorkRow {
  return {
    ...notAsked(tool),
    state: "not_signed_in",
    status: "Couldn’t start · not signed in",
    error: true,
    buttons: [
      { label: "Sign in…", icon: "log-in", enabled: true, quiet: false, action: { kind: "sign_in", tool } },
    ],
  };
}

function tab(row: FlowAtWorkRow): FlowAtWorkTab | null {
  if (row.agent === null) return null;
  const parts: Record<string, Pick<FlowAtWorkTab, "status" | "dot" | "pill" | "foot">> = {
    working: { status: row.status, dot: "working", pill: null, foot: null },
    plan_limit: {
      status: "Stopped",
      dot: "error",
      pill: { palette: "queued", icon: "circle-pause", label: "Stopped" },
      foot: null,
    },
    ended: { status: "Ended", dot: "queued", pill: null, foot: "Read-only · the session ended" },
    proposed: { status: "Proposed", dot: "approved", pill: null, foot: null },
  };
  const part = parts[row.state];
  if (part === undefined) return null;
  return { agent: row.agent, tool: row.tool, avatar: row.avatar, name: row.name, ...part };
}

/** The agents at work with these rows, as `flow_at_work.rs` builds them. */
export function atWork(rows: FlowAtWorkRow[]): FlowAtWorkView {
  return {
    title: "Agents at work",
    text: "Each looks at how you use it on this Mac and at this repo. Watch any of them in its terminal, or come back later: the proposals wait for you.",
    rows,
    tabs: rows.map(tab).filter((t) => t !== null),
    watch: "Watch",
    watching: "Watching",
    stop_all: "Stop all and use a preset",
    running: rows
      .filter((r) => r.state === "working" || r.state === "plan_limit")
      .flatMap((r) => r.agent ?? []),
    skip: "Skip, use the default",
    working: rows.filter((r) => r.state === "working").length,
  };
}

const CARD = (name: string, file: string, columns: string[]) => ({
  name,
  delivers: "Done · click",
  line: "Today’s board",
  columns: columns.map((label) => ({ icon: "bot" as const, label })),
  file,
});

const INTRO =
  "An agent proposes the board’s columns: who works on a ticket, what checks it, how it’s delivered. Nothing changes until you say so.";

/** `onboarding.flow` for shop-web with Claude Code and OpenCode connected, nothing asked (`s14-12`). */
export const STEP: OnboardingFlowStep = {
  project: "SHOP",
  title: "Build shop-web’s flow",
  intro: INTRO,
  line: "Each agent looks at how you use it on this Mac and at this repo, and proposes a flow for shop-web.",
  heading: "ASK AN AGENT",
  rows: (["claude", "opencode"] as const).map((tool) => ({
    tool,
    avatar: AVATARS[tool],
    name: NAMES[tool],
    detail: "connected",
    ask: `Ask ${NAMES[tool]}`,
    primary: false,
  })),
  provider: "What an agent reads goes to that tool’s model provider, and it uses tokens.",
  skip: "Skip, use the default",
  use_preset: "Use a preset instead",
  ask_an_agent: "Ask an agent",
  back_to_tools: null,
  opens: "go_ahead",
  presets: {
    text: INTRO,
    note: null,
    heading: "START FROM A PRESET",
    cards: [
      CARD("Otaq default", "otaq-default.yaml", ["Ready for Dev", "Dev", "Validate", "Review", "Done"]),
      CARD("Grill first", "grill-first.yaml", [
        "Ready for Dev",
        "Grill",
        "Dev",
        "Validate",
        "Review",
        "Done",
      ]),
    ],
  },
  at_work: atWork([notAsked("claude"), notAsked("opencode")]),
};

/** No agent tool connected (`s14-12 → No agent tool connected`). */
export const NO_TOOL: OnboardingFlowStep = {
  ...STEP,
  rows: [],
  ask_an_agent: null,
  back_to_tools: "Back to Tools",
  opens: "no_tool",
  presets: {
    ...STEP.presets,
    text: "Start from a preset. You can change it later by asking the Project agent.",
    note: "Connect an agent tool in the Tools step and an agent can recommend a flow for shop-web.",
  },
  at_work: atWork([]),
};

/** The step once its agents at work are `rows`. */
export function stepAtWork(rows: FlowAtWorkRow[]): OnboardingFlowStep {
  return { ...STEP, opens: "at_work", at_work: atWork(rows) };
}

/** A request call that records what it was asked and answers from `answers`. */
export function recorder(answers: Record<string, unknown> = {}) {
  const asked: [string, unknown][] = [];
  const ask = (method: string, params?: unknown) => {
    asked.push([method, params]);
    return Promise.resolve(answers[method] ?? null);
  };
  return { asked, ask };
}
