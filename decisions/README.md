# Decision Log

This folder is the **source of truth** for product and architecture decisions in **Otaq**.
The product was called "Crewboard" until [0076](0076-product-name-otaq.md); the decisions are reworded to the new name.
Decisions before [0028](0028-product-name.md) use the working name "factory". Dated records outside this folder may still
say Crewboard, and "orange" for the needs-you family (now turquoise, 0077).
`DESIGN.md` is the narrative overview. If the two disagree, this folder wins and `DESIGN.md` gets fixed.

## Rules (for humans and agents)

1. **One decision per file**: `NNNN-short-slug.md`. Numbers are never reused or renumbered.
2. **Status** is one of `Proposed`, `Accepted`, or `Superseded by NNNN`.
3. **Check before acting.** Before you act on any product or architecture instruction, check it against
   the accepted decisions here.
4. **Conflicts must be raised, not silently resolved.** If a new instruction conflicts with an accepted
   decision, point it out to the user, quoting the decision number and the conflicting text, and confirm
   the change before you write any code. Then:
   - **Refinement** (same direction, more detail): edit the file in place and add a line to its `Changelog`.
   - **Reversal or replacement**: write a new decision file, set the old one to `Superseded by NNNN`,
     and link the two.
5. Keep the index below and `DESIGN.md` in sync with every change.

## Template

```markdown
# NNNN. Title

- **Status:** Accepted
- **Date:** YYYY-MM-DD
- **Related:** NNNN, NNNN

## Context
Why this needed deciding.

## Decision
What we decided.

## Alternatives considered
What we rejected, and why.

## Consequences
What this makes easier or harder, and any follow-ups.

## Changelog
- YYYY-MM-DD: created
```

## Index

