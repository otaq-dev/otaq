// What the demo shows, worked out from its state (the canvas's `pres` and `renderVals`): plain functions, so React only
// draws their result. `Moment` is what React keeps on its own: the card under the mouse and the one being dragged.

import {
  AGENT_NAMES,
  AGENTS,
  type AgentName,
  type Column,
  type DiffFile,
  genericDiff,
  type HistoryEntry,
  type Line,
  NOTE4_DIFF,
  type Option,
  project,
  type Project,
  type ProjectAgent,
  PROJECTS,
  type ProjectKey,
  type Ticket,
  type ToolId,
} from "./data";
import { type DemoState, type PanelTab, type Role, type SettingsPage, unmetIn } from "./state";

export interface Moment {
  hoverKey: string | null;
  dragKey: string | null;
  dropCol: Column | null;
}

export const NO_MOMENT: Moment = { hoverKey: null, dragKey: null, dropCol: null };

export interface Pill {
  cls: string;
  icon: string;
  label: string;
  spin: boolean;
}

export interface Meta {
  i: string;
  t: string;
  cls: string;
}

const P = (cls: string, icon: string, label: string, spin = false): Pill => ({ cls, icon, label, spin });

/** A ticket's pill, metas and card class (`pres` in the canvas). */
export function present(
  tickets: readonly Ticket[],
  t: Ticket,
): { pill: Pill | null; metas: Meta[]; cls: string } {
  let pill: Pill | null = null;
  let cls = "";
  const metas: Meta[] = [];
  const un = unmetIn(tickets, t);
  switch (t.st) {
    case "queued":
      if (t.col === "backlog") metas.push({ i: "circle-play", t: "Ready to start", cls: "" });
      else if (un.length)
        metas.push({ i: "hourglass", t: "waits for " + un.join(", ") + " to merge", cls: "" });
      else metas.push({ i: "hourglass", t: "starts when an agent is free", cls: "" });
      if (t.col === "backlog" && un.length)
        metas.push({ i: "hourglass", t: "waits for " + un.join(", "), cls: "" });
      break;
    case "starting":
      pill = P("pl-w", "loader", t.stage === "setup" ? "Running setup" : "Creating worktree", true);
      metas.push({ i: "git-branch", t: "otaq/" + t.k, cls: "m" });
      break;
    case "working":
      pill = P("pl-w", "activity", "Working");
      metas.push({
        i: "activity",
        t: (t.age ?? "active just now") + (t.tokens ? " · " + t.tokens : ""),
        cls: "",
      });
      break;
    case "attn": {
      const perm = t.ask?.kind === "perm";
      pill = P("pl-a", perm ? "hand" : "message-q", perm ? "Needs you" : "Asks you");
      metas.push({ i: "chevron-right", t: t.ask?.short ?? "", cls: "aq" });
      cls = "attn";
      break;
    }
    case "error":
      pill = P("pl-e", "circle-alert", "Interrupted");
      metas.push({ i: "refresh", t: "Resume from the panel", cls: "" });
      cls = "err";
      break;
    case "validating":
      pill = P("pl-w", "shield-check", "Validating · round " + String(t.round));
      metas.push({ i: "activity", t: t.tokens, cls: "m" });
      break;
    case "review":
      pill = P("pl-r", "shield-check", "Validated · round " + String(t.round));
      if (t.decisions)
        metas.push({
          i: "lightbulb",
          t: String(t.decisions) + (t.decisions > 1 ? " decisions" : " decision"),
          cls: "inv",
        });
      if (t.adds) metas.push({ i: "file-diff", t: "+" + String(t.adds) + " −" + String(t.dels), cls: "m" });
      break;
    case "merging":
      pill = P("pl-ok", "loader", "Merging…", true);
      break;
    case "done":
      pill = P("pl-d", "git-merge", "Merged");
      break;
  }
  if (t.by) metas.push({ i: "bot", t: "created by Project agent “" + t.by + "”", cls: "inv" });
  return { pill, metas, cls };
}

export interface CardView {
  k: string;
  title: string;
  tool: ToolId;
  agentLabel: string;
  prio: number;
  showPrio: boolean;
  prioIcon: string;
  epicPos: string | null;
  pill: Pill | null;
  metas: Meta[];
  conflict: boolean;
  isReview: boolean;
  cls: string;
  movable: boolean;
}

