// The demo's state and every change to it, apart from drawing (WEB-07; the app's rule in SDD §5.5): a small store
// ported from the canvas's Demo.dc.html script. React subscribes and draws; the timed steps go through a scheduler, so
// tests can drive them with fake timers and `dispose()` clears every one. Nothing is stored and nothing leaves the page.

import {
  AGENTS,
  type AgentName,
  type Answer,
  type Column,
  initialAgents,
  initialTickets,
  L,
  type Line,
  project,
  type ProjectAgent,
  type ProjectKey,
  T,
  type Ticket,
} from "./data";

export type PanelTab = "terminal" | "diff" | "run" | "shell" | "ticket" | "history";
export type SettingsPage =
  | "tools"
  | "agents"
  | "notifications"
  | "appearance"
  | "keyboard"
  | "data"
  | "about"
  | "proj-NOTE"
  | "proj-SHOP";
export type Role = "dev" | "val" | "lead" | "proj";
export type ProjectField = "setup" | "run" | "instr";
export type Theme = "dark" | "light";

export interface DemoState {
  project: ProjectKey;
  tickets: readonly Ticket[];
  agents: readonly ProjectAgent[];
  agentSel: Readonly<Record<ProjectKey, string>>;
  openKey: string | null;
  ptab: PanelTab;
  inboxOpen: boolean;
  settingsOpen: boolean;
  spage: SettingsPage;
  paneHidden: boolean;
  doneOpen: boolean;
  toast: string | null;
  coach: boolean;
  search: string;
  notif: boolean;
  fontSize: number;
  rechecking: boolean;
  /** The theme card picked in Settings → Appearance. The site draws the demo dark whatever it is (WEB-03). */
  theme: Theme;
  roles: Readonly<Record<Role, AgentName>>;
  projFields: Readonly<Record<ProjectKey, Readonly<Record<ProjectField, string>>>>;
  openCodeInstalled: boolean;
  nextId: Readonly<Record<ProjectKey, number>>;
  /** Counts the agents added with New agent, for their ids. */
  newAgents: number;
}

export function initialState(): DemoState {
  const [note, shop] = [project("NOTE"), project("SHOP")];
  return {
    project: "NOTE",
    tickets: initialTickets(),
    agents: initialAgents(),
    agentSel: { NOTE: "a1", SHOP: "a3" },
    openKey: null,
    ptab: "terminal",
    inboxOpen: false,
    settingsOpen: false,
    spage: "tools",
    paneHidden: false,
    doneOpen: false,
    toast: null,
    coach: true,
    search: "",
    notif: true,
    fontSize: 13,
    rechecking: false,
    theme: "dark",
    roles: { dev: "Theo", val: "Vera", lead: "Mira", proj: "Mira" },
    projFields: {
      NOTE: { setup: note.setup, run: note.run, instr: note.instr },
      SHOP: { setup: shop.setup, run: shop.run, instr: shop.instr },
    },
    openCodeInstalled: false,
    nextId: { NOTE: 14, SHOP: 35 },
    newAgents: 0,
  };
}

/** Where the timed steps go: the browser's timers by default, fake ones in tests. */
export interface Scheduler {
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(handle: unknown): void;
}

const browserScheduler: Scheduler = {
  setTimeout: (fn, ms) => globalThis.setTimeout(fn, ms),
  clearTimeout: (handle) => {
    globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>);
  },
};

/** How long the toast stays (`say` in the canvas). */
export const TOAST_MS = 3600;

export class DemoStore {
  private state: DemoState = initialState();
  private readonly listeners = new Set<() => void>();
  private readonly timers = new Set<unknown>();
  private toastTimer: unknown = null;
  private started = false;

  constructor(private readonly scheduler: Scheduler = browserScheduler) {}

  // --- the store -----------------------------------------------------------------------------------------------

  getState = (): DemoState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  private set(patch: Partial<DemoState> | ((s: DemoState) => Partial<DemoState>)): void {
    const p = typeof patch === "function" ? patch(this.state) : patch;
    this.state = { ...this.state, ...p };
    for (const l of this.listeners) l();
  }

  /** The canvas's `componentDidMount`: NOTE-10 passes validation, NOTE-13.2 and SHOP-19 finish on their own. */
  start(): void {
    if (this.started) return;
    this.started = true;
    this.later(9000, () => {
      const t = this.find("NOTE-10");
      if (t?.st !== "validating") return;
      this.addLog("NOTE-10", [L("ok", "✓ Validator · round 1 passed: 2 of 2 acceptance checks met")]);
      this.upd("NOTE-10", () => ({ st: "review", col: "review", adds: 37, dels: 5 }));
    });
    this.later(16000, () => {
      this.runToReview("NOTE-13.2");
    });
    this.later(24000, () => {
      this.runToReview("SHOP-19");
    });
  }

