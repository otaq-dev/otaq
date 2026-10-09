# Crewboard: design briefs

These files are the **design requests** for Crewboard. Each one covers one area of the product and can be
handed to a design agent on its own, together with this README and `01-design-system.md`.

The goal is **design exploration** (Figma-style screens, components and states), not production-ready
assets. The designs will be used later as the reference when the app is built.

## How to use

1. Give the design agent this README, `01-design-system.md`, and **one** brief file (02 to 16).
2. Paste the prompt below and fill in the brief's file name.
3. Start with `01-design-system.md` itself, so every later brief reuses the same foundations.

### Prompt to give the design agent

```text
You are the product designer for Crewboard, a native desktop app (macOS, Windows, Linux) that lets one
developer run 5–10 Claude Code agents in parallel from a kanban board.

Read these files first:
  1. design-briefs/00-README.md         (product, principles, constraints, sample data, deliverables)
  2. design-briefs/01-design-system.md  (foundations every screen must reuse)
Then design everything in:  design-briefs/<BRIEF FILE NAME>

Rules:
- Design EVERY screen, component, state and variant listed in the brief. The state lists are the most
  important part. Do not skip "rare" states such as empty, error, disconnected, paused or interrupted.
- Use the shared sample data from 00-README.md, so all briefs show the same projects and tickets.
- Deliver light and dark versions of each main screen.
- Annotate interactions: what happens on click, hover, drag, keyboard shortcut and right-click.
- Respect the constraints in 00-README.md (custom-drawn native UI, no web views, keyboard-first).
- Do NOT invent features that are not in the briefs. If something seems missing or contradictory,
  list it under "Open questions" at the end instead of designing it silently.
- Output: a Figma-style file (or .pen / exported frames) with one page per brief section, plus a short
  written summary of key design choices and open questions.
```

## The product in one paragraph

Crewboard is a native kanban board that drives **Claude Code** agents. You create tickets in Backlog, then
drag one to **Dev** (the agent investigates and builds in one session, 0036). Crewboard starts a
real interactive `claude` session for that ticket in its own git worktree. You can watch it and type into it
at any time through an embedded terminal. When the agent is done, the ticket moves to **Ready for Review**.
You review the diff and either approve it (Local merge, or Open PR on GitHub or Azure DevOps) or send it
back. Tickets can depend on each other: like beads, a ticket starts only when all its parents are merged
(0049). The user runs **5–10 agents at once across several projects**, so the UI is mostly about
**attention**: what needs me right now?

## Design principles (derived from the decision log)

| Principle | What it means for the UI | Decisions |
|---|---|---|
| **The user is in control** | Anything that starts an agent (spends tokens) or deletes something needs a visible button and a click. Nothing restarts, retries or cleans up by itself. Use buttons and confirmations, not silent automation. | 0017, 0018 |
| **Attention is the scarce resource** | With 5–10 agents, "what needs me?" must be answerable at a glance: the inbox, badges, "last signal" times. Avoid noise: one banner instead of 8 alerts. | 0016, 0020, 0027 |
| **Native, fast, keyboard-first** | Dense but calm. Every main action has a shortcut. No loading spinners for local data. | 0001, 0014 |
| **The terminal is a first-class citizen** | The live Claude Code terminal is the main way to see and steer an agent. It is not a hidden debug view. | 0008, 0022 |
| **Developer audience** | Show real details: branch names, ports, commit SHAs, token counts. Don't over-explain. | 0014, 0021 |
| **Low maintenance, few moving parts** | Prefer simple components that are cheap to build and maintain. No decorative animation systems. | 0012, 0023 |
| **Never touch user repos** | No UI that suggests writing config files into a repo. All settings live in the app. | 0023 |

## Technical constraints for the designs

- **Built with GPUI** (the Rust UI framework behind the Zed editor). Everything is **custom-drawn**: there
  are no native OS widgets and no HTML. Anything is possible, but every custom component costs build time.
  - Fine: flat colors, borders, rounded rectangles, simple shadows, icons, monospace text, subtle hover
    and press states, simple fades and slides.
  - Avoid: glassmorphism and background blur, heavy gradients, complex physics animations, web-only
    patterns (embedded web pages, CSS-like effects).
- **No embedded browser.** If a project sets an Open URL, it opens in the user's normal browser. Many projects are
  backends, CLIs or libraries with no URL at all (0035).
- **Cross-platform, one look.** The same design is used on macOS, Windows and Linux. Show the window controls
  for macOS (traffic lights, top-left) and Windows (caption buttons, top-right) on the app-shell brief only.
- **Desktop window sizes:** design at **1440×900**, check at the **1280×800** minimum, and show the board at
  **1920×1080** as well.
- **The terminal** renders a monospace character grid with ANSI colors (16, 256 and truecolor), a cursor,
  selection and scrollback.
- **Fonts are bundled** with the app: one UI sans-serif and one monospace (suggest specific open-license fonts).
- **Graph view (boxes and arrows) is NOT in v0.1** (0006). Do not design it, except as an optional extra.

## Vocabulary: columns and statuses

**Columns** (0004, 0036): Backlog → **Dev** (Queued + Agent Working) →
**Ready for Review** → **Approved** → **Done**. Questions and permission prompts stay in Dev, highlighted at the top (0037). **Send back** is an action, not a column: it returns the
ticket to Dev with a "Sent back" marker (0034).

"Dragging to Dev" means dropping a ticket into Queued: it starts as soon as its dependencies allow.

**Statuses** (shown on cards, not as columns):