export interface SectionView {
  title: string | null;
  icon: string;
  color: string;
  cards: CardView[];
  emptyText: string;
}

export interface ColumnView {
  id: Column;
  title: string;
  icon: string;
  color: string;
  sub: string;
  count: number;
  cls: string;
  coach: boolean;
  sections: SectionView[];
}

export interface EpicView {
  name: string;
  lead: string;
  progress: string;
  stops: { k: string; label: string; icon: string; color: string }[];
}

export interface InboxItem {
  pk: ProjectKey;
  k: string;
  title: string;
  reason: string;
  age: string;
  /** What a click opens: a ticket (on a tab), or a Project agent. */
  go: { ticket: string; tab: PanelTab } | { agent: string };
}

export interface InboxGroup {
  title: string;
  color: string;
  items: InboxItem[];
}

export interface AgentView {
  id: string;
  name: string;
  tokens: string;
  pillCls: string;
  pillLabel: string;
  lines: readonly Line[];
  isNew: boolean;
  created: readonly string[];
  askQ: string | null;
  opts: readonly Option[];
}

export interface PanelView {
  k: string;
  title: string;
  prio: number;
  prioIcon: string;
  prioWord: string;
  pill: Pill;
  epicPos: string | null;
  tokens: string;
  tool: ToolId;
  agentLabel: string;
  canApprove: boolean;
  canStart: boolean;
  isAsking: boolean;
  isRunning: boolean;
  isDone: boolean;
  round: number;
  noSession: boolean;
  lines: readonly Line[];
  ask: { title: string; icon: string; opts: readonly Option[] } | null;
  adds: number;
  dels: number;
  files: readonly DiffFile[];
  decision: string | null;
  desc: string;
  acc: readonly string[];
  depsText: string;
  hist: readonly HistoryEntry[];
}

export interface BoardView {
  proj: Project;
  runningLabel: string;
  running: number;
  needs: number;
  projects: { key: ProjectKey; name: string; on: boolean; badge: number }[];
  paneAttn: boolean;
  agentTabs: { id: string; name: string; on: boolean; attn: boolean; working: boolean }[];
  agent: AgentView;
  sugg: readonly [string, string];
  epic: EpicView | null;
  columns: ColumnView[];
  doneCount: number;
  panel: PanelView | null;
  inbox: InboxGroup[];
}

const COL_WORD: Record<Column, string> = {
  backlog: "Backlog",
  ready: "Ready for Dev",
  dev: "Dev",
  review: "Ready for Review",
  done: "Done",
};

const PRIO_WORD = ["", "Urgent", "High", "Normal", "Low"];

const REASON_AGE: Record<string, string> = {
  "NOTE-8": "2m",
  "SHOP-22": "4m",
  "SHOP-25": "18m",
  "NOTE-4": "12m",
  "NOTE-5": "31m",
  "SHOP-18": "1h",
};

const STOP_STYLE: Record<Column, readonly [string, string]> = {
  done: ["circle-check", "var(--done)"],
  dev: ["activity", "var(--work)"],
  ready: ["hourglass", "var(--q)"],
  backlog: ["circle-dashed", "var(--q)"],
  review: ["eye", "var(--rev)"],
};

const AGENT_PILL: Record<ProjectAgent["st"], readonly [string, string]> = {
  attn: ["pl-a", "Asks you"],
  working: ["pl-w", "Working"],
  idle: ["pl-q", "Idle"],
  new: ["pl-q", "New"],
};

const order = (a: Ticket, b: Ticket): number => {
  const w = (t: Ticket) => (t.st === "attn" || t.st === "error" ? 0 : 1);
  return w(a) - w(b) || a.prio - b.prio;
};

const prioIcon = (prio: number) => (prio <= 2 ? "chevron-up" : "chevron-down");

/** The diff a ticket shows: none until it has changes. */
export function diffOf(t: Ticket): readonly DiffFile[] {
  if (!t.adds) return [];
  return t.diffKey ? NOTE4_DIFF : genericDiff(t.title);
}