  /** Clears every timer (the island unmounts). `start()` may run again after it. */
  dispose(): void {
    for (const h of this.timers) this.scheduler.clearTimeout(h);
    this.timers.clear();
    this.toastTimer = null;
    this.started = false;
  }

  /** How many timers are waiting; for tests. */
  pendingTimers(): number {
    return this.timers.size;
  }

  private later(ms: number, fn: () => void): unknown {
    const h = this.scheduler.setTimeout(() => {
      this.timers.delete(h);
      fn();
    }, ms);
    this.timers.add(h);
    return h;
  }

  private cancel(h: unknown): void {
    if (h === null) return;
    this.scheduler.clearTimeout(h);
    this.timers.delete(h);
  }

  say(text: string): void {
    this.cancel(this.toastTimer);
    this.set({ toast: text });
    this.toastTimer = this.later(TOAST_MS, () => {
      this.toastTimer = null;
      this.set({ toast: null });
    });
  }

  find(k: string): Ticket | undefined {
    return this.state.tickets.find((t) => t.k === k);
  }

  private upd(k: string, fn: (t: Ticket) => Partial<Ticket>): void {
    this.set((s) => ({ tickets: s.tickets.map((t) => (t.k === k ? { ...t, ...fn(t) } : t)) }));
  }

  private addLog(k: string, lines: readonly Line[]): void {
    this.upd(k, (t) => ({ log: t.log.concat(lines) }));
  }

  private updAgent(id: string, fn: (a: ProjectAgent) => Partial<ProjectAgent>): void {
    this.set((s) => ({ agents: s.agents.map((a) => (a.id === id ? { ...a, ...fn(a) } : a)) }));
  }

  /** The tickets `t` waits for that haven't merged yet. */
  unmet(t: Ticket): string[] {
    return unmetIn(this.state.tickets, t);
  }

  // --- a ticket's lifecycle ------------------------------------------------------------------------------------

  /** After a ticket starts or resumes: tests, submit, validation, then Ready for Review. */
  private runToReview(k: string, fast = false): void {
    this.later(fast ? 3500 : 6500, () => {
      if (this.find(k)?.st !== "working") return;
      this.addLog(k, [
        L("tool", "Bash npm test"),
        L("ok", "✓ all tests pass"),
        L("tool", "otaq submit"),
        L("a", "Submitted " + k + " for review."),
        L("sp", ""),
        L("d", "Validator · Vera · Copilot · gemini-2.5-pro · round 1"),
      ]);
      this.upd(k, () => ({ st: "validating" }));
      this.later(4000, () => {
        const t2 = this.find(k);
        if (t2?.st !== "validating") return;
        this.addLog(k, [L("ok", "✓ Validator · round 1 passed: every acceptance check met")]);
        this.upd(k, () => ({
          st: "review",
          col: "review",
          adds: t2.adds ?? 40,
          dels: t2.dels ?? 4,
          hist: t2.hist.concat([
            { at: "just now", t: "Validation round 1 passed · moved to Ready for Review" },
          ]),
        }));
        this.say(k + " is ready for review.");
      });
    });
  }

  private startAgent(k: string): void {
    const t = this.find(k);
    if (!t) return;
    const proj = project(t.p);
    const ag = AGENTS[t.agent];
    this.upd(k, () => ({
      col: "dev",
      st: "starting",
      stage: "worktree",
      tokens: "",
      log: [L("d", "Creating worktree ~/.otaq/wt/" + proj.name + "/" + k + " from " + proj.branch)],
    }));
    this.later(1300, () => {
      this.upd(k, () => ({ stage: "setup" }));
      this.addLog(k, [L("d", "Setup: " + this.state.projFields[t.p].setup + " · done in 6s")]);
    });
    this.later(2700, () => {
      this.upd(k, () => ({ st: "working", age: "active just now", tokens: "3k tokens" }));
      this.addLog(k, [
        L("d", ag.toolName + " · " + t.agent + " · " + ag.model),
        L("tool", "otaq show " + k),
        L("a", "Reading the ticket and the code it touches."),
        L("tool", "Read src/cli.js"),
        L("tool", "Edit src/cli.js"),
      ]);
      this.runToReview(k);
    });
  }

