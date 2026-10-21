// How it works: NOTE-14's eight beats across the six lanes, from the canvas's `LANES`, `BEATS` and `renderVals`
// (Main.dc.html). Plain data and functions; HowItWorks.astro draws the first beat and its script the rest.

export const BEAT_MS = 3200;

export interface Lane {
  title: string;
  icon: string;
  color: string;
}

export const LANES: readonly Lane[] = [
  { title: "Backlog", icon: "circle-dashed", color: "var(--st-queued)" },
  { title: "Ready for Dev", icon: "hourglass", color: "var(--st-queued)" },
  { title: "Dev", icon: "activity", color: "var(--st-working)" },
  { title: "Ready for Review", icon: "eye", color: "var(--st-review)" },
  { title: "Approved", icon: "check", color: "var(--st-approved)" },
  { title: "Done", icon: "git-merge", color: "var(--st-queued)" },
];

interface Beat {
  lane: number;
  label: string;
  title: string;
  text: string;
  /** Pill class, icon, label, and "spin" when the icon turns. */
  pill: [string, string, string, string?];
  /** "m-inv" for the investigate colour, icon, text. */
  meta: [string, string, string];
  /** Avatar class and initials on the card. */
  av: [string, string];
  attn?: boolean;
  /** The bubble: avatar class ("av-u" is you), initials, who, what. */
  bub?: [string, string, string, string];
}

const BEATS: readonly Beat[] = [
  {
    lane: 0,
    label: "Planned",
    title: "The Project agent writes it",
    text: "You ran your /import-github skill on the issues labelled ready. It read the code, proposed two tickets, and created them when you said go.",
    pill: ["p-i", "bot", "Created by Project agent"],
    meta: ["m-inv", "bot", "from GitHub issue #31"],
    av: ["av-pa", "PA"],
    bub: ["av-pa", "PA", "Project agent", "created NOTE-14 and NOTE-15"],
  },
  {
    lane: 1,
    label: "Queued",
    title: "It waits its turn",
    text: "Tickets start in priority order, once what they depend on is merged and an agent is free.",
    pill: ["p-q", "hourglass", "Ready for Dev"],
    meta: ["", "hourglass", "starts when an agent is free"],
    av: ["av-cc", "CC"],
  },
  {
    lane: 2,
    label: "Picked up",
    title: "An agent picks it up",
    text: "Otaq makes a git worktree for it, runs your setup command, and starts Mira in Claude Code.",
    pill: ["p-w", "loader", "Working", "spin"],
    meta: ["", "git-branch", "otaq/NOTE-14"],
    av: ["av-cc", "CC"],
    bub: ["av-cc", "CC", "Mira", "is reading the ticket and the code"],
  },
  {
    lane: 2,
    label: "Asks you",
    title: "It asks you when it’s stuck",
    text: "A real question turns the card turquoise and puts it in your inbox. You answer with one click, and Mira carries on.",
    pill: ["p-a", "message-q", "Asks you"],
    meta: ["", "message-q", "Say how to add the first note?"],
    av: ["av-cc", "CC"],
    attn: true,
    bub: ["av-u", "You", "You", "answered: yes, show notes add"],
  },
  {
    lane: 2,
    label: "Checked",
    title: "Another agent checks it",
    text: "When Mira submits, Vera, a read-only validator on another company’s model, checks the change against the ticket. Misses go straight back to Mira.",
    pill: ["p-w", "shield-check", "Validating · round 1"],
    meta: ["", "shield-check", "checking 2 acceptance criteria"],
    av: ["av-cp", "CP"],
    bub: ["av-cp", "CP", "Vera", "passed it: 2 of 2 criteria met"],
  },
  {
    lane: 3,
    label: "Review",
    title: "You review the diff",
    text: "You get the diff, Mira’s summary, every product choice she made and Vera’s report. Comment on lines, or send it back.",
    pill: ["p-r", "shield-check", "Validated · round 1"],
    meta: ["m-inv", "lightbulb", "+24 −3 · 1 decision"],
    av: ["av-cc", "CC"],
  },
  {
    lane: 4,
    label: "Approved",
    title: "You approve it",
    text: "Squash-merge it locally, or open a GitHub or Azure DevOps pull request. Your choice, every time.",
    pill: ["p-ok", "loader", "Merging…", "spin"],
    meta: ["", "git-merge", "into main"],
    av: ["av-cc", "CC"],
  },
  {
    lane: 5,
    label: "Merged",
    title: "Merged. The next one starts.",
    text: "NOTE-15 was waiting for NOTE-14. Now it starts on its own, building on finished work.",
    pill: ["p-d", "check", "Merged"],
    meta: ["", "git-merge", "squash 4f1c9e2"],
    av: ["av-cc", "CC"],
  },
];