/** The whole screen from the state (`renderVals` in the canvas). */
export function boardView(s: DemoState, m: Moment = NO_MOMENT): BoardView {
  const proj = project(s.project);
  const all = s.tickets;
  const mine = all.filter((t) => t.p === s.project);
  const q = s.search.trim().toLowerCase();
  const visible = q ? mine.filter((t) => (t.k + " " + t.title).toLowerCase().includes(q)) : mine;

  // the dependency highlight while a card is hovered
  const rel = new Set<string>();
  const h = m.hoverKey ? all.find((t) => t.k === m.hoverKey) : undefined;
  if (h) {
    rel.add(h.k);
    for (const w of h.waits) rel.add(w);
    for (const t of mine) if (t.waits.includes(h.k)) rel.add(t.k);
  }
  const hasRel = rel.size > 1;

  const card = (t: Ticket): CardView => {
    const pr = present(all, t);
    const cls = [
      pr.cls,
      t.fresh ? "fresh" : "",
      s.openKey === t.k ? "sel" : "",
      t.col === "done" ? "donec" : "",
    ];
    cls.push(m.dragKey === t.k ? "drag" : "");
    if (hasRel) cls.push(rel.has(t.k) ? "rel" : "dim");
    const ag = AGENTS[t.agent];
    return {
      k: t.k,
      title: t.title,
      tool: ag.tool,
      agentLabel: t.agent + " · " + ag.toolName,
      prio: t.prio,
      showPrio: t.prio !== 3 && t.col !== "done",
      prioIcon: prioIcon(t.prio),
      epicPos: t.epic ? t.epic + " · " + (t.epicPos ?? "") : null,
      pill: pr.pill,
      metas: pr.metas,
      conflict: !!t.conflict && t.st === "review",
      isReview: t.st === "review",
      cls: cls.filter(Boolean).join(" "),
      movable: t.col === "backlog" || t.col === "ready",
    };
  };
  const inCol = (c: Column) =>
    visible
      .filter((t) => t.col === c)
      .sort(order)
      .map(card);
  const reviewCards = visible
    .filter((t) => t.col === "review" && t.st !== "merging")
    .sort(order)
    .map(card);
  const approvedCards = visible.filter((t) => t.col === "review" && t.st === "merging").map(card);
  const note6 = all.find((t) => t.k === "NOTE-6");

  type ColDef = Omit<ColumnView, "count" | "cls" | "coach" | "sections"> & {
    sections: (Omit<SectionView, "title" | "icon" | "color"> & Partial<SectionView>)[];
  };
  const colDef: ColDef[] = [
    {
      id: "backlog",
      title: "Backlog",
      icon: "circle-dashed",
      color: "var(--q)",
      sub: "Tickets the Project agent planned, and yours",
      sections: [{ cards: inCol("backlog"), emptyText: "No tickets" }],
    },
    {
      id: "ready",
      title: "Ready for Dev",
      icon: "hourglass",
      color: "var(--q)",
      sub: "Starts when ready, in priority order",
      sections: [{ cards: inCol("ready"), emptyText: "Drop a ticket here to start it when it’s ready" }],
    },
    {
      id: "dev",
      title: "Dev",
      icon: "activity",
      color: "var(--work)",
      sub: "One agent per ticket, each in its own worktree",
      sections: [{ cards: inCol("dev"), emptyText: "Drop a ticket here to start its agent now" }],
    },
    {
      id: "review",
      title: "Review",
      icon: "eye",
      color: "var(--rev)",
      sub: "Passed validation, waiting for you",
      sections: [
        {
          title: "Ready for Review",
          icon: "eye",
          color: "var(--rev)",
          cards: reviewCards,
          emptyText: "Nothing to review",
        },
        {
          title: "Approved",
          icon: "check",
          color: "var(--ok)",
          cards: approvedCards,
          emptyText: "Approve a ticket from its panel: Local merge or Open PR",
        },
      ],
    },
  ];
  if (s.doneOpen)
    colDef.push({
      id: "done",
      title: "Done",
      icon: "circle-check",
      color: "var(--done)",
      sub: "Merged",
      sections: [{ cards: inCol("done"), emptyText: "Nothing merged yet" }],
    });
  const columns: ColumnView[] = colDef.map((c) => {
    const droppable = c.id === "backlog" || c.id === "ready" || c.id === "dev";
    return {
      ...c,
      count: c.sections.reduce((n, x) => n + x.cards.length, 0),
      cls: m.dragKey ? (m.dropCol === c.id ? "drop" : droppable ? "" : "nodrop") : "",
      coach: c.id === "backlog" && s.coach && s.project === "NOTE" && note6?.col === "backlog",
      sections: c.sections.map((x) => ({
        title: x.title ?? null,
        icon: x.icon ?? "circle",
        color: x.color ?? "",
        cards: x.cards,
        emptyText: x.emptyText,
      })),
    };
  });

  // the epic strip
  const kids = mine.filter((t) => t.epic === "Notebooks").sort((a, b) => (a.k < b.k ? -1 : 1));
  const epic: EpicView | null =
    s.project === "NOTE" && kids.some((k) => k.col !== "done")
      ? {
          name: "Notebooks",
          lead: "Notebooks lead · idle",
          progress: String(kids.filter((t) => t.col === "done").length) + " of " + String(kids.length),
          stops: kids.map((t) => {
            const st = t.st === "attn" ? (["message-q", "var(--attn)"] as const) : STOP_STYLE[t.col];
            return { k: t.k, label: t.k.replace("NOTE-", ""), icon: st[0], color: st[1] };
          }),
        }
      : null;

  // the inbox
  const age = (k: string) => REASON_AGE[k] ?? "now";
  const g1: InboxItem[] = all
    .filter((t) => t.st === "attn")
    .map((t) => ({
      pk: t.p,
      k: t.k,
      title: t.title,
      reason: t.ask?.short ?? "",
      age: age(t.k),
      go: { ticket: t.k, tab: "terminal" },
    }));
  for (const a of s.agents)
    if (a.st === "attn")
      g1.push({
        pk: a.p,
        k: "Agent",
        title: "“" + a.name + "”",
        reason: "Project agent asks: " + (a.ask?.q ?? ""),
        age: "1m",
        go: { agent: a.id },
      });
  const g2: InboxItem[] = all
    .filter((t) => t.st === "error")
    .map((t) => ({
      pk: t.p,
      k: t.k,
      title: t.title,
      reason: "Session interrupted · Resume continues the same conversation",
      age: age(t.k),
      go: { ticket: t.k, tab: "terminal" },
    }));
  const g3: InboxItem[] = all
    .filter((t) => t.st === "review")
    .map((t) => ({
      pk: t.p,
      k: t.k,
      title: t.title,
      reason:
        "Ready for review · +" +
        String(t.adds) +
        " −" +
        String(t.dels) +
        (t.decisions ? " · " + String(t.decisions) + " decision" + (t.decisions > 1 ? "s" : "") : ""),
      age: age(t.k),
      go: { ticket: t.k, tab: "diff" },
    }));
  const inbox: InboxGroup[] = [];
  if (g1.length) inbox.push({ title: "Needs you", color: "var(--attn)", items: g1 });
  if (g2.length) inbox.push({ title: "Interrupted", color: "var(--err)", items: g2 });
  if (g3.length) inbox.push({ title: "Ready for review", color: "var(--rev)", items: g3 });
  const needs = g1.length + g2.length;
  const running = all.filter(
    (t) => t.st === "working" || t.st === "validating" || t.st === "starting",
  ).length;

  // the Project agent pane
  const pAgents = s.agents.filter((a) => a.p === s.project);
  const ag = pAgents.find((a) => a.id === s.agentSel[s.project]) ?? pAgents[0];
  if (!ag) throw new Error("no Project agent");
  const agPill = AGENT_PILL[ag.st];
  const agent: AgentView = {
    id: ag.id,
    name: ag.name,
    tokens: ag.tokens,
    pillCls: agPill[0],
    pillLabel: agPill[1],
    lines: ag.lines,
    isNew: ag.st === "new" && ag.lines.length === 0,
    created: ag.created,
    askQ: ag.ask && ag.st === "attn" ? ag.ask.q : null,
    opts: ag.ask?.opts ?? [],
  };
  const sugg =
    s.project === "NOTE"
      ? ([
          "Why is notes ls slow with 10,000 notes? Make tickets for what you find.",
          "Plan a “tags” feature as an epic.",
        ] as const)
      : (["Find pages that load images eagerly and make tickets.", "/import-github label:ready"] as const);

  return {
    proj,
    running,
    runningLabel: String(running) + " running",
    needs,
    projects: PROJECTS.map((p) => ({
      key: p.key,
      name: p.name,
      on: p.key === s.project,
      badge:
        all.filter((t) => t.p === p.key && (t.st === "attn" || t.st === "error")).length +
        s.agents.filter((a) => a.p === p.key && a.st === "attn").length,
    })),
    paneAttn: pAgents.some((a) => a.st === "attn"),
    agentTabs: pAgents.map((a) => ({
      id: a.id,
      name: a.name,
      on: a.id === ag.id,
      attn: a.st === "attn",
      working: a.st === "working",
    })),
    agent,
    sugg,
    epic,
    columns,
    doneCount: mine.filter((x) => x.col === "done").length,
    panel: panelView(s),
    inbox,
  };
}