  /** A card dropped on a column, or Move to Dev in the panel. */
  move(k: string, col: Column): void {
    const t = this.find(k);
    if (!t || t.col === col) return;
    if (col === "review" || col === "done") {
      this.say("Agents move tickets to Review when they submit. Approve from the ticket’s panel.");
      return;
    }
    if (t.col === "review" || t.col === "done") {
      this.say("Use Send back in the ticket’s panel to return it to its agent.");
      return;
    }
    if (t.col === "dev") {
      this.say("Stop the agent from the ticket’s panel first.");
      return;
    }
    if (col === "backlog") {
      this.upd(k, () => ({ col: "backlog", st: "queued" }));
      return;
    }
    const un = this.unmet(t);
    if (col === "ready" || un.length) {
      this.upd(k, () => ({ col: "ready", st: "queued" }));
      if (un.length)
        this.say(k + " waits for " + un.join(" and ") + " to merge. It starts on its own after that.");
      else this.say(k + " starts when an agent is free, in priority order.");
      return;
    }
    this.set({ coach: false });
    this.startAgent(k);
    this.say("Starting " + k + " in its own worktree.");
  }

  /** Approve · Local merge. */
  approve(k: string): void {
    this.upd(k, () => ({ col: "review", st: "merging" }));
    this.later(1500, () => {
      this.upd(k, (t) => ({
        col: "done",
        st: "done",
        conflict: false,
        log: t.log.concat([L("ok", "Squash-merged into main")]),
      }));
      const waiting = this.state.tickets.filter(
        (x) => x.col === "ready" && x.waits.includes(k) && this.unmet(x).length === 0,
      );
      if (waiting.length) {
        this.say(
          k + " is merged. " + waiting.map((w) => w.k).join(", ") + " was waiting for it and starts now.",
        );
        for (const w of waiting) {
          this.later(600, () => {
            this.startAgent(w.k);
          });
        }
      } else this.say(k + " is merged into main.");
    });
  }

  sendBack(k: string): void {
    this.upd(k, () => ({ col: "dev", st: "working", round: 1, age: "active just now" }));
    this.addLog(k, [
      L("u", "❯ (sent back) Please also cover the empty-list case."),
      L("a", "Adding the empty-list case and a test for it."),
    ]);
    this.say(k + " went back to its agent with your comment.");
    this.runToReview(k, true);
  }

  // --- answers -------------------------------------------------------------------------------------------------

  answer(act: Answer): void {
    const resume = (k: string, line: string, lines: Line[]) => {
      this.upd(k, () => ({ st: "working", ask: null, age: "active just now" }));
      this.addLog(k, [L("u", "❯ " + line), ...lines]);
      this.runToReview(k, true);
    };
    switch (act) {
      case "note8":
        resume("NOTE-8", "Open $EDITOR with the current text", [
          L("a", "Got it. With no text, notes edit opens $EDITOR on the note, falling back to vi."),
          L("tool", "Edit src/cli.js  +14 −2"),
        ]);
        return;
      case "note8b":
        resume("NOTE-8", "Fail with a usage message", [
          L("a", "Okay. With no text, notes edit prints its usage and exits with 1."),
          L("tool", "Edit src/cli.js  +5 −1"),
        ]);
        return;
      case "shop22":
        resume("SHOP-22", "Allow once", [
          L("tool", "Bash pnpm prisma migrate dev --name order-status"),
          L("o", "Applied migration 20261019_order_status"),
        ]);
        return;
      case "shop22b":
        this.upd("SHOP-22", () => ({ st: "working", ask: null }));
        this.addLog("SHOP-22", [
          L("u", "❯ Deny"),
          L(
            "a",
            "Understood. I’ll write the migration file without running it, and note that in my summary.",
          ),
        ]);
        this.runToReview("SHOP-22", true);
        return;
      case "shop25":
        resume("SHOP-25", "Resume", [
          L("d", "Resumed the same conversation"),
          L("a", "Picking up where I stopped: the return URL is dropped when the state cookie expires."),
        ]);
        return;
      case "imp-open":
        this.importIssues(false);
        return;
      case "imp-closed":
        this.importIssues(true);
        return;
      case "shop-create":
        this.createCheckoutTickets();
        return;
      case "shop-no":
        this.updAgent("a3", (a) => ({
          st: "idle",
          ask: null,
          lines: a.lines.concat([
            L("u", "❯ Not yet"),
            L("a", "Okay. Tell me what to change and I’ll update the list."),
          ]),
        }));
        return;
    }
  }