| Status | Meaning |
|---|---|
| **Needs you** | The agent is blocked on a permission prompt or is waiting idle for the user. |
| **Interrupted** | The session died (reboot or crash) and waits for Resume (0018). |
| **Start interrupted** | Starting the ticket stopped halfway, so the card offers Retry (0018). |
| **Closed · not merged** | Closed without merging (Mark closed, or Close ticket from any state). Shown inside Done and reopenable (0033). |
| **Parent closed** | A child whose parent was closed. It can't be approved until the parent is reopened or the dependency is removed (0033). |
| **PR open** | Approved via Open PR, and waiting for the user to click Mark merged or Mark closed (0017). |
| **Running** | The project's run command is running for this ticket on its port (Run, 0035). |
| **Asked / waiting for answer** | The agent asked a linked ticket's agent a question through Crewboard (0030). |
| **Last signal** | "agent last active 3 min ago" on working cards (0027). |

## Shared sample data (use this in every brief)

**Projects:** `shop-web` (key `SHOP`, Next.js, GitHub), `billing-api` (key `BILL`, .NET, Azure DevOps, base `develop`), `crewboard` (key `CREW`, Rust, idle; in the "PR provider not detected" sample its origin is `git.acme.internal/web/crewboard`).
Tickets are identified as `KEY-number`, for example SHOP-18 (0029).
**Usage today:** 1.84M tokens. The plan limit is not reached, except in the banner states.

`shop-web` tickets:

| Key | Title | Column / status | Details |
|---|---|---|---|
| SHOP-12 | Add dark mode toggle | Dev · **Asks you** | The agent explored the code first ("colors are hard-coded in 14 files") and asks: "Should dark mode follow the system setting?" |
| SHOP-17 | Add Stripe client | Done | Merged locally |
| SHOP-18 | Checkout API | Ready for Review | 9 files, +412 −38, temporary branch `crewboard/SHOP-18` (real name chosen at Open PR, 0050) |
| SHOP-19 | Price table | Agent Working | Last signal 40 s ago, 182k tokens, agent port 4019, Run port 4020 (0026) |
| SHOP-20 | Checkout page | Queued | Depends on SHOP-18 and SHOP-19: "waits for SHOP-18, SHOP-19 to merge" (0049) |
| SHOP-21 | Order confirmation email | Queued | Waiting on SHOP-20 |
| SHOP-22 | Fix flaky login redirect | Agent Working + **Needs you** | Permission prompt: "Allow `rm -rf .next`?" |
| SHOP-23 | Split user full name | Dev · **Asks you** | Agent asked (`crewboard ask human`): "Should `fullName` be split into first and last name everywhere, or only in the profile form?" |
| SHOP-24 | Update footer links | Backlog | No "Done when" list (warning case) |
| SHOP-25 | Upgrade to Next.js 15 | Agent Working + **Interrupted** | Session lost after reboot |
| SHOP-26 | Add sitemap | Approved + **PR open** | GitHub PR #481 |
| SHOP-40 | Wishlist (epic) | Backlog | External ID **PAY-123**; children SHOP-40.1–40.3 merge into the epic's local branch; delivered as one PR (0059); in the PR-open sample it is PR #490 |
| SHOP-31 | Hotfix: checkout crash | Backlog | Base branch `release/2.3` instead of the project default `main` (0051) |
| BILL-6 | Late fee rule | Queued (held) | `billing-api` base `develop` has diverged from `origin/develop`: "Push or pull, then retry" (0059) |
| SHOP-27 | Search results paging | Agent Working + **Sent back** | "Sent back · 3 comments · round 2" |
| SHOP-28 | Cart badge count | Queued + **Start interrupted** | Setup command failed halfway |

**Epic (0039):** `SHOP-40 Wishlist` (in Backlog, 1 / 3 done) with children **SHOP-40.1** Wishlist API (Done ·
Merged), **SHOP-40.2** Wishlist button (Dev · Agent Working, depends on SHOP-40.1), and **SHOP-40.3** Wishlist page (Backlog).

`billing-api` tickets: BILL-4 Invoice PDF export (Agent Working, last signal 4 min ago), BILL-5 VAT rounding fix
(Ready for Review, PR route: Azure DevOps).

## Brief index

| File | Area |
|---|---|
| `01-design-system.md` | Foundations: color, type, spacing, icons, status language, shortcuts |
| `02-app-shell-and-project-switcher.md` | Window layout, header, project switcher |
| `03-board-and-columns.md` | The board, columns, drag and drop |
| `04-ticket-card.md` | Ticket card anatomy and every card state |
| `05-ticket-side-panel.md` | What happens when you click a card: the panel, tabs and actions |
| `06-terminal-tab.md` | Embedded live Claude Code terminal |
| `07-diff-and-review.md` | Diff tab, line comments, Approve and Send back flows |
| `08-ticket-editor.md` | Create and edit tickets, the form and warnings |
| ~~`09-investigation.md`~~ | **Removed by decision 0036.** There is no Investigate step. Don't design it. |
| `10-needs-you-inbox.md` | The global inbox and the next-item flow |
| `11-banners-and-notifications.md` | Usage-limit banner, warnings, toasts, OS notifications |
| `12-stack-and-dependencies.md` | Stack list, chain highlight, and how chains wait for merges (0049) |
| `13-previews.md` | Run (any stack), optional Open URL, the Shell tab, and the run-command dialog (0035) |
| `14-first-run-and-onboarding.md` | First launch, tool checklist, adding a project |
| `15-settings.md` | Global and per-project settings |
| `16-recovery-and-cleanup.md` | Interrupted sessions, Resume all, the Cleanup view |
| `17-project-terminal.md` | The Project terminal, where Claude creates tickets |

The source of truth for behavior is `../decisions/`. If a brief and a decision disagree, the decision wins.
Flag it as an open question.