function panelView(s: DemoState): PanelView | null {
  const ot = s.openKey ? s.tickets.find((t) => t.k === s.openKey) : undefined;
  if (!ot) return null;
  const pr = present(s.tickets, ot);
  const files = diffOf(ot);
  const pill =
    pr.pill ??
    (ot.col === "backlog" ? P("pl-q", "circle-dashed", "Backlog") : P("pl-q", "hourglass", "Ready for Dev"));
  const un = unmetIn(s.tickets, ot);
  const ag = AGENTS[ot.agent];
  const asking = ot.st === "attn" || ot.st === "error";
  return {
    k: ot.k,
    title: ot.title,
    prio: ot.prio,
    prioIcon: prioIcon(ot.prio),
    prioWord: PRIO_WORD[ot.prio] ?? "",
    pill,
    epicPos: ot.epic ? ot.epic + " · " + (ot.epicPos ?? "") : null,
    tokens: ot.tokens,
    tool: ag.tool,
    agentLabel: ot.agent + " · " + ag.toolName + " · " + ag.model,
    canApprove: ot.st === "review",
    canStart: ot.col === "backlog" || ot.col === "ready",
    isAsking: asking,
    isRunning: ot.st === "working" || ot.st === "validating" || ot.st === "starting",
    isDone: ot.st === "done",
    round: ot.round,
    noSession: ot.log.length === 0,
    lines: ot.log,
    ask:
      ot.ask && asking
        ? {
            title: ot.ask.title,
            icon: ot.ask.kind === "perm" ? "hand" : ot.ask.kind === "err" ? "circle-alert" : "message-q",
            opts: ot.ask.opts,
          }
        : null,
    adds: ot.adds ?? 0,
    dels: ot.dels ?? 0,
    files,
    decision: ot.decision ?? null,
    desc: ot.desc ?? "Imported ticket.",
    acc: ot.acc,
    depsText: ot.waits.length
      ? ot.waits.join(", ") + (un.length ? " · waiting for " + un.join(", ") + " to merge" : " · all merged")
      : "Nothing. It can start any time.",
    hist: ot.hist.length
      ? ot.hist
      : [
          { at: "yesterday", t: "Created by you" },
          { at: "today", t: "Moved to " + COL_WORD[ot.col] },
        ],
  };
}