  /** The "Import GitHub issues" Project agent's answer: two new Backlog tickets. */
  private importIssues(closed: boolean): void {
    const n = this.state.nextId.NOTE;
    const k1 = "NOTE-" + String(n);
    const k2 = "NOTE-" + String(n + 1);
    this.updAgent("a1", (a) => ({
      st: "idle",
      ask: null,
      created: [k1, k2],
      lines: a.lines.concat([
        L("u", closed ? "❯ Include closed ones" : "❯ Only open issues"),
        ...(closed
          ? [L("a", "All 3 closed issues are fixed in main, so there’s nothing to import from them.")]
          : []),
        L("tool", "otaq create -f import.md"),
        L("ok", "Created " + k1 + " Show a hint when the notes file is missing · GH-31 · Pip"),
        L("ok", "Created " + k2 + " Read notes from stdin · GH-34 · Theo"),
        L("a", "Both are in Backlog. " + k2 + " changes src/cli.js like NOTE-8, so it waits for NOTE-8."),
      ]),
    }));
    this.set((s) => ({
      nextId: { ...s.nextId, NOTE: n + 2 },
      tickets: s.tickets.concat([
        T({
          k: k1,
          p: "NOTE",
          title: "Show a hint when the notes file is missing",
          col: "backlog",
          st: "queued",
          prio: 3,
          agent: "Pip",
          fresh: true,
          by: "Import GitHub issues",
          desc: "From GH-31. A first run without a notes file prints an error; it should say how to start.",
          acc: ["Prints “No notes yet. Add one with notes add”"],
        }),
        T({
          k: k2,
          p: "NOTE",
          title: "Read notes from stdin",
          col: "backlog",
          st: "queued",
          prio: 3,
          agent: "Theo",
          fresh: true,
          by: "Import GitHub issues",
          waits: ["NOTE-8"],
          desc: "From GH-34. echo text | notes add should add a note.",
          acc: ["Reads stdin when no text is given and stdin isn’t a terminal"],
        }),
      ]),
    }));
    this.say("The Project agent created " + k1 + " and " + k2 + " in Backlog.");
  }

  /** The "Checkout slowness" Project agent's Create them: three new Backlog tickets. */
  private createCheckoutTickets(): void {
    const n = this.state.nextId.SHOP;
    const [k0, k1, k2] = [n, n + 1, n + 2].map((x) => "SHOP-" + String(x)) as [string, string, string];
    this.updAgent("a3", (a) => ({
      st: "idle",
      ask: null,
      created: [k0, k1, k2],
      lines: a.lines.concat([
        L("u", "❯ Create them"),
        L("tool", "otaq create -f checkout-plan.md"),
        L("ok", "Created " + k0 + " Fetch carrier quotes in parallel"),
        L("ok", "Created " + k1 + " Cache shipping quotes per cart, 5 min"),
        L("ok", "Created " + k2 + " Load the reviews widget after paint"),
        L("a", k1 + " waits for " + k0 + ": both touch lib/shipping.ts."),
      ]),
    }));
    this.set((s) => ({
      nextId: { ...s.nextId, SHOP: n + 3 },
      tickets: s.tickets.concat([
        T({
          k: k0,
          p: "SHOP",
          title: "Fetch carrier quotes in parallel",
          col: "backlog",
          st: "queued",
          prio: 2,
          fresh: true,
          by: "Checkout slowness",
          desc: "The 3 carrier calls run one after another and take 1.9 s together.",
          acc: ["Quotes are fetched in parallel", "One slow carrier doesn’t block the others past 2 s"],
        }),
        T({
          k: k1,
          p: "SHOP",
          title: "Cache shipping quotes per cart, 5 min",
          col: "backlog",
          st: "queued",
          prio: 3,
          fresh: true,
          by: "Checkout slowness",
          waits: [k0],
          desc: "Repeat visits to checkout ask the carriers again.",
          acc: ["Same cart, same address: no new carrier calls for 5 minutes"],
        }),
        T({
          k: k2,
          p: "SHOP",
          title: "Load the reviews widget after paint",
          col: "backlog",
          st: "queued",
          prio: 4,
          agent: "Pip",
          fresh: true,
          by: "Checkout slowness",
          desc: "The reviews widget adds 640 ms of hydration on checkout.",
          acc: ["Loads after first paint"],
        }),
      ]),
    }));
    this.say("The Project agent created 3 tickets in Backlog.");
  }

  // --- the Project agent pane ----------------------------------------------------------------------------------