export const BEAT_COUNT = BEATS.length;

/** The step buttons' labels, in order. */
export const STEP_LABELS: readonly string[] = BEATS.map((b) => b.label);

/** The player: which beat shows (0-based) and whether it moves on by itself. */
export interface Player {
  beat: number;
  playing: boolean;
}

/** The timer's tick: the next beat while playing, wrapping from the last to the first. */
export function tick(p: Player): Player {
  return p.playing ? { ...p, beat: (p.beat + 1) % BEAT_COUNT } : p;
}

/** A click on a step: jump there (the caller restarts the timer). */
export function jump(p: Player, beat: number): Player {
  return { ...p, beat: Math.min(Math.max(0, Math.trunc(beat)), BEAT_COUNT - 1) };
}

/** Pause / Play. */
export function toggle(p: Player): Player {
  return { ...p, playing: !p.playing };
}

/** A lane's left edge on the stage, as the canvas's `laneLeft`. */
export function laneLeft(i: number): string {
  return `calc(10px + (100% - 20px - 50px) / 6 * ${String(i)} + 10px * ${String(i)})`;
}

export interface Bubble {
  left: string;
  avClass: string;
  initials: string;
  who: string;
  text: string;
}

/** Everything the stage and the text under it show for one beat (the canvas's `fx`, lanes and steps). */
export interface BeatView {
  /** "1 of 8" … */
  n: string;
  title: string;
  text: string;
  /** Which of the six lanes are lit. */
  lanesOn: boolean[];
  card: {
    left: string;
    attn: boolean;
    avClass: string;
    av: string;
    pillClass: string;
    pillIcon: string;
    pillLabel: string;
    spin: boolean;
    metaInv: boolean;
    metaIcon: string;
    meta: string;
  };
  ghost: { left: string; pillClass: string; icon: string; label: string };
  bubble: Bubble | null;
  steps: { cls: "on" | "past" | ""; current: boolean }[];
  /** The progress bar's animation, alternating so it restarts on every beat. */
  bar: "a" | "b";
}

export function beatView(beat: number): BeatView {
  const b = BEATS[beat] ?? BEATS[0];
  if (b === undefined) throw new Error("no beats");
  const lanesOn = LANES.map((_, i) => i === b.lane || (beat === 7 && i === 2));
  const last = beat === 7;
  return {
    n: `${String(beat + 1)} of ${String(BEAT_COUNT)}`,
    title: b.title,
    text: b.text,
    lanesOn,
    card: {
      left: laneLeft(b.lane),
      attn: b.attn === true,
      avClass: b.av[0],
      av: b.av[1],
      pillClass: b.pill[0],
      pillIcon: b.pill[1],
      pillLabel: b.pill[2],
      spin: b.pill[3] === "spin",
      metaInv: b.meta[0] === "m-inv",
      metaIcon: b.meta[1],
      meta: b.meta[2],
    },
    ghost: {
      left: last ? laneLeft(2) : beat === 0 ? laneLeft(0) : laneLeft(1),
      pillClass: last ? "p-w" : "p-q",
      icon: last ? "activity" : "hourglass",
      label: last ? "Working" : beat === 0 ? "Backlog" : "waits for NOTE-14",
    },
    bubble: b.bub
      ? {
          left: laneLeft(Math.min(b.lane, 4)),
          avClass: b.bub[0] === "av-u" ? "p-q" : b.bub[0],
          initials: b.bub[1] === "You" ? "U" : b.bub[1],
          who: b.bub[2],
          text: b.bub[3],
        }
      : null,
    steps: BEATS.map((_, i) => ({ cls: i === beat ? "on" : i < beat ? "past" : "", current: i === beat })),
    bar: beat % 2 ? "b" : "a",
  };
}