// --- static parts of the panel and Settings ------------------------------------------------------------------------

export const PANEL_TABS: readonly (readonly [PanelTab, string, string])[] = [
  ["terminal", "Terminal", "square-terminal"],
  ["diff", "Diff", "file-diff"],
  ["run", "Run", "play"],
  ["shell", "Shell", "terminal"],
  ["ticket", "Ticket", "file-text"],
  ["history", "History", "history"],
];

/** The Run and Shell tabs: a note that they work in the app. */
export function otherTab(s: DemoState): { icon: string; title: string; text: string } {
  if (s.ptab === "run")
    return {
      icon: "play",
      title: "Run",
      text:
        "Starts “" +
        s.projFields[s.project].run +
        "” for this ticket on its own $PORT, so several dev servers run side by side. Try it in the app.",
    };
  if (s.ptab === "shell")
    return {
      icon: "terminal",
      title: "Shell",
      text: "A plain terminal in this ticket’s worktree. Try it in the app.",
    };
  return { icon: "info", title: "", text: "" };
}

export const SETTINGS_GLOBAL: readonly (readonly [SettingsPage, string, string])[] = [
  ["tools", "Agent tools", "plug"],
  ["agents", "Agents", "bot"],
  ["notifications", "Notifications", "bell"],
  ["appearance", "Appearance", "palette"],
  ["keyboard", "Keyboard shortcuts", "keyboard"],
  ["data", "Data", "database"],
  ["about", "About", "info"],
];