| # | Decision | Status |
|---|---|---|
| [0001](0001-native-app-gpui.md) | Native desktop app, UI built with GPUI | Superseded by 0074 |
| [0002](0002-rust-core-ui-agnostic.md) | UI-agnostic Rust core, Cargo workspace layout | Accepted |
| [0003](0003-sqlite-storage.md) | SQLite storage; beads only as an import adapter | Accepted |
| [0004](0004-board-columns-state-machine.md) | Board columns and ticket state machine | Accepted |
| [0005](0005-stacked-dependencies.md) | Dependency DAG with stacked branches | Accepted (stacked mode deferred for v0.1 by 0049) |
| [0006](0006-dependency-visualization.md) | Dependency visualization: badges, chain highlight, Stack list (graph view maybe later) | Accepted |
| [0007](0007-scheduler-loop.md) | Scheduler loop with a concurrency limit | Accepted |
| [0008](0008-claude-cli-in-tmux.md) | Interactive Claude Code CLI in tmux, embedded terminal | Superseded by 0022 |
| [0009](0009-factory-claude-plugin.md) | Agent behavior packaged as a Claude Code plugin | Accepted (MCP part superseded by 0031) |
| [0010](0010-worktrees-and-previews.md) | A worktree per ticket, with per-ticket browser previews | Accepted |
| [0011](0011-inherit-claude-permission-mode.md) | Agents inherit the user's Claude Code permission mode | Accepted |
| [0012](0012-open-source-flexibility.md) | Built as a possible open-source product, so flexibility comes first | Accepted |
| [0013](0013-release-order-v0.1.md) | Release order: v0.1 is the usable core loop, including dependencies | Accepted (milestones superseded by 0053) |
| [0014](0014-target-user-and-positioning.md) | Target user (solo dev, 5–10 agents) and positioning | Accepted |
| [0015](0015-approve-actions.md) | Two approve actions: Local merge and PR (GitHub / Azure DevOps) | Accepted |
| [0016](0016-needs-you-inbox-and-concurrency.md) | "Needs you" inbox; optional concurrency limit and back-pressure (off by default) | Accepted |
| [0017](0017-review-flow.md) | Review flow: CLI auth, Diff tab (unified + side-by-side), manual PR state (no polling) | Accepted |
| [0018](0018-failure-and-recovery.md) | Failure and recovery: detect and report, clicks for tokens and deletes | Accepted ("no test gate" superseded by 0063) |
| [0019](0019-ticket-authoring-and-investigation.md) | Ticket form, Claude ticket tools, investigation step | Accepted (investigation superseded by 0036, Done when by 0067) |
| [0020](0020-usage-visibility-and-limits.md) | Usage visibility (tokens, optional $) and a single plan-limit banner | Accepted |
| [0021](0021-platform-and-distribution.md) | Platforms: macOS, Windows, Linux at launch; no signing; no CLI version checks | Accepted |
| [0022](0022-background-service-owns-sessions.md) | A background service owns agent sessions (replaces tmux) | Accepted |
| [0023](0023-no-files-in-user-repos.md) | Zero-config start; never add files to user repos; settings live in the app | Accepted |
| [0024](0024-multi-project-boards.md) | Multiple projects (one repo each), a board per project; inbox, limits and usage are global | Accepted |
| [0025](0025-personal-build-first.md) | v0.1 is a personal build: no test-repo choice, no finish line | Accepted |
| [0026](0026-click-only-previews-and-ports.md) | Click-only previews, no proxy; a unique `$PORT` per ticket in v0.1 | Accepted (preview part superseded by 0035) |
| [0027](0027-build-order-and-safeguards.md) | Build macOS and Local merge first; "last signal" on cards; socket locked to the user | Accepted |
| [0028](0028-product-name.md) | Product name (replaces the working name "factory"); first Crewboard | Superseded by 0076 |
| [0029](0029-ticket-keys.md) | Ticket keys: project key + number (SHOP-18), never reused | Accepted |
| [0030](0030-agent-to-agent-questions.md) | Agents ask each other via Otaq (`ask_ticket`), linked tickets only, all logged | Accepted |
| [0031](0031-cli-and-skills-instead-of-mcp.md) | Agents use the `otaq` CLI through skills; no MCP server | Accepted |
| [0032](0032-cli-modeled-on-beads.md) | The `otaq` CLI follows beads (`bd`) conventions | Accepted |
| [0033](0033-closed-tickets.md) | Closed tickets: inside Done, closable from any state, reopenable | Accepted |
| [0034](0034-send-back-returns-to-dev.md) | No Changes Requested column: Send back returns the ticket to Dev | Accepted |
| [0035](0035-run-and-shell-instead-of-browser-preview.md) | "Run" (any stack) + optional Open URL + Shell tab replace the browser Preview | Accepted |
| [0036](0036-no-investigate-no-ready-for-dev.md) | Remove Investigate and Ready for Dev: Backlog goes straight to Dev | Accepted |
| [0037](0037-no-needs-input-column.md) | No Needs Input column: tickets needing you stay in Dev, highlighted at the top | Accepted |
| [0038](0038-local-merge-when-main-moved.md) | Local merge when main moved: free rebase on click, early conflict flag, never touch user edits | Accepted |
| [0039](0039-epics.md) | Epics: Backlog-only containers, children SHOP-40.1…, auto-Done when all children are Done | Accepted (partly superseded by 0059) |
| [0040](0040-agent-instructions.md) | "Agent instructions" (global + per project) shown via `otaq prime`; your rules for agents | Accepted |
| [0041](0041-pr-rework-flow.md) | PR rework: drag Approved → Review, Send back; user pushes every PR update | Accepted |
| [0042](0042-waiting-signals.md) | Needs-you family (was "orange") Asks you / Needs you / Idle; cleared by your input; counts toward the optional limit | Accepted (colors refined by 0077) |
| [0043](0043-changed-since-review-check.md) | Approve warns "changed since you reviewed it" and shows only new commits | Accepted |
| [0044](0044-closing-and-reopening-rules.md) | Closing parents and epics, grouping into epics later, reopening only Closed tickets | Accepted |
| [0045](0045-reboot-coverage.md) | Reboot coverage by state; "Before the restart" view from the stored Claude session ID | Accepted |
| [0046](0046-housekeeping-run-epic-deps-limit-wait.md) | Run and Shell in v0.1; a ticket can depend on an epic; "Waiting for limit reset" | Accepted |
| [0047](0047-pinned-tickets.md) | Pinned tickets stay at the top of their column (above the needs-you ones), pin follows the ticket | Accepted |
| [0048](0048-typing-into-live-sessions.md) | Typing into live sessions: automatic messages wait for an empty input line; your clicks ask before clearing unsent text | Accepted |
| [0049](0049-beads-style-dependencies-first.md) | v0.1 dependencies work like beads: wait until every parent is merged; stacking later as a per-project setting | Accepted |
| [0050](0050-workflow-skills-commit-push-pr.md) | Workflow skills (Commit, Push, Open PR) configurable in settings; agent commits; branch named at first push; confirmation dialog before pushing | Accepted |
| [0051](0051-per-ticket-base-branch.md) | Each ticket has a base branch: project default, per-ticket override; linked tickets share a base | Accepted |
| [0052](0052-submit-yourself-and-park.md) | "Submit for review" by the user; "Park" (Dev → Backlog) keeps the work and resumes the same conversation | Accepted |
| [0053](0053-v0.1-milestones-and-dogfooding.md) | v0.1 milestones M0–M14; Otaq builds itself once usable | Accepted (dogfooding superseded by 0062) |
| [0054](0054-mit-license.md) | License: MIT | Accepted |
| [0055](0055-technical-baseline.md) | Technical baseline: JSON socket protocol, `~/.otaq/`, git CLI, crates, no login auto-start, logs | Accepted |
| [0056](0056-quality-bar-for-tickets.md) | Quality bar: core tests without UI, git tests on temp repos, no fake claude, local checks (no GitHub Actions yet), definition of done | Accepted |
| [0057](0057-repository-layout-and-design-source.md) | This folder is the code repo; the design lives only in pen.dev (`crewboard.pen`) | Accepted |
| [0058](0058-keyboard-shortcuts.md) | Keyboard shortcuts: full map; no duplicates across macOS, Win/Linux and in-terminal columns | Accepted |
| [0059](0059-epics-as-delivery-units.md) | Epics are delivery units: children merge locally into the epic branch; one reviewed PR per epic; External ID; base prefilled from the remote | Accepted |
| [0060](0060-delete-never-started-tickets.md) | Delete only tickets that never went to Dev (confirmation, links removed, number never reused) | Accepted |
| [0061](0061-answer-options-for-human-questions.md) | Questions to the user can carry 2–4 options (recommended preselected, "Other…" always) | Accepted |
| [0062](0062-build-v0.1-outside-otaq.md) | v0.1 is built outside Otaq (runbook; no review stop between milestones, only blockers stop, other questions deferred); Otaq is used from the work after v0.1 | Accepted (refined by 0079: outside Otaq until the personal build installs) |
| [0063](0063-agent-validation-before-review.md) | Validation before Review: a separate read-only validator checks every submit; Validate skill; 1–5 rounds | Accepted |
| [0064](0064-auto-merge.md) | Auto-merge (Off / Epic children only / All tickets): validated tickets Local-merge without a click; conflicts go back to the agent | Accepted |
| [0065](0065-ticket-priorities.md) | Priorities 1–4 (default 3); start order pinned → priority → ready the longest | Accepted |
| [0066](0066-ready-for-dev-column.md) | "Ready for Dev": Queued gets its own column; Backlog never starts by itself | Accepted |
| [0067](0067-remove-done-when.md) | No "Done when" field: acceptance criteria live in the description | Accepted |
| [0068](0068-project-agent.md) | Project agents (investigate, create and import tickets) and several Project terminals per project | Accepted |
| [0069](0069-themes.md) | Themes: mode (Light / Dark / System) plus a dark and a light theme from 15 bundled ones; picked in onboarding and Settings; credits dialog | Accepted (mode and two themes superseded by 0070) |
| [0070](0070-one-theme-no-mode.md) | One theme, no Light / Dark / System mode; the first launch matches the OS once | Accepted |
| [0071](0071-about-screen-and-third-party-notices.md) | Third-party notices generated with cargo-about; an About section in Settings | Accepted |
| [0072](0072-install-from-github-releases.md) | Installing: GitHub Releases archives and a one-line install script; package managers later | Accepted |
| [0073](0073-updates.md) | Updates: daily check (on by default), Update now runs the install script, side-by-side versions, restart when the user picks | Accepted |
| [0074](0074-ui-on-tauri.md) | The UI moves to Tauri 2 before v0.1 (system webview, HTML/CSS/TS); GPUI dropped; core, CLI and protocol unchanged | Accepted |
| [0075](0075-tauri-ui-stack-and-m14.md) | Tauri UI stack and M14: test build first (go / no-go), React + TypeScript + Vite, screen logic in Rust (ts-rs types), xterm.js, strict TypeScript and ESLint, Node 24 + npm | Accepted |
| [0076](0076-product-name-otaq.md) | Product name: Otaq; everything renamed (command, crates, `~/.otaq/`, `otaq/` branches, plugin, bundle ID), no migration; Command tent logo; "The Otaq Authors" | Accepted |
| [0077](0077-needs-you-turquoise-in-review-magenta.md) | Needs-you family turquoise, In review magenta; Investigating moves where pink was taken | Accepted |
| [0078](0078-description-markdown-editor.md) | Ticket description: rich Markdown editor on Tiptap v3, stored as Markdown; MIT packages only, plus highlight.js (BSD-3-Clause) for code colours | Accepted |
| [0079](0079-m15-markdown-editor-milestone.md) | M15 · Markdown description editor: small tickets T-0248–T-0261; the orchestrator picks parallel or sequence (worktrees, one checked fast-forward per ticket); built outside Otaq until the personal build installs (refines 0062) | Accepted |

## Open questions (not yet decided)

- Publisher name (the `owner` in the app ID and repository path, installer metadata). The copyright line is "The Otaq
  Authors" and the logo is the Command tent (0076).
- Trademark check for "Otaq" before launch (0076). License decided: MIT (0054).
