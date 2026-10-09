# Crewboard — Software Design Document (v0.1)

| | |
|---|---|
| **Product** | Crewboard: a native kanban board that drives Claude Code agents |
| **Version** | v0.1 (personal build, used daily, MIT licensed) |
| **Date** | 2026-10-09 |
| **Sources** | `decisions/` 0001–0062 (source of truth), `DESIGN.md` (overview), `design-briefs/` (UI requests), pen.dev document **`crewboard.pen`** (visual design) |
| **Audience** | (1) an agent that turns this document into tickets; (2) agents and humans implementing those tickets |

---

## 0. How to use this document

### 0.1 Source of truth
- The **decision log** (`decisions/`) wins over this document. Every rule here cites its decision as `(dNNNN)`. If this document
  and a decision disagree, the decision is right and this document must be fixed.
- An implementer who finds a conflict between a ticket and a decision **stops and asks the user** (d0056). Nobody resolves a
  conflict silently.
- The **visual design** lives only in pen.dev, in **`crewboard.pen`** (d0057). UI requirements name the frame they implement,
  for example *crewboard.pen → 03 · Board — shop-web · 1440 · Light*. Agents building UI need the pen.dev connection.

### 0.2 Conventions
- **Requirement IDs** look like `BRD-04`. A ticket should cite the IDs it implements.
- **Milestone tags** like `[M3]` say when a requirement is built (§24).
- **"Implementation choice"** marks details that no decision fixes (for example a port range). The implementer picks a
  sensible value, documents it in the code, and names it in the PR. These are not product decisions.
- Sample data (projects `shop-web`/SHOP, `billing-api`/BILL, `crewboard`/CREW; tickets SHOP-12…SHOP-40.3) is defined in
  `design-briefs/00-README.md` and used in all examples.

### 0.3 Guidance for the ticket-writing agent
- Create tickets **per milestone, in order** (§24). Inside a milestone, start with core (headless) work, then CLI, then UI.
- Each ticket gets: a title, "What and why", a **"Done when" checklist**, the requirement IDs, the decision numbers, the
  design frame (for UI work), and its dependencies on other tickets.
- Keep tickets small enough for one agent session (roughly one requirement group).
- Every ticket inherits the **definition of done** in §23.3.

---

## 1. Product summary

Crewboard is a native desktop app, written fully in Rust with GPUI (d0001), that turns a kanban board into a control plane
for coding agents. The user writes tickets and drags one to **Dev**. Crewboard creates an isolated git worktree and starts a
**real, interactive `claude` CLI session** for it. The user can watch and type into that session at any time through an
embedded terminal (d0008, d0022). When the agent finishes, it moves the ticket to **Ready for Review**. The user reviews the
diff, then approves (Local merge or Open PR) or sends it back with comments.

- **User:** a solo developer supervising **5–10 agents in parallel** across several projects (d0014, d0024). The user's
  attention is the bottleneck, so the UI is built around "what needs me right now?" (d0016).
- **Two levels of work** (d0059): **agent tickets** stay local. **Delivery units** (an epic, or a standalone ticket) are what
  reach the team's remote, as one branch and one PR, optionally tied to an External ID such as `PAY-123`.
- **First use:** after v0.1: new work on Crewboard itself, then the user's other projects. v0.1 itself is built outside
  Crewboard (d0062).

## 2. Principles every ticket must respect

| # | Principle | Meaning in code | Decisions |
|---|---|---|---|
| P1 | **The user is in control** | Anything that **launches `claude` (spends tokens)** or **deletes** something needs a user click. Pure bookkeeping is automatic. Exceptions are listed explicitly (§16.2). | d0018 |
| P2 | **Detect and report, never self-heal** | No auto-resume, no auto-retry, no auto-cleanup. Show the state and offer an action. | d0018 |
| P3 | **Never touch user repos or user edits** | No files written into the user's repo. Worktrees live in `~/.crewboard/wt`. Never stash, never modify a dirty checkout. | d0023, d0038 |
| P4 | **Respect the user's setup** | Inherit the user's Claude Code permission mode and settings, git config, hooks and credentials. Only the narrow exceptions in §11.1. | d0011, d0012, d0055 |
| P5 | **Don't change how `claude` works** | Crewboard runs the real interactive CLI. No test gate, no retry limit, no wrapping. Workflow policy lives in the user's skills and Agent instructions. | d0018, d0036, d0040 |
| P6 | **One banner per situation** | Never one alert per agent. OS notifications only for items that block an agent. | d0016, d0020 |
| P7 | **Never mix Crewboard's text with the user's** | Automatic typing into a session waits for an empty input line. | d0048 |
| P8 | **Low maintenance** | Prefer simple, few moving parts. Read Claude Code data read-only; a format change may break a display, never a workflow. | d0012, d0020, d0031 |

## 3. Scope

### 3.1 In v0.1
Everything in §5–§22, built in milestones M0–M11 (§24): macOS first, then Windows and Linux (d0021, d0027, d0053).

### 3.2 Not in v0.1
- **Stacking** (children building on unmerged parents, Paused, restack, stacked PRs). It may return later as a per-project
  setting (d0005, d0049).
- Graph view of dependencies (maybe later, d0006). Importers (beads, Linear, Jira) and the planner agent (d0003, d0013).
- Teams and shared boards (d0014). Code signing, package channels, the public launch (d0021, d0025).
- "Open in editor" (d0035). Per-project permission-mode override (d0016). Per-ticket services (d0026). Claude version checks (d0021).
- Polling or webhooks for PR state (d0017). Priorities (d0032, d0039).

## 4. Glossary

| Term | Meaning |
|---|---|
| **Project** | One git repository, with its own key (SHOP), board, settings and worktrees (d0024, d0029). |
| **Ticket** | A unit of agent work. Key `SHOP-18`; epic children `SHOP-40.1` (d0029, d0039). |
| **Epic** | A container ticket that is also a **delivery unit**: it has a local integration branch, no agent, and is reviewed and delivered as one feature (d0039, d0059). |
| **Standalone ticket** | A ticket not in an epic. It is its own delivery unit (d0059). |
| **Delivery unit** | What reaches the remote: an epic or a standalone ticket (d0059). |
| **External ID** | Optional ID of the human ticket in the team's tracker, e.g. `PAY-123` (d0059). |
| **Base branch** | The branch a delivery unit starts from and merges into. Project setting prefilled from the remote's default branch, overridable per ticket/epic (d0051, d0059). For epic children: the epic's branch. |
| **`base_sha`** | The commit where a ticket left its base. The Diff is against it (d0017, d0049). |
| **Session** | A process owned by the background service: the agent (`claude`), Shell, Project terminal, or a Run process (d0022, d0035). |
| **Background service** | `crewboard daemon`: owns sessions, the scheduler, the store and git (d0022). |
| **Orange family** | Asks you / Needs you / Idle: the agent is waiting for the user (d0042). |
| **Inbox** | The global "Needs you" queue across all projects (d0016, d0024). |
| **Workflow skills** | Commit, Push, Open PR: Claude Code skills, replaceable globally or per project (d0050). |
| **Agent instructions** | The user's own rules for agents, global plus per project, printed by `crewboard prime` (d0040). |
| **Project terminal** | A plain terminal in the project's own folder (d0019). |

---

## 5. Architecture

### 5.1 Components (d0002, d0022, d0057)

```
repository root (this folder, d0057)
├── crates/
│   ├── crewboard-core/   domain, SQLite store, scheduler, git & worktrees, sessions (PTY), ports, Run/Shell processes,
│   │                     hook handling, usage reading, socket server. Runs as `crewboard daemon`. No UI dependency.
│   ├── crewboard-app/    GPUI client: board, panel, terminal view, diff view, inbox, settings, onboarding.
│   └── crewboard-cli/    the `crewboard` binary: launches the app; `daemon`, `attach`, `hook`, agent commands.
├── plugin/               the Claude Code plugin: skills + hooks (agent behaviour lives here, not in Rust strings)
├── decisions/  design-briefs/  DESIGN.md  SDD.md  LICENSE (MIT)
```

```
 GPUI app (client) ◄──── JSON over user-locked socket ────► crewboard daemon (core)
 crewboard CLI     ◄────────────────────────────────────────┘   │
 (agents, hooks, user)                                          ├── PTY ── claude (ticket SHOP-18, cwd = its worktree)
                                                                ├── PTY ── shell (Shell tab / Project terminal)
                                                                ├── proc ─ Run command (log captured)
                                                                ├── git (the user's `git` CLI)
                                                                └── SQLite (~/.crewboard/)
```

- **ARC-01** `[M0]` Cargo workspace with the three crates and `plugin/`. The **product name, command name, plugin namespace
  and env-var prefix come from one constant** (d0028).
- **ARC-02** `[M0]` `crewboard-core` has no UI dependency and is fully testable headless (d0002, d0056).
- **ARC-03** `[M1]` The app is a **client**. Closing its window never stops agents (d0022).