  /** A message to the selected Project agent (the box under the pane, or a suggestion). */
  sendToAgent(raw: string): void {
    const text = raw.trim();
    if (!text) return;
    const id = this.state.agentSel[this.state.project];
    const ag = this.state.agents.find((a) => a.id === id);
    if (!ag) return;
    const name = ag.st === "new" ? (text.length > 26 ? text.slice(0, 24).trim() + "…" : text) : ag.name;
    this.updAgent(id, (a) => ({ name, st: "working", lines: a.lines.concat([L("u", "❯ " + text)]) }));
    this.later(1100, () => {
      this.updAgent(id, (a) => ({
        st: "idle",
        lines: a.lines.concat([
          L(
            "d",
            "(Demo) In Otaq, a real Claude Code session starts here, reads the code and proposes tickets for you to approve.",
          ),
        ]),
      }));
    });
  }

  newAgent(): void {
    this.set((s) => {
      const id = "n" + String(s.newAgents + 1);
      return {
        newAgents: s.newAgents + 1,
        agents: s.agents.concat([
          { id, p: s.project, name: "New agent", st: "new", tokens: "0 tokens", created: [], lines: [] },
        ]),
        agentSel: { ...s.agentSel, [s.project]: id },
      };
    });
  }

  selectAgent(id: string): void {
    this.set((s) => ({ agentSel: { ...s.agentSel, [s.project]: id } }));
  }

  /** An inbox item for a Project agent: its project, the pane open, that agent selected. */
  showAgent(id: string): void {
    const a = this.state.agents.find((x) => x.id === id);
    if (!a) return;
    this.set((s) => ({
      project: a.p,
      paneHidden: false,
      inboxOpen: false,
      openKey: null,
      agentSel: { ...s.agentSel, [a.p]: a.id },
    }));
  }

  togglePane(): void {
    this.set((s) => ({ paneHidden: !s.paneHidden }));
  }

  // --- the board and the panel ---------------------------------------------------------------------------------

  pickProject(key: ProjectKey): void {
    this.set({ project: key, openKey: null, search: "" });
  }

  setSearch(search: string): void {
    this.set({ search });
  }

  hideCoach(): void {
    this.set({ coach: false });
  }

  showDone(): void {
    this.set({ doneOpen: true });
  }

  openTicket(k: string, tab: PanelTab = "terminal"): void {
    const t = this.find(k);
    this.set((s) => ({ openKey: k, ptab: tab, project: t ? t.p : s.project, inboxOpen: false }));
    if (t?.fresh) this.upd(k, () => ({ fresh: false }));
  }

  closePanel(): void {
    this.set({ openKey: null });
  }

  setPanelTab(ptab: PanelTab): void {
    this.set({ ptab });
  }

  /** Typing into the ticket's terminal. */
  typeToTicket(raw: string): void {
    const text = raw.trim();
    const k = this.state.openKey;
    if (!text || !k || !this.find(k)) return;
    this.addLog(k, [
      L("u", "❯ " + text),
      L("d", "(Demo) In Otaq, this goes straight into the agent’s real session."),
    ]);
  }

  // --- header, inbox, Settings ---------------------------------------------------------------------------------

  toggleInbox(): void {
    this.set((s) => ({ inboxOpen: !s.inboxOpen }));
  }

  openSettings(): void {
    this.set({ settingsOpen: true, inboxOpen: false });
  }

  closeSettings(): void {
    this.set({ settingsOpen: false });
  }

  setSettingsPage(spage: SettingsPage): void {
    this.set({ spage });
  }

  /** Check again on Settings → Agent tools: OpenCode is found after 1.3 s. */
  recheck(): void {
    this.set({ rechecking: true });
    this.later(1300, () => {
      this.set({ rechecking: false, openCodeInstalled: true });
      this.say("Checked again: OpenCode is installed and signed in now.");
    });
  }

  setRole(role: Role, agent: AgentName): void {
    this.set((s) => ({ roles: { ...s.roles, [role]: agent } }));
  }

  toggleNotif(): void {
    this.set((s) => ({ notif: !s.notif }));
  }

  pickTheme(theme: Theme): void {
    this.set({ theme });
  }

  fontDown(): void {
    this.set((s) => ({ fontSize: Math.max(10, s.fontSize - 1) }));
  }

  fontUp(): void {
    this.set((s) => ({ fontSize: Math.min(20, s.fontSize + 1) }));
  }

  setProjectField(key: ProjectKey, field: ProjectField, value: string): void {
    this.set((s) => ({ projFields: { ...s.projFields, [key]: { ...s.projFields[key], [field]: value } } }));
  }
}

/** The tickets `t` waits for that haven't merged yet, among `tickets`. */
export function unmetIn(tickets: readonly Ticket[], t: Ticket): string[] {
  return t.waits.filter((w) => {
    const x = tickets.find((y) => y.k === w);
    return x !== undefined && x.col !== "done";
  });
}