export function settingsTitle(s: DemoState): string {
  if (s.spage === "proj-NOTE" || s.spage === "proj-SHOP")
    return project(s.spage === "proj-NOTE" ? "NOTE" : "SHOP").name;
  return SETTINGS_GLOBAL.find((x) => x[0] === s.spage)?.[1] ?? "";
}

/** The project a project page of Settings shows (the current project on the other pages). */
export function settingsProject(s: DemoState): ProjectKey {
  if (s.spage === "proj-NOTE") return "NOTE";
  if (s.spage === "proj-SHOP") return "SHOP";
  return s.project;
}

export interface ToolRow {
  id: ToolId;
  name: string;
  detail: string;
  status: string;
  pillCls: string;
  pillIcon: string;
  action: string;
  tip: string;
}

export function toolRows(s: DemoState): ToolRow[] {
  const ok = s.openCodeInstalled;
  return [
    {
      id: "CC",
      name: "Claude Code",
      detail: "Signed in · hooks installed · 2 project folders trusted",
      status: "Connected",
      pillCls: "pl-ok",
      pillIcon: "check",
      action: "Details",
      tip: "Claude Code uses its own sign-in and permission mode. Otaq never changes them.",
    },
    {
      id: "CX",
      name: "Codex CLI",
      detail: "Signed in with ChatGPT · otaq.rules in ~/.codex",
      status: "Connected",
      pillCls: "pl-ok",
      pillIcon: "check",
      action: "Details",
      tip: "Otaq pre-approves only its own otaq commands, and removes otaq.rules when you disconnect.",
    },
    {
      id: "CP",
      name: "GitHub Copilot CLI",
      detail: "Signed in · month: 80% left · resets Nov 1",
      status: "Connected",
      pillCls: "pl-ok",
      pillIcon: "check",
      action: "Details",
      tip: "Copilot can run models from several companies, so your validator can come from a different one.",
    },
    {
      id: "OC",
      name: "OpenCode",
      detail: ok ? "Found · signed in" : "Not found on this computer. Install it, then press Check again.",
      status: ok ? "Connected" : "Not installed",
      pillCls: ok ? "pl-ok" : "pl-q",
      pillIcon: ok ? "check" : "circle-dashed",
      action: ok ? "Details" : "How to install",
      tip: "In the app this opens OpenCode’s install guide.",
    },
  ];
}

export const THEMES = [
  { id: "dark", name: "Otaq Dark", bg: "#0e0f11", col: "#121316", card: "#23262c", ac: "#6e78f7" },
  { id: "light", name: "Otaq Light", bg: "#f6f6f4", col: "#eeeeeb", card: "#ffffff", ac: "#4c57d6" },
] as const;

export const ROLES: readonly (readonly [Role, string, string])[] = [
  ["dev", "Writes code", "For tickets that don’t pick an agent"],
  ["val", "Validates", "Checks every submitted change"],
  ["lead", "Leads epics", "Plans an epic’s children with you"],
  ["proj", "Project agent", "Runs in the pane left of the board"],
];

export const KEYS: readonly { t: string; k: string }[] = [
  { t: "Search", k: "⌘K" },
  { t: "Focus the Project agent pane", k: "⌘L" },
  { t: "Hide or show the pane", k: "⌘\\" },
  { t: "New Project agent", k: "⇧⌘A" },
  { t: "Board", k: "⇧⌘B" },
  { t: "Terminals", k: "⇧⌘T" },
  { t: "Activity", k: "⇧⌘U" },
  { t: "Expand the ticket panel", k: "⇧⌘E" },
  { t: "Project 1–9", k: "⌘1–9" },
];

export const NAMED_AGENTS: readonly AgentName[] = AGENT_NAMES;