### 5.2 Technical baseline (d0055)
| Topic | Rule |
|---|---|
| Messages | JSON over the user-locked local socket: requests/replies plus a live **event stream** the app subscribes to. Every message carries a **protocol version**. The CLI uses the same protocol. |
| Data folder | `~/.crewboard/` on every platform: database, `wt/<repo-name>/<KEY>` worktrees, `logs/`, the socket (named pipe on Windows). |
| Git | Run the user's own `git` command, never a git library. |
| Crates | `gpui` pinned to one Zed commit (use Zed's source as the reference, d0001), `alacritty_terminal`, `portable-pty`, `sqlx` (SQLite), `tokio`, `syntect`, `tracing`. |
| Service start | The app starts `crewboard daemon` when it isn't running. **No auto-start at login.** |
| Logs | `~/.crewboard/logs/`, rotated daily; path shown in Settings. |

### 5.3 Background service (d0022, d0027)
- **SVC-01** `[M1]` `crewboard daemon start | stop | status`. Owns every PTY and process, the scheduler, the store, git, Run
  and Shell sessions.
- **SVC-02** `[M1]` **Socket locked to the current OS user**: Unix socket file permissions on macOS/Linux, an owner-only ACL
  on the Windows named pipe (d0027).
- **SVC-03** `[M1]` **Build-ID and protocol check** (d0022): on connect, the app compares build IDs. If the service is older,
  show the banner **"Service update ready · Restart now (3 agents working, 2 waiting)"**. The old service keeps running until
  the user clicks. If the protocol is incompatible, the same banner reads **"Restart required"** and only the Restart button
  works. Restart confirmation: "3 working agents and 2 waiting ones will be interrupted. You can Resume all afterwards." A
  restart is treated like a reboot (§17). The service **never restarts itself**.
- **SVC-04** `[M1]` The app reconnects automatically. While disconnected: a calm "Reconnecting… agents keep running" banner,
  terminals keep their scrollback, input is disabled (brief 02, 06).
- **SVC-05** `[M3]` The service **stores scrollback** per session so the app can redraw a terminal on reconnect (d0022). Size
  limit: implementation choice.
- **SVC-06** `[M3]` The service **relays every keystroke** from the app or `crewboard attach` to the PTY, and tracks a per-session
  **"unsent input" flag**: set by any typed character, cleared by Enter or when Crewboard clears the line (d0048).
- **SVC-07** `[M1]` Advice (d0027): keep the service small and boring. It is the component whose crash interrupts every agent.

### 5.4 Protocol surface (logical; exact message names are an implementation choice)
- **Requests:** projects (add, update settings, remove, list); tickets (create, update, delete (never-started only), move, pin, close, reopen, park,
  submit for review, approve-local-merge, open-pr, push-to-pr, mark-merged, mark-closed, send-back, add comment, move to epic);
  dependencies (add, remove, tree, cycles); sessions (attach, input, resize, resume, resume all, retry start, start from scratch,
  deliver now); Run (start, stop); Shell and Project terminal (open); inbox (list, answer); cleanup (list, delete); usage;
  settings; service (status, restart).
- **Events:** ticket changed, card status changed, session output, session state, inbox changed, banner changed, usage
  changed, run output, toast.
- **CLI requests** carry the **caller identity** (§11.3).

---

## 6. Domain model

### 6.1 Projects (d0023, d0024, d0029, d0059)
- **PRJ-01** `[M2]` A project is **one git repository**. Adding one needs only a folder that is a git repo (zero config, d0023).
  Validation: not a git repo; already added.
- **PRJ-02** `[M2]` **Project key**: short uppercase key suggested from the repo name (`shop-web` → `SHOP`), editable when
  adding, **not changeable later** (d0029, brief 14).
- **PRJ-03** `[M2]` **Base branch** setting: **prefilled with the remote's default branch** (where `origin/HEAD` points). A
  project without a remote uses the current branch. Editable later (d0059).
- **PRJ-04** `[M2]` Removing a project deletes its tickets in Crewboard, **never the repo** (brief 15).
- **PRJ-05** `[M2]` Dependencies stay **within one project** (d0024).

### 6.2 Tickets and epics
- **TKT-01** `[M2]` **Keys:** `<KEY>-<number>`, numbers increase per project and are **never reused**, even after deletion
  (d0029). Epic children use `<EPIC-KEY>.<n>` (`SHOP-40.1`), one level only, numbers never reused within the epic (d0039).
- **TKT-02** `[M2]` **Types:** `ticket` (default) and `epic`. No priorities (d0032, d0039).
- **TKT-03** `[M2]` **Ticket fields** (d0019, d0051, d0059):
  - Title; **What and why** (Markdown); **Done when** (ordered checklist); **Depends on**
  - **Start from and merge into** (base branch): standalone tickets and epics only; defaults to the project's base branch,
    locked once started. Epic children show it read-only: "From the epic's branch (SHOP-40)".
  - **External ID** (optional): epics and standalone tickets only.
  - Origin: "created by agent" when an agent created it (d0019, d0032 `--actor`).
- **TKT-04** `[M2]` **Empty "Done when"** shows a **warning** dialog when the ticket is dragged to Dev or Started ("Start
  anyway" / "Add criteria"). It never blocks (d0019).
- **TKT-05** `[M2]` **Agent edits are visible:** "Updated by agent" marker with a before/after view (d0019, d0036).
- **TKT-06** `[M2]` **Pinning** (d0047): any ticket or epic can be pinned/unpinned (right-click menu and the panel's "…" menu).
  A pinned ticket is **always first in its column**; several pins → most recently pinned first; the pin follows the ticket
  across columns. Pinning affects board order only.
- **TKT-08** `[M2]` **Delete** (d0060): only tickets/epics that **never went to Dev**. From the card's right-click menu,
  the panel's "…" menu, and the Project terminal's summary of created tickets (RUN-06). Confirmation names any dependent
  tickets and removes those links. The number is never reused. Started tickets can only be Closed.
  **Deleting an epic** deletes its never-started children too, in one confirmation that lists them.
- **TKT-07** `[M2]` **Key aliases** (d0044): a never-started ticket moved into an epic gets the next child key
  (SHOP-51 → SHOP-50.2); the old key keeps resolving everywhere (`crewboard show SHOP-51`).

### 6.3 Dependencies (d0005 DAG part, d0006, d0049, d0051)
- **DEP-01** `[M2]` Tickets form a DAG with **any number of dependencies**. A dependency that would create a cycle is
  rejected, in the editor (with an inline error that explains the loop, e.g. SHOP-21 → SHOP-20 → SHOP-21) and in the CLI.
- **DEP-02** `[M2]` **Linked tickets must share a base.** A link across bases is rejected: "SHOP-31 merges into `release/2.3`,
  SHOP-20 into `main`. Linked tickets must use the same base branch." Epic children share the epic's branch (d0051, d0059).
- **DEP-03** `[M5]` **Readiness (beads-style):** a dependency is ready only when the parent is **Done · Merged** into the
  child's base (for epic children: merged into the epic's branch). **Closed never counts** (d0044, d0049).
- **DEP-04** `[M2]` A ticket can **depend on an epic**. It's ready when the epic is **Done · Merged**. An epic that ended
  "Closed · not merged" never counts (d0046). The dependent must have the epic's base.
- **DEP-05** `[M2]` **Visualization:** dependency badges on cards (`⛓ SHOP-18 SHOP-19`); hover or select a card to highlight
  its whole chain across columns, dim the rest, and draw arrows **only for that chain**; the **Stack list** in the side panel
  (indented parents and children with statuses; click to jump) (d0006).

### 6.4 Epics as delivery units (d0039, d0044, d0059)
- **EPC-01** `[M2]` An epic card shows title, External ID chip, progress ("2 / 3 merged") and its child list. "Add ticket to
  epic" creates a child. The board can be filtered by epic; children show an epic chip.
- **EPC-02** `[M3]` The epic gets a **local integration branch** when its first child starts (temporary name
  `crewboard/SHOP-40`), created from its base branch (§10.1). No agent; merges into it run in a Crewboard-owned scratch
  worktree.
- **EPC-03** `[M6]` **Children branch from the epic's branch and Local-merge back into it.** Never pushed. Children don't
  offer Open PR.
- **EPC-04** `[M6]` When **every child is Done** (merged, or closed with at least one merged), the epic moves to **Ready for
  Review**. If every child ended Closed, the epic becomes **"Closed · not merged"** (d0044).
- **EPC-05** `[M6]` Epic review: the Diff shows the **whole feature** (epic branch vs its base), grouped by child. Approve:
  **Open PR** (one commit per merged child) or **Local merge** into the base branch. **No Send back** on an epic: "Add or reopen
  a child ticket to change it". Adding or reopening a child returns the epic to Backlog until that child is merged.
- **EPC-06** `[M2]` **Closing an epic** with unfinished children: one confirmation lists the children to close and agents to
  stop, then closes them all. Merged children stay merged (d0044).
- **EPC-07** `[M2]` **Move to epic** is available only for tickets that **never went to Dev**; the ticket gets a child key and
  uses the epic's branch (d0044, d0059).
- **EPC-08** `[M10]` Open PR / Push to PR on an epic runs the Open PR / Push skill in a **short-lived `claude` session in the
  epic's scratch worktree**, started by the user's click and ended after `crewboard pr opened` (the epic has no agent session).

---

## 7. Ticket lifecycle

### 7.1 Columns (d0004, d0034, d0036, d0037)
**Backlog → Dev (Queued + Agent Working) → Ready for Review → Approved → Done.** Five columns. No Investigate, Ready for Dev,
Needs Input, Changes Requested or Failed columns.

### 7.2 States
| Column | Sub-state / status | Meaning | Decisions |
|---|---|---|---|
| Backlog | normal | Not started | d0004 |
| Backlog | **Parked · has work** | Taken out of Dev; branch, worktree and Claude session kept | d0052 |
| Backlog | epic (in progress) | Epic with children working | d0059 |
| Dev · Queued | waiting for dependencies | "waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)" | d0049 |
| Dev · Queued | waiting for a slot | Optional limit full (d0016) | d0016 |
| Dev · Queued | held | Plan limit active, or base diverged ("`develop` and `origin/develop` have diverged. Push or pull, then retry") | d0020, d0059 |
| Dev · Queued | **parent closed** | "parent SHOP-26 closed" + inbox item | d0033, d0044 |
| Dev · Queued | **Start interrupted** | Start stopped halfway → Retry start | d0018 |
| Dev · Working | working | Agent active | d0004 |
| Dev · Working | **Working in background** | Turn ended but background tasks or wakeups pending (neutral) | d0042 |
| Dev · Working | **Asks you** / **Needs you** / **Idle** | Orange family | d0042 |
| Dev · Working | **Waiting for limit reset** | Stopped by the plan limit (neutral, no inbox item) | d0046 |
| Dev · Working | **Interrupted** | Session died → Resume | d0018, d0045 |
| Dev · Working | **Resume failed** | Retry / Start from scratch | d0018 |
| Ready for Review | — | Agent (or user) submitted | d0004, d0052 |
| Approved | **PR open** | PR opened; waits for Mark merged / Mark closed | d0015, d0017 |
| Done | **Merged** | Merged locally or Mark merged | d0004 |
| Done | **Closed · not merged** | Closed without merging; reopenable | d0033 |

**Flags** (independent of the state): pinned; sent back · N comments · round K (d0034); conflicts with `<base>` (d0038);
changed since review (d0043); N messages waiting (d0048); Run running `▶ :4020` (d0035); updated by agent (d0019); answering
SHOP-40.2 (d0030); asked SHOP-40.1 · waiting (on the asker, until delivered; low priority, d0030); last signal (d0027); tokens (d0020); PR #N chip; "PR provider not detected" (d0050).

### 7.3 Transitions
| From | To | Trigger | Actor | Notes |
|---|---|---|---|---|
| Backlog | Dev · Queued | Drag to Dev / Start (⇧⌘D) | User | Empty Done-when warning (TKT-04). Epics can't be dragged to Dev. |
| Backlog (Parked) | Dev | Drag to Dev | User | **Resumes the same conversation** (`claude --resume`), new ports (d0052) |
| Backlog (Closed→reopened) | Dev | Drag to Dev | User | Old branch reused; free rebase onto the current base first; then a **new session** (`work-ticket`), not a resume; on conflict, `crewboard prime` tells the agent to resolve it first (d0044) |
| Queued | Working | Scheduler (§13) | Service | Start sequence §10 |
| Working | Backlog (Parked) | Drag Dev → Backlog + confirm | User | Agent stopped; work kept (d0052) |
| Working | Ready for Review | `crewboard review submit` | Agent | Refused while uncommitted files exist (d0050) |
| Working (not mid-turn) | Ready for Review | "Submit for review" + optional note | User | Same checks; summary "Submitted by you" (d0052) |
| Ready for Review | Dev | **Send back** | User | Comments + note to the same session; "Sent back · N comments · round K"; if the limit is on and full → Queued (d0034) |
| Ready for Review | Done · Merged | Approve → **Local merge** | User | §14.3. For epic children: into the epic branch |
| Ready for Review | Approved · PR open | Approve → **Open PR** | User | Delivery units only (§14.6) |
| Approved | Ready for Review | Drag back (PR rework) | User | PR chip stays; then Send back (d0041) |
| Ready for Review (PR exists) | Approved · PR open | Approve → **Push to PR #N** | User | §14.7 |
| Approved | Done · Merged | **Mark merged** | User | Then fetch base, start waiting tickets (d0049) |
| Approved | Done · Closed | **Mark closed** | User | (d0017, d0033) |
| Any | Done · Closed | **Close ticket** (+ confirm if an agent runs) | User | Branch kept; worktree removed unless dirty; Run/Shell stop (d0033) |
| Done · Closed | Backlog | Drag out / Reopen | User | **Only Closed tickets** can be reopened; Merged tickets are locked (d0044) |
| Epic (Backlog) | Ready for Review | All children Done (≥1 merged) | Service | EPC-04 |

### 7.4 Drag rules (brief 03, d0041, d0052)
- **Allowed:** Backlog → Dev; Dev → Backlog (Park); Ready for Review → Approved (asks: Local merge or Open PR); Approved →
  Ready for Review (PR rework); Done (Closed only) → Backlog.
- **Not allowed** (show "not allowed" feedback): into Agent Working or Ready for Review by drag (only agents, or the user's
  "Submit for review" menu action); Merged tickets out of Done; epics into Dev.

### 7.5 Ordering (d0037, d0042, d0047)
- **Every column:** pinned first (most recently pinned first).
- **Dev:** pinned → orange (Asks you / Needs you / Idle, in inbox order) → Interrupted / Start interrupted / Resume failed /
  Parent closed (inbox group 2 order) → working (incl. Working in background, Waiting for limit reset) → Queued (its own section;
  pinned tickets also first there). **Needs-you cards are never folded** under a "N more" row; the fold covers only working and
  queued cards ("4 more · 2 working · 2 queued").
- Inside a group, tickets that **block more tickets** come first (same rule as the inbox, d0016).

---

## 8. Data storage (d0003, d0055)

SQLite via `sqlx`, written so a move to Postgres stays possible. File in `~/.crewboard/`. Migrations from M0. Backup relies on
the user's machine backup; Settings shows the database path (d0003). Proposed tables (implementers may refine names and
types; the facts to store are fixed):

| Table | Key contents |
|---|---|
| `projects` | id, key, name, repo_path, base_branch, next_ticket_number, settings (setup cmd, run cmd, open URL, env files, agent instructions override, workflow skill overrides, "let finished tickets answer" flag), created_at |
| `settings` | global key/values: agent limits (on/off, max, back-pressure), agent instructions, workflow skills, dollar estimate, notifications, appearance, terminal font size |
| `tickets` | id, project_id, number, epic_id, child_number, kind (ticket/epic), title, body, external_id, column, state, outcome (merged/closed), pinned_at, parked, base_branch (resolved), branch_name, branch_locked, base_sha, review_head_sha, review_entered_at, worktree_path, agent_port, run_port, pr_url, pr_number, sent_back_round, created_by, updated_by, started_at, created_at, updated_at |
| `ticket_done_when` | ticket_id, position, text, added_by |
| `ticket_deps` | ticket_id, depends_on_id |
| `ticket_key_aliases` | old_key, ticket_id (d0044) |
| `sessions` | id, ticket_id (nullable for Project terminal), kind (agent/answer/delivery/shell/project/run), pty_id, claude_session_id, state, started_at, ended_at, end_reason. **Every Claude session ID is kept** (d0045) |
| `events` | id, ticket_id, kind, actor, payload_json, at: audit log, History tab, "last signal" source (d0027) |
| `review_comments` | id, ticket_id, round, file, line, side, body, created_at, sent_at |
| `reviews` | id, ticket_id, round, action (send_back/approve_local/open_pr/push), note, at |
| `messages` | id, from_ticket_id, to_ticket_id, question, answer, delivery (live/resumed/inbox), asked_at, delivered_at, answered_at (d0030) |
| `pending_deliveries` | id, session_id, kind (agent message, ticket-change note, continue after limit), text, created_at (d0048) |
| `usage` | session_id, tokens in/out/cache, day (d0020) |
| `plan_limits` | five_hour {used %, resets_at}, seven_day {used %, resets_at}, limit_active, observed_at (d0020) |

Derived (not stored, computed by the service): inbox items, readiness, sort order, cleanup candidates.

---

## 9. Starting, sessions and terminals

### 9.1 Start sequence (d0007, d0018, d0026, d0049, d0059)
- **SES-01** `[M3]` Starting a ticket (scheduler or Resume/Retry), in order:
  1. Resolve the base: standalone ticket or epic → its base branch; epic child → the epic's branch (create the epic branch
     on first start, EPC-02).
  2. For a base branch (not an epic branch): `git fetch`; if the local base is only **behind** its remote, fast-forward it
     (in the user's checkout only if clean, d0038); if **diverged**, hold in Queued with the diverged message (d0059).
  3. Create the branch `crewboard/<KEY>` and the worktree `~/.crewboard/wt/<repo-name>/<KEY>` (git worktree) from the base;
     record `base_sha`.
  4. Copy the project's **env files** into the worktree (d0010, d0023).
  5. Run the project's **setup command** in the worktree, if set (d0007).
  6. Allocate **two ports**: agent port and Run port (d0026).
  7. Start the agent session in the service (§9.2), store the Claude session ID, set the ticket to Agent Working.
- **SES-02** `[M3]` The start is **idempotent**: re-running it reuses an existing worktree or branch. A start that stopped
  halfway shows **"Start interrupted"** with one action, **Retry start**, which re-runs the whole sequence. It is never re-run
  silently (d0018).

### 9.2 Agent launch (d0009, d0011, d0020, d0031)
- **SES-03** `[M3]` Launch, with cwd = the worktree:
  ```
  CREWBOARD_TICKET=SHOP-18  PORT=<agent port>  PATH=<crewboard bin dir>:$PATH
  claude --plugin-dir <crewboard plugin> \
         --allowedTools "Bash(crewboard:*)" \
         --settings <Crewboard status-line settings>   # §18.2
         "/crewboard:work-ticket"
  ```
  - No permission mode or other permission flags: the session uses the user's own Claude Code settings (d0011). The only
    pre-approval is `Bash(crewboard:*)`.
  - The exact plugin-loading flag is verified when built (d0009).
- **SES-04** `[M3]` One agent session per ticket. The user can **type into it at any time** (d0008, d0022).
- **SES-05** `[M3]` `crewboard attach <KEY>` opens a ticket's live session from any terminal (d0022).

### 9.3 Typing into sessions (d0048)
- **SES-06** `[M3]` **Automatic deliveries** are sent only when the agent is **idle between turns and the input line is
  empty**: another agent's question or answer (d0030), the ticket-change note (§9.4), "continue" after the limit reset (d0046).
  Until then the card and terminal header show **"1 message waiting · Deliver now"**.
- **SES-07** `[M3]` **The user's own clicks** (Send back, an inbox answer) are never held. If the input line has unsent text:
  *"SHOP-23 has unsent text in its terminal"* → **Keep it and cancel** / **Clear it and send**. Crewboard **never clears the
  user's text silently**.

### 9.4 Ticket edits while working (d0048)
- **SES-08** `[M4]` When the **user** saves a change to a ticket whose agent is running, Crewboard queues a short note with
  only what changed, e.g. *"The ticket was updated by the user. Added to 'Done when': 'works on mobile widths'. Run
  `crewboard prime` for the full ticket."* Agent edits send nothing. Queued, Parked and Backlog tickets get nothing. The
  editor shows "SHOP-19's agent will be told what changed."

### 9.5 Terminal view (d0001, d0022, brief 06)
- **SES-09** `[M3]` `alacritty_terminal` emulation rendered in GPUI: ANSI colours, cursor, selection, scrollback, **find in
  scrollback** (⌘F / Ctrl+Shift+F), "jump to latest", font size.
- **SES-10** `[M3]` **Focus:** while the terminal has focus, every key goes to Claude Code, Esc included. Leave with ⇧⌘` /
  Ctrl+Shift+`, a click outside, or the inbox shortcut. On Windows/Linux app shortcuts move to Ctrl+Shift (§21.3).
- **SES-11** `[M3]` Header strip: session state, worktree path, agent `$PORT`, "Copy attach command", font size, and
  "Instructions: shop-web" linking to settings (d0040).

---

## 10. Git operations (d0038, d0044, d0049, d0050, d0055, d0059)

All git work uses the user's `git` CLI. Crewboard never stashes and never modifies a dirty user checkout.

### 10.1 Keeping the base current
- **GIT-01** `[M6]` Before a delivery unit starts and before a Local merge into a base branch: `git fetch`; fast-forward the
  local base if it is only behind; refuse/hold if it has diverged (§9.1 step 2). **Local merge never pushes.**

### 10.2 Checkpoint commit (d0005)
- **GIT-02** `[M6]` Before any Crewboard rebase in an agent's worktree: commit the agent's uncommitted changes as
  `wip: paused by Crewboard`, rebase, then `git reset --soft HEAD~1` so the files come back exactly as they were. If the rebase
  or the undo conflicts, the ticket gets the conflict handling of the operation that caused it. Never stash.
  **The user's git hooks run on it** (P4, d0055). If a hook rejects the checkpoint, stop with: "Your pre-commit hook
  rejected Crewboard's temporary checkpoint. Commit or discard the files in SHOP-26, then retry." No `--no-verify`.

### 10.3 Local merge (d0015, d0038, d0059)
- **GIT-03** `[M6]` On the user's Local merge click, in a **Crewboard-owned scratch worktree**:
  1. Refuse if the ticket's worktree has **uncommitted files** ("SHOP-19 has 2 uncommitted files" · View · Send back) (d0050).
  2. Update the target (GIT-01 for a base branch; the epic branch is local only).
  3. **Free rebase** of the ticket branch onto the latest target (no tokens).
  4. **Clean →** squash-merge into the target with the confirmed squash message (editable in the confirmation). If the target
     is checked out in the user's folder and that checkout is **clean**, fast-forward it. If it has **uncommitted changes**,
     refuse: "shop-web has uncommitted changes on main. Clean up and retry."
  5. **Conflict →** nothing is merged; the ticket stays in Ready for Review with **"Send to agent to fix"**, which returns it to
     Dev with the `restack` skill (the click spends the tokens).
- **GIT-04** `[M6]` **Early conflict flag:** after every merge into a branch, run a **read-only test merge** for every ticket
  in Ready for Review that targets that branch. Conflicting tickets get **"conflicts with `<target>`"** on the card and panel.
- **GIT-05** `[M6]` Crewboard's own rebases (onto the target) don't trigger "changed since review" (d0043).

### 10.4 Reopen (d0044)
- **GIT-06** `[M6]` When a reopened (previously Closed) ticket is dragged to Dev, its old branch is first rebased onto the
  current base (no tokens, GIT-02 applies). The agent then starts in a **new session** (not a resume). On a conflict,
  `crewboard prime` tells it to resolve that first (`restack` skill).

### 10.5 Branch names (d0050)
- **GIT-07** `[M3]` Tickets and epics start on a **temporary local branch** `crewboard/<KEY>`. The **real name** is chosen at
  the **first push** (§14.6) and then **locked**: `crewboard branch rename` refuses afterwards. Worktree folders keep key-based
  names. History logs the rename.

### 10.6 Commit ownership (d0050)
- **GIT-08** `[M4]` **The agent commits**, through the Commit skill. **Crewboard never commits by itself**, except the
  checkpoint it undoes (GIT-02).

---

## 11. Agent integration

### 11.1 The plugin (d0009, d0031, d0040, d0050)
```
plugin/
  .claude-plugin/plugin.json
  skills/
    work-ticket/       crewboard prime → plan → implement → Commit skill → crewboard review submit
    address-review/    crewboard review feedback → fix → Commit skill → crewboard review submit
    restack/           resolve rebase conflicts (Send to agent to fix; reopen)
    answer-question/   resumed finished ticket: answer a linked agent's question, change nothing, reply with crewboard answer
    commit/ push/ open-pr/   built-in workflow skills (replaceable, §14.6)
  hooks/hooks.json     Notification, Stop, StopFailure, UserPromptSubmit → `crewboard hook <event>`
```
- **PLG-01** `[M4]` Agent behaviour lives in Markdown skills; changing it needs no recompile (d0009).
- **PLG-02** `[M4]` Every skill tells the agent to run `crewboard prime` first and to **follow the Agent instructions** it
  prints. The user's text can't break Crewboard's own steps (get context, submit) (d0040).
- **PLG-03** `[M4]` The `work-ticket` and `address-review` skills tell the agent to **commit everything** with the configured
  Commit skill before submitting (d0050).
- **PLG-04** `[M4]` Hooks **do nothing when `CREWBOARD_TICKET` is unset**, so the user's ordinary `claude` sessions are
  unaffected. The plugin also works by hand: `CREWBOARD_TICKET=SHOP-12 claude` in a worktree (d0009).
- **PLG-06** `[M4]` The `work-ticket` and `address-review` skills teach the agent to offer **2–4 options with a recommendation**
  for questions with clear alternatives (`ask human --option … --recommended N`), and free text otherwise (d0061).
- **PLG-05** `[M4]` Long text always goes through **stdin** (`--stdin`), never quoted arguments (d0031).

### 11.2 CLI reference (d0031, d0032, d0039, d0050)
Conventions: global `--json`, `-q/--quiet`, `--silent` (print only the ID), `-P/--project <KEY>`, `--actor`. JSON errors as
`{"error": {...}}` with a non-zero exit. Omitted KEY → `$CREWBOARD_TICKET`. A bare number (`18`) resolves in the current
project. Human-readable output by default.

| Command | Purpose |
|---|---|
| `crewboard prime` | Context for the current ticket: details, Done when, parents and their summaries, review feedback, pending questions, rules, **Agent instructions** (global + project) |
| `crewboard show [KEY]` (alias `view`) | Ticket details; `--long` adds messages and history; `--children` for epics |
| `crewboard list` | `--status`, `--project`, `--tree`, `--parent <EPIC>` |
| `crewboard ready` / `blocked` | Dependency state per project |
| `crewboard create "Title"` (alias `new`) | `-d`, `--done-when <item>` (repeatable), `--deps A,B`, `--stdin` (Markdown body), `-f <file>` (batch), `--type epic`, `--parent <EPIC>`, `--silent`. Lands in **Backlog** |
| `crewboard update [KEY]` | `--title`, `-d`, `--add-done-when`, `--stdin` |
| `crewboard ask human [--option "<label>" …] [--recommended N] --stdin` | Shows "Asks you" at the top of Dev; 2–4 optional answer options (d0061) |
| `crewboard ask <KEY> --stdin` | Ask a directly linked ticket (§16) |
| `crewboard answer <QUESTION-ID> --stdin`, `crewboard questions` | Answer / list pending questions |
| `crewboard review submit [KEY] --stdin` | Summary on stdin; moves to Ready for Review; **refused with uncommitted files** |
| `crewboard review feedback [KEY]` | In-app comments + unresolved PR comments (fetched on demand) |
| `crewboard dep add <KEY> <DEPENDS-ON>`, `dep remove`, `dep tree [KEY]`, `dep cycles` | Dependencies; `dep tree` prints the Stack list |
| `crewboard pr propose --stdin` | Open PR skill proposes branch name, commit messages, PR title/description |
| `crewboard pr opened <url>` | Open PR skill reports the PR |
| `crewboard branch rename <name>` | Refused after the first push |
| `crewboard project list \| show` | Projects |
| `crewboard attach <KEY>` | Attach to a live session |
| `crewboard daemon start \| stop \| status` | Service control |
| `crewboard hook <event>` | Internal, called by the plugin hooks |

Not adopted from beads: hash IDs, `--claim`, priorities, `bd init`/`AGENTS.md`, Dolt sync, gates, molecules (d0032).

### 11.3 Who may change what (d0031)
- **CLI-01** `[M4]` The service identifies an **agent caller** by **where the process runs** (inside one of its agent
  sessions, by process ancestry), not only by `$CREWBOARD_TICKET`.
- **CLI-02** `[M4]` An agent caller may: **read anything** in the project; **change only its own ticket** (`update`, `review
  submit`, `ask`, `answer`, `pr propose`, `pr opened`, `branch rename`); **create tickets in Backlog** (incl. `--parent`); add
  dependencies only from its own ticket or between tickets it created. Everything else is refused: "SHOP-18's agent can't
  change SHOP-12".
- **CLI-03** `[M4]` The Project terminal and the user's own terminals have full access. The service enforces every rule
  (cycles, same base, permissions); the CLI is a thin client.
- **CLI-04** `[M4]` `--actor` defaults to `agent:<KEY>` for agents, else the OS user. It drives "created/updated by agent".

### 11.4 Hook mapping (d0009, d0020, d0042, d0046)
| Hook | Condition | Result |
|---|---|---|
| `Notification` | `notification_type` ∈ `permission_prompt`, `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input` | **Needs you** (incl. prompts inside a helper agent) |
| `Notification` | any other type (`idle_prompt`, `auth_success`, …) | ignored |
| `Stop` | `background_tasks` or `session_crons` non-empty | **Working in background**, showing what it waits for ("background: test run, 1 helper agent · 6 min", "wakes up at 4:10pm") |
| `Stop` | both empty, and no `review submit` / `ask human` this turn | **Idle** (orange) |
| `StopFailure` | error `rate_limit` and the limit text ("You've hit your session limit · resets 3:45pm" / "…weekly limit…") | **Waiting for limit reset** + banner (§18) |
| `UserPromptSubmit` | — | back to working (new turn, incl. automatic re-invocation after background work) |
| any hook / any `crewboard` command | — | updates **last signal** |

Hook payload fields beyond the documented ones must be verified by logging real payloads (d0020, d0042).

---

## 12. Attention: signals, inbox, notifications

### 12.1 Orange family (d0042)
- **ATT-01** `[M4]` **Asks you** (`crewboard ask human`), **Needs you** (permission/elicitation/helper request), **Idle**
  (turn ended, nothing pending, no submit/ask). Orange card, sorted to the top of Dev, inbox group 1.
- **ATT-02** `[M4]` **Clearing:** Enter in that ticket's terminal; an answer from the inbox; or the agent running any
  `crewboard` command.
- **ATT-03** `[M7]` **While the user is looking:** while a ticket's terminal has focus (in the app or via `crewboard attach`),
  that ticket raises **no orange, no inbox item, no OS notification**; only a subtle "your turn" in the header strip. If the user
  leaves the terminal while the agent still waits, it turns orange at that moment.
- **ATT-04** `[M4]` **Working in background** is neutral (calm blue, cpu icon; wakeups use an alarm-clock icon), sorts with
  working cards, never creates inbox items, and counts as working for the agent limit.
- **ATT-05** `[M4]` **Last signal** on every working card ("active 40 s ago"), from the latest hook event, `crewboard` command
  or background activity. It turns into a **warning when stale** (more than 5 min, brief 04) (d0027).

### 12.2 Inbox (d0016, d0024, d0030, d0044, d0045)
- **ATT-06** `[M7]` One ordered queue **across all projects**, items labelled with their project:
  1. Orange: Asks you, Needs you, Idle; plus agent-to-agent questions routed to the inbox (only when "Let finished tickets
     answer questions" is off).
  2. Interrupted, Start interrupted, Resume failed, Parent closed (with Reopen parent / Remove dependency).
  3. Ready for Review (tickets and epics).
  Within a group, tickets that block more tickets first. Item: project, key, title, reason, age, blocked count.
- **ATT-07** `[M7]` **Next-item hotkey** (⌘J / Ctrl+J) opens the next item with its terminal focused. It **doesn't
  auto-advance**: the ticket stays open; the user presses it again. "3 of 7" progress and an "all clear" state.
- **ATT-12** `[M7]` **Answer options** (d0061): when the question has options, the inbox, card and panel number them, mark and
  preselect the recommended one; **1–4** picks, **Enter** sends, **Other…** types free text. The delivered text names the choice
  ("Answer to your question: Postmark (option 1).").
- **ATT-08** `[M7]` Answering from the inbox types the answer into the session (§9.3 rules). For an Interrupted orange ticket
  it **resumes the session and delivers the answer in one click** (d0045).
- **ATT-09** `[M7]` Items that resolve elsewhere (e.g. answered via `crewboard attach`) disappear gracefully. While the plan
  limit banner is active, limit-stopped agents create **no** items (d0046).
- **ATT-10** `[M7]` The inbox count shows in the header and in the project switcher.

### 12.3 OS notifications (d0016)
- **ATT-11** `[M7]` Only for items that **block an agent** (orange, Interrupted). Never for the focused ticket (ATT-03).
  Clicking opens the ticket with its terminal focused. Toggle in Settings (on by default).

---

## 13. Scheduler and limits (d0007, d0016, d0020, d0049)
- **SCH-01** `[M5]` A loop runs on every state change plus a slow tick: select Queued tickets whose dependencies are ready
  (DEP-03) and start them (§9.1), oldest first.
- **SCH-02** `[M5]` **The scheduler never queues tickets by itself**; only the user's drag puts a ticket in Queued (d0016).
- **SCH-03** `[M5]` **Optional limits, OFF by default:** `max_concurrency` (suggested 5) counts Agent Working tickets
  **including orange and background ones**; Paused does not exist; Review tickets don't count. **Back-pressure** (suggested 3):
  no new starts while that many items wait in the inbox. Both only control **starting**; an agent the user unblocks always
  resumes. Adjustable live from the header ("3 / 5 agents").
- **SCH-04** `[M8]` **Plan-limit hold:** while the usage limit is active, nothing starts (d0020).
- **SCH-05** `[M5]` After **Mark merged**, fetch the base before starting the tickets that just became ready (d0049).

---

## 14. Review and delivery

### 14.1 Diff tab (d0017, d0049, d0059)
- **REV-01** `[M6]` Diff against the ticket's **`base_sha`** (where it left its base). Epics: the epic branch vs its base,
  grouped by child. Base indicator, e.g. "diff vs `main` at `a1b2c3d`".
- **REV-02** `[M6]` File list (added/modified/deleted/renamed, counts, "viewed" ticks), **Unified and Side-by-side** views,
  `syntect` highlighting, the agent's summary at the top (collapsible). Large diffs, binary files, renames, empty diffs.
- **REV-03** `[M6]` **Line comments** (add, edit, delete, threads, pending counter). **No partial approval.**

### 14.2 Send back (d0034, d0017)
- **REV-04** `[M6]` Send back bundles the line comments and an optional note, and **moves the ticket straight to Dev**: typed
  as `/crewboard:address-review` into the **same session** (or `claude --resume <id>` as part of the same click if the session
  is gone). If the optional limit is full → Queued. Card marker **"Sent back · N comments · round K"**. Not available on epics.
- **REV-05** `[M6]` The agent fetches feedback with `crewboard review feedback` (in-app comments + unresolved PR comments,
  fetched on demand). PR feedback **never** re-queues a ticket by itself.

### 14.3 Local merge — see GIT-03…GIT-05 `[M6]`
Confirmation shows the target branch and an editable squash message. Success toast; waiting tickets start ("2 waiting
tickets started"). Epic children merge into the epic branch.

### 14.4 Changed since review (d0043)
- **REV-06** `[M6]` On entering Ready for Review, record the branch head. If **new commits** appear before Approve (Local merge,
  Open PR, Push to PR): **"SHOP-18 changed since you reviewed it · 1 new commit"**, a diff of **only the new commits**, and
  **Approve anyway** / **Send back**. Crewboard's own rebases don't count. After a push, compare **content** (not commit IDs).

### 14.5 Uncommitted files (d0050)
- **REV-07** `[M4]` `review submit` refuses while the worktree has uncommitted changes ("2 files are not committed. Commit
  them, or discard them if they're scratch files").
- **REV-08** `[M6]` Local merge, Open PR and Push to PR refuse too: "SHOP-19 has 2 uncommitted files" · **View** · **Send back**.

### 14.6 Open PR (delivery units only) (d0050, d0059)
- **REV-09** `[M10]` **Workflow skills:** Commit, Push, Open PR, each a skill name with a **global default and a per-project
  override** (defaults `/crewboard:commit`, `/crewboard:push`, `/crewboard:open-pr`).
- **REV-10** `[M10]` Flow on **Open PR**:
  1. Run the Open PR skill in the ticket's session (resumed if gone; for an epic, a short-lived delivery session, EPC-08). The
     skill **doesn't push**; it proposes via `crewboard pr propose --stdin`: branch name (following the user's rules, may use
     the External ID), one message per commit (epics: one commit per merged child), PR title and description.
  2. **Confirmation dialog**, everything editable: branch name (validated live: valid ref, not taken locally or on the
     remote), each commit message, PR title, description; target shown. **Push & open PR** / **Cancel**. States: "Open PR skill
     is preparing…", pushing, success (PR #N + link), invalid/taken name.
  3. On confirm, **Crewboard applies the edits itself** (rename branch, reword commits; no tokens, no file changes), then
     tells the skill to push and open the PR. The skill reports `crewboard pr opened <url>`; the card shows "PR #481".
  4. Verify the pushed **content** equals what was reviewed; warn otherwise (REV-06).
- **REV-11** `[M10]` **Provider detection** (no setting): from the remote URL. `github.com` → GitHub (`gh`);
  `dev.azure.com` / `visualstudio.com` → Azure DevOps (`az`). Project settings show it read-only ("GitHub (detected from origin)
  · gh: logged in"). Other remotes: "Not detected: use your own Open PR skill"; Open PR then works only with a custom skill.
- **REV-12** `[M10]` **Auth:** reuse `gh` / `az` logins; store **no tokens**. `GH_TOKEN` / `AZURE_DEVOPS_EXT_PAT` work because
  the CLIs read them (d0017).

### 14.7 After the PR (d0017, d0041)
- **REV-13** `[M10]` Approved · **PR open** with the PR link, **Mark merged** (→ Done; waiting tickets start after a fetch) and
  **Mark closed** (→ Closed · not merged). A note says Crewboard doesn't watch PRs. **No polling, no webhooks.**
- **REV-14** `[M10]` **PR rework:** drag Approved → Ready for Review (PR chip stays), then Send back. The agent never pushes on
  its own. After rework, Approve offers **"Push to PR #481"**: the same dialog with the **branch locked** and only **new**
  commits editable; returns to Approved · PR open. For epics: add or reopen a child; when it's merged the epic returns to
  Review and offers Push to PR.

---

## 15. Closing, reopening, parking, submitting (d0033, d0044, d0052)
- **CLS-01** `[M6]` **Close ticket** from any state (panel "…" menu); confirmation if an agent is running ("it will be stopped
  first"). On close: branch kept; worktree removed unless it has uncommitted changes (then flagged in Cleanup); Run/Shell stop.
- **CLS-02** `[M6]` **Mark closed** on a PR ticket closes it the same way.
- **CLS-03** `[M6]` **Reopen only Closed tickets** (drag out of Done or "Reopen") → Backlog. Merged tickets are locked; fixing
  merged work means a new ticket. Reopening any epic child returns the epic to Backlog.
- **CLS-04** `[M6]` **Closed parent:** never counts as ready. Queued children show "parent SHOP-26 closed"; the inbox item
  offers **Reopen parent** / **Remove dependency**. Crewboard doesn't decide.
- **CLS-05** `[M6]` **Submit for review** (panel "…" menu) when the agent isn't mid-turn (idle, orange, Interrupted): same
  checks as the agent's submit; summary "Submitted by you" + optional note; session stays alive.
- **CLS-06** `[M6]` **Park** (drag Dev → Backlog, confirm "SHOP-25's agent will be stopped. Its branch, files and conversation
  are kept."): stop the agent; keep branch, worktree (incl. uncommitted files) and Claude session ID; free ports; card "Parked ·
  has work"; doesn't count toward the limit. Dragging back to Dev **resumes the same conversation** with new ports ("Resuming
  conversation…").

## 16. Agent-to-agent questions (d0030, d0018 exception, d0043)
- **MSG-01** `[M10]` `crewboard ask <KEY> --stdin` works **only between directly linked tickets** (parent ↔ child). Asking is
  **non-blocking**; the answer is typed into the asker's session when it's idle (§9.3). Every question and answer is logged on
  both tickets (panel **Messages** section; read-only except inbox-routed questions).
- **MSG-02** `[M10]` Target **has a live session** → delivered when its agent is idle and its input line is empty; it answers
  with `crewboard answer`.
- **MSG-03** `[M10]` Target has **no live session but a stored Claude session** (normally a merged parent): **Crewboard resumes
  it automatically to answer** (the one explicit exception to P1, d0018): recreate the worktree **at the same path** on the
  target's kept branch so `claude --resume` finds the conversation; type the question with the **`answer-question`** skill
  (answer only, change nothing, reply with `crewboard answer`); then end the session and remove the temporary worktree. If
  files were changed, keep the worktree and list it in Cleanup. The target card shows **"answering SHOP-40.2"**. The plan-limit
  hold applies.
- **MSG-04** `[M10]` Target **never had a session** → `crewboard ask` is **rejected**: "SHOP-18 hasn't started yet, so there
  is no progress to ask about. Ask the human instead (`crewboard ask human`)." Same rejection if the stored conversation can't
  be resumed ("SHOP-18's conversation is no longer available").
- **MSG-06** `[M10]` The asker's card shows **"asked SHOP-40.1 · waiting"** until the answer is delivered; it ranks below
  every orange status.
- **MSG-05** `[M10]` Per-project setting **"Let finished tickets answer questions"** (on by default). Off → such questions go
  to the inbox ("SHOP-40.2 asks SHOP-40.1: …"), where the user answers.

## 17. Failure and recovery (d0018, d0022, d0045)
- **REC-01** `[M7]` **Startup reconcile:** compare every live ticket with its session and worktree. Alive → reattach silently.
  Gone → by state:
  | State before | After a reboot / service crash / service restart |
  |---|---|
  | Dev · working, background, limit-wait | **Interrupted**; inbox item; Resume / Resume all |
  | Dev · orange | **Interrupted, keeping the label and question**; answering from the inbox resumes and delivers in one click; permission prompts simply ask again after Resume |
  | Ready for Review, Approved | **Not** Interrupted, no inbox item, **not in Resume all**; History notes "session ended (restart)"; resumed only by Send back; questions to them follow §16 |
  | Run and Shell | "stopped by restart"; never restarted automatically |
  **Nothing resumes automatically.** Resume = `claude --resume <id>` in a new session.
- **REC-02** `[M7]` **"Before the restart" view:** an Interrupted ticket's Terminal tab shows the agent's last messages and
  actions, read **read-only from Claude Code's saved conversation** for the stored session ID; a "conversation no longer
  available" variant. Crewboard never copies terminal output to disk.
- **REC-03** `[M7]` **Resume failed** (conversation can't be found): **Retry** or **Start from scratch** (a new `claude`
  session in the same worktree after confirmation; keeps branch, commits and files; stores the new session ID; logs History).
  Never falls back on its own.
- **REC-04** `[M7]` **Cleanup view:** orphaned sessions, worktrees, branches that no ticket owns, leaked ports, and Done/Closed
  tickets whose worktree was kept because of uncommitted changes. Details (size, path, age), checkboxes, confirmation listing
  what will be deleted. **Nothing is deleted automatically.** Empty state "Nothing to clean up".
- **REC-05** `[M6]` **Done:** worktree removed automatically (branch kept), unless it has uncommitted changes; Run stops; Shell
  closes; ports are freed (d0018, d0035).
- **REC-06** `[M7]` After a reboot the app opens with a summary ("4 sessions were interrupted"); after a normal reopen, a quiet
  "reconnected".

## 18. Usage and plan limits (d0020, d0046)
### 18.1 Tokens
- **USG-01** `[M8]` Read Claude Code's own session data **read-only** to show **tokens per card** and **today's total** in the
  header. A format change may only break this display.
- **USG-02** `[M8]` Optional **dollar estimate**, OFF by default (for API-key users).

### 18.2 Plan limit
- **USG-03** `[M8]` **Detection ("it happened"):** the plugin's `StopFailure` hook with `rate_limit` + the limit text →
  agent shows **"Waiting for limit reset"** (neutral, no inbox item) and **one banner** appears.
- **USG-04** `[M8]` **Reset time:** agents launch with Crewboard's **transparent status-line command** (via `--settings`). It
  records `rate_limits.five_hour` / `seven_day` (`used_percentage`, `resets_at`), then runs the **user's own** status-line
  command and prints its output unchanged. Fallback: parse the reset time from the on-screen limit text. Else "resets at an
  unknown time".
- **USG-05** `[M8]` **Plan meter** in the header: "5h · 82% · week · 40%", reset times on hover, amber near the limit;
  Pro/Max only, hidden for API-key users (who get Claude Code's automatic retries).
- **USG-06** `[M8]` **Banner:** "Usage limit reached · resets at 3:45 pm · no new tickets will start" (session / weekly /
  unknown variants, countdown). No new starts while active (SCH-04). After the reset, the user clicks **Resume all**, which
  tells the limit-stopped sessions to continue (automatic delivery rules, §9.3). Nothing restarts on its own.
- **USG-08** `[M8]` **Enterprise plans** get no special banner in v0.1: their limit shows the "reset time unknown" variant,
  and Resume all serves as the retry (d0020). The enterprise frames in `11 · Banners…` are labelled "after v0.1".
- **USG-07** `[M8]` **Safety net:** any Idle inbox item has **"This is the usage limit"**; one click folds the Idle items into the
  banner and marks those agents "Waiting for limit reset".

## 19. Run, Shell, Project terminal (d0019, d0026, d0035)
- **RUN-01** `[M9]` **Run** starts the project's **run command** in the ticket's worktree with `$PORT` = the ticket's **Run
  port**; **click-only**; Stop; stops at Done (toast). The **Run tab** shows the live output log. States: not started, starting,
  running (`▶ :4020`), stopped, failed to start (log + "edit command"), port in use.
- **RUN-02** `[M9]` First Run in a project asks for the run command, with **Ask Claude** (Claude looks at the repo and suggests,
  the user confirms or edits); saved in project settings. **No auto-detection rules** (d0023).
- **RUN-03** `[M9]` **Open URL** only if the project sets one (e.g. `http://localhost:$PORT/swagger`); opens the system browser.
  No proxy, no embedded browser, no auto-start.
- **RUN-04** `[M9]` A list of all running Run processes ("▶ 3 processes" in the header) to stop them.
- **RUN-05** `[M9]` **Shell tab:** a plain terminal in the ticket's worktree with `$PORT` = Run port and the ticket's env;
  owned by the service; closes at Done.
- **RUN-06** `[M9]` **Project terminal** (⇧⌘T / Ctrl+Shift+T): a **plain terminal in the project's own folder** (the user's
  repo checkout); doesn't start Claude; `crewboard` on PATH with full access; the user can run `claude` there themselves;
  owned by the service, survives closing the window. Changes made there are the user's own.

## 20. Settings (d0003, d0016, d0020, d0023, d0030, d0040, d0050, d0059)
| Global | Per project |
|---|---|
| Agent limits (off; max 5; back-pressure 3) | Name, key (read-only), repo path |
| Agent instructions (default text) | **Base branch** (prefilled from the remote default) |
| Workflow skills: Commit / Push / Open PR | Agent instructions override (empty = global) |
| Usage: dollar estimate (off) | Workflow skills override (empty = global) |
| Notifications (on) | Let finished tickets answer questions (on) |
| Data: database path + "Reveal", logs path, backup note | Setup command · Run command (Ask Claude) · Open URL (optional) · Env files to copy |
| Appearance (light/dark/system), terminal font size | PR provider (read-only, detected) + login status |
| Tools checklist (git, claude, gh, az + `azure-devops` ext.) with Re-check | Remove project (confirmation) |

- **SET-01** `[M2]` All settings live in the app's database; **nothing is written into repos** (d0023). Invalid values (e.g. a
  missing env file) are flagged.
- **SET-02** `[M1]` **First-run checklist** (d0021, d0017): git and `claude` required; `gh` / `az` (+ extension) optional with
  login status; copyable install command per OS; Crewboard never installs anything. **Permission-mode warning** if the user's
  Claude Code mode prompts: "With 5–10 agents, permission prompts will stop agents often", links to allowlists and auto mode;
  warning only (d0011, d0016).

## 21. User interface (crewboard.pen)

### 21.1 Frame map (d0057)
| Area | Frame(s) in `crewboard.pen` | Brief |
|---|---|---|
| Tokens, status legend, type, icons | 01 · Foundations | 01 |
| Components (buttons, pills, chips, kbd, toast, banner, Ticket Card, Epic Card), keyboard map | 01 · Components | 01 |
| Window, header (inbox, tokens, plan meter, limits, processes), project switcher | 02 · App shell & project switcher | 02 |
| Board | 03 · Board — shop-web · 1440 · Light / Dark; 03 · Board & columns | 03 |
| Cards | 04 · Ticket card variants · Light / Dark | 04 |
| Side panel, actions, Stack list, Messages, History | 05 · Ticket side panel; 05/06 · Panel — SHOP-22 Needs you · Terminal · Light / Dark | 05 |
| Terminal tab | 06 · Terminal tab | 06 |
| Diff, review, approve, send back, Open PR dialog | 07 · Diff, review, approve & send back | 07 |
| Ticket editor | 08 · Ticket editor | 08 |
| Inbox | 10 · Needs-you inbox | 10 |
| Banners, toasts, OS notifications | 11 · Banners, toasts & OS notifications | 11 |
| Dependencies storyboard | 12 · Stack & dependencies | 12 |
| Run & Shell | 13 · Run & Shell | 13 |
| First run | 14 · First run & onboarding | 14 |
| Settings | 15 · Settings | 15 |
| Recovery & Cleanup | 16 · Recovery & cleanup | 16 |
| Project terminal | 17 · Project terminal | 17 |

Pending design updates: none. Updates A–I (`design-briefs/CHANGES-*.md`) are applied in `crewboard.pen` (checked 2026-10-09).

### 21.2 Visual rules (brief 01)
- Calm, dense, developer-grade; custom-drawn GPUI (flat fills, borders, radius, simple shadows; no blur). Same look on all OSes.
- **Needs the user** (orange family, Interrupted, Start interrupted, Parent closed) stands out; **working** (incl. background,
  queued) stays calm; **review/PR** in between; **Done** fades. Colour is never the only signal (always icon + label).
- No spinners for local data. Every main action has a shortcut. Light and dark.

### 21.3 Keyboard map (d0058)
No shortcut may appear twice across the three columns; OS system shortcuts are off-limits.

| Action | macOS | Windows / Linux | In terminal (Win/Linux) |
|---|---|---|---|
| Next inbox item | ⌘J | Ctrl+J | Ctrl+Shift+J |
| Open inbox | ⇧⌘I | Ctrl+Shift+I | Ctrl+Shift+I |
| Previous / next ticket in panel | ⌘[ / ⌘] | Ctrl+[ / ] | Ctrl+Shift+[ / ] |
| Leave terminal focus | ⇧⌘` | Ctrl+Shift+` | Ctrl+Shift+` |
| Search / filter | ⌘K | Ctrl+K | Ctrl+Shift+K |
| Switch project | ⌘P | Ctrl+P | Ctrl+Shift+P |
| Project 1–9 | ⌘1–9 | Ctrl+1–9 | Ctrl+Shift+1–9 |
| New ticket | ⌘N | Ctrl+N | Ctrl+Shift+N |
| Project terminal | ⇧⌘T | Ctrl+Shift+T | Ctrl+Shift+T |
| Settings | ⌘, | Ctrl+, | Ctrl+Shift+, |
| Open / close panel | ↵ / Esc | ↵ / Esc | — |
| Expand panel | ⇧⌘E | Ctrl+Shift+E | Ctrl+Shift+E |
| Switch tab | ⌘⌥1–6 | Ctrl+Alt+1–6 | Ctrl+Shift+Alt+1–6 |
| Approve ▾ | ⌘↵ | Ctrl+↵ | Ctrl+Shift+↵ |
| Send back | ⌘⌫ | Ctrl+⌫ | Ctrl+Shift+⌫ |
| Start / move to Dev | ⇧⌘D | Ctrl+Shift+D | Ctrl+Shift+D |
| Find in terminal scrollback | ⌘F | — | Ctrl+Shift+F |

### 21.4 Screens (requirements)
- **UI-01** `[M2]` **App shell:** header (project name, base branch, inbox button + count, tokens today, plan meter, optional
  limits control, "N running · ▶ N processes", New ticket, Project terminal, Settings), project switcher (name · key, agents
  working, items needing the user; quick switcher; Add project; quiet idle projects), board, side-panel slot, banner area.
- **UI-02** `[M2]` **Board:** 5 columns with counts and hints; collapsible Done rail; drag and drop with allowed/forbidden
  feedback (§7.4); approve-choice popover on drop into Approved; chain highlight; search/filter (text, status, "needs me", epic);
  empty states; very full columns scroll.
- **UI-03** `[M2]` **Ticket card:** key, title (2 lines), status icon + label, dependency badges, last signal, tokens, flags
  (§7.2), hover quick actions; full variant set per frame 04 with a defined flag priority.
- **UI-04** `[M3]` **Side panel:** resize, expand (⇧⌘E), next/previous; header (key, title, status, project, branch copyable,
  base, tokens, last signal); context action bar per state (brief 05); tabs **Terminal · Diff · Run · Shell · Ticket · History**
  with indicators; Stack list; Messages; History lists sessions with time, duration, tokens and **Claude session IDs**.
- **UI-05** `[M2]` **Ticket editor:** quick-create in Backlog; full form; Done-when checklist editing; Depends-on picker (same
  project, shows statuses, cycle and cross-base errors, lists epics); External ID; base-branch field; agent-edit markers.
- **UI-06** `[M7]` **Inbox** surface reachable from anywhere (⇧⌘I), groups with counts, Resume all on the interrupted group.
- **UI-07** `[M1]` **Banners and toasts:** one banner per situation (usage limit, reconnecting, service stopped, service update
  ready/restart required, permission-mode warning) with stacking rules; toasts for completed actions; inline errors (`gh` not
  logged in with `gh auth login`, merge conflict, setup failed with Retry link).
- **UI-08** `[M1]` **Onboarding:** welcome, tools checklist, add first project (folder picker, base branch detected, key field
  "Used in ticket IDs like SHOP-18. Can't be changed later."), empty board.

---

## 22. Platforms (d0021, d0022, d0027)
- **PLT-01** `[M0–M10]` macOS first: Unix domain socket, `portable-pty`.
- **PLT-02** `[M11]` Windows: ConPTY via `portable-pty`, named pipe with an owner-only ACL, Ctrl+Shift app shortcuts in the
  terminal. Linux: Unix socket. The same checks run in local Windows and Linux VMs (§23.2).
- **PLT-03** Unsigned builds; the OS "unverified developer" warning is accepted. No claude CLI version checks.

## 23. Quality (d0056)
### 23.1 Tests
- Core tested without a UI: readiness, sort orders, hook-to-status mapping, permission scope, delivery rules, reconcile table.
- **Git tests against temporary repositories:** worktrees, branch names, fetch + fast-forward + divergence, Local merge (clean,
  conflict, dirty checkout refusal), checkpoint commit, epic integration branch, reopen rebase, changed-since-review.
- **No fake `claude`.** Hook handling is tested by feeding `crewboard hook <event>` real-shaped JSON; PTY and session plumbing
  with an ordinary command in place of `claude`. Real `claude` behaviour is checked by hand.
- UI logic in testable view models; screens checked by hand against the named `crewboard.pen` frame.

### 23.2 Checks (d0056)
One local script, `scripts/check.sh`: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, run before every commit.
The repository has no remote yet, so there are no GitHub Actions. macOS from M0; Windows and Linux at the end of v0.1 (M11),
in local virtual machines. GitHub Actions are added once the repository has a remote.

### 23.3 Definition of done (every ticket)
- Checks green (`scripts/check.sh`); new behaviour has tests; the ticket's "Done when" items are met.
- The commit names the decisions and requirement IDs it implements.
- A conflict with a decision → stop and ask the user.

## 24. Milestones (d0053)
macOS only until M11. All of v0.1 is built outside Crewboard; daily use starts with the work after v0.1 (d0062).

| # | Milestone | Requirements |
|---|---|---|
| **M0** | Foundations | ARC-01, ARC-02; SQLite schema + migrations (§8); `LICENSE` (MIT); local check script (§23.2) |
| **M1** | Background service | SVC-01…04, SVC-07, ARC-03; protocol (§5.4); SET-02; UI-07 (service banners); UI-08 |
| **M2** | Board | PRJ-01…05, TKT-01…08, DEP-01, DEP-02, DEP-04, DEP-05, EPC-01, EPC-06, EPC-07, §7 states/transitions/drag/order, SET-01, UI-01…03, UI-05 |
| **M3** | Sessions | SES-01…07, SES-09…11, SVC-05, SVC-06, EPC-02, GIT-07, UI-04 |
| **M4** | Agent signals | PLG-01…06, §11.2 commands (except `pr …`, `ask <KEY>`, `answer`), CLI-01…04, §11.4, ATT-01, ATT-02, ATT-04, ATT-05, SES-08, GIT-08, REV-07 |
| **M5** | Scheduler | SCH-01…03, SCH-05, DEP-03 |
| **M6** | Review & Local merge | REV-01…06, REV-08, GIT-01…06, EPC-03…05, CLS-01…06, REC-05 |
| **M7** | Attention & recovery | ATT-03, ATT-06…12, REC-01…04, REC-06, UI-06 |
| **M8** | Usage | USG-01…08, SCH-04 |
| **M9** | Run, Shell, Project terminal | RUN-01…06 |
| **M10** | PR flow & agent questions | REV-09…14, EPC-08, MSG-01…06, `crewboard pr …`, `branch rename`, `ask <KEY>`, `answer`, `questions` |
| **M11** | Windows & Linux | PLT-02, checks in Windows and Linux VMs (§23.2) |

## 25. Open items (not decided; don't implement)
- Package channels per platform (d0021). Trademark check for "Crewboard" before any public launch (d0028).
- 0014's pitch item 1 ("dependencies and stacking") must be revisited before a public launch (d0049).
- Stacking as a later per-project setting (d0005, d0049).
- Enterprise-plan limit banners ("Request more", Retry) (d0020).

## 26. Implementation choices (implementer decides and documents)
Port pool range and allocation (must skip ports in use); scrollback size; protocol message names and framing; database file
name; log rotation size; exact stale-signal threshold if 5 min proves wrong (brief 04); the plugin-loading flag (verify,
d0009); hook payload fields not documented (verify with real payloads, d0020, d0042).

## 27. Decision index (traceability)
| Decision | Where in this document |
|---|---|
| 0001 GPUI, 0002 core/workspace, 0055 baseline, 0057 repo | §5 |
| 0003 SQLite | §8 |
| 0004, 0033, 0034, 0036, 0037, 0047, 0052 board & lifecycle | §7, §15 |
| 0005 (deferred), 0006, 0049, 0051, 0059 dependencies, bases, epics | §6.3, §6.4, §10 |
| 0007, 0016 scheduler & limits | §13 |
| 0008/0022 sessions & service | §5.3, §9 |
| 0009, 0031, 0032, 0040 plugin & CLI | §11 |
| 0010, 0026, 0035, 0019 worktrees, ports, Run, Shell, Project terminal | §9, §19 |
| 0011, 0012, 0018, 0023 principles | §2 |
| 0013/0053 milestones, 0025 personal build, 0054 MIT, 0062 built outside Crewboard | §0, §1, §3, §24 |
| 0014 user | §1 |
| 0015, 0017, 0038, 0041, 0043, 0050 review & delivery | §10, §14 |
| 0020, 0046 usage & limits | §18 |
| 0021, 0027 platforms & safeguards | §5.3, §22 |
| 0024, 0029 projects & keys | §6.1, §6.2 |
| 0028 name | §5.1 |
| 0030 agent questions | §16 |
| 0039, 0044 epics, closing; 0060 delete | §6.2, §6.4, §15 |
| 0042, 0045 signals, reboot | §11.4, §12, §17 |
| 0048 typing into sessions | §9.3, §9.4 |
| 0060 delete, 0061 answer options | §6.2, §11.2, §12.2 |
| 0056 quality | §23 |
| 0058 shortcuts | §21.3 |
