# Prompt: plan and build M17 · Epic lead and recorded decisions (orchestrator, teammates)

Start **one fresh Claude Code session** in this repository and paste everything below the line. Like M16, the session
works in two phases: it writes the tickets and stops for the user's approval, then builds them with teammates the way M15
was built (decision 0079).

It needs the **Agent** tool (and SendMessage), the **pen.dev MCP connection** with `crewboard.pen` open, and the **VMPal
MCP connection** for the last ticket's Windows check. It must be the only session building code in this repository.

**Before you paste it (the user):** commit the planning documents first (decisions 0083 and 0084, `SDD.md` §6.6 and §6.7,
the design briefs, this prompt), so `git status` is clean. Make sure the design agent's last fix for update T (state 29)
is applied.

---

You are the orchestrator for milestone **M17 · Epic lead and recorded decisions** of **Otaq**, a Rust kanban desktop app
(Tauri 2, React, TypeScript) that drives Claude Code agents. M17 gives an epic a read-only **lead** agent that plans the
epic with the user, answers its children's questions first, leaves notes for running children, nudges a stuck child once
and re-plans after each merge (decision 0083). It also lets every ticket's agent **record** a product choice instead of
stopping to ask, and lists every recorded decision in the review (decision 0084). The work has two phases: first you write
the tickets and the user approves them, then you build them with teammates, exactly the way M15 and M16 were built.

## 1. Read first
1. `CLAUDE.md` and `decisions/README.md`. Then `decisions/0083-epic-lead-agent.md` and `0084-recorded-decisions.md`,
   including their changelogs (this feature; they override anything older that disagrees), and 0016, 0018, 0022, 0030,
   0034, 0040, 0042, 0044, 0045, 0048, 0050, 0059, 0060, 0061, 0063, 0064, 0068, 0079, 0080.
2. `SDD.md` §6.6 (LEAD-01…LEAD-12) and §6.7 (DEC-01…DEC-06), the M17 rows of §11.2, CLI-07 and CLI-08 in §11.3, the M17
   lines of §12.2, then §9, §11, §12, §14 (validation, Send back, Auto-merge), §15, §16, §17, §19.1 (Project agents: the
   lead reuses that session kind), §21, §23, and the M17 row in §24. §5.5 has the Tauri UI rules: screen logic and wording
   come from Rust view models (ts-rs types).
3. `design-briefs/CHANGES-2026-10-16-s.md` and `CHANGES-2026-10-16-t.md`: the frame maps (what to build from) and the
   audits (points decided during them that some frames don't fully draw, and fixes asked of the design agent).
   `reports/Lead and child agents - evidence from past runs.md` explains why the feature looks the way it does.
4. The design, read-only: load the tools with ToolSearch
   (`select:mcp__pencil__get_app_state,mcp__pencil__execute,mcp__pencil__read_skill`) and read the frames in the two frame
   maps. Don't build from rows labelled "before 0083" / "before 0084" or from older SHOP-23 "Asks you" frames. Never
   modify `crewboard.pen`.
5. How M15 and M16 were run: decision 0079, `prompts/m15-markdown-editor.md`, `tickets/m16-tickets.md` (ticket format,
   "Words used", "Where the code goes", "Shared files", "The example", "Parallel lanes"), `tickets/review-M15.md` and
   `tickets/review-M16.md` ("How it was run"). Also `prompts/orchestrator.md`, `prompts/implementer.md` and
   `prompts/ticket-writer.md`.
6. The current code this builds on: Project agents (sessions, `otaq prime --project`, naming, lifecycle, the Terminals &
   agents UI), the validator and validation rounds, `otaq ask human` / `answer` / Messages, the inbox groups,
   `crates/otaq-core/src/policy.rs` (who may change what), Send back, Park, the epic strip and track label (M16), the
   plugin skills in `plugin/skills/` (`work-ticket`, `commit`, `project-agent`).
7. `tickets/progress.md` and `tickets/questions.md`. The last ticket is T-0275; M17 tickets start at **T-0276**. The last
   question is Q60; new ones start at **Q61**.

## 2. Before anything else
- `git status` is clean and HEAD is on `main`. If anything is uncommitted, ask the user whose it is; don't commit, stash or
  discard it. Ask whether another session is building in this repository. If M17 rows, `m17/*` branches or worktrees
  already exist, an earlier run was interrupted: say what's there and ask.
- Machine, disk, Node and `check.sh`: exactly as `prompts/m15-markdown-editor.md` §2 (Node 24 PATH prefix; `check.sh`
  green on `main`, 15–25 minutes; below 60 GB free one Rust-building teammate at a time, below 30 GB ask first).
- Tell the user in three lines what you'll do first.

## 3. Phase 1: write the tickets (you, no code)
Write `tickets/m17-tickets.md` in the M16 format: an intro ("Words used", "Where the code goes", "Shared files", "The
example"), then tickets T-0276…, each small enough for one teammate session, each naming its requirement IDs (LEAD-01…12,
DEC-01…06, CLI-07, CLI-08), its decisions, its `crewboard.pen` frames (from the frame maps), its Depends on tickets, its
Shared files, "Done when" boxes and hand checks. End with a "Parallel lanes" table. Also add the new IDs and tickets to
`tickets/coverage.md`.

What to cover (split as you see fit; these are areas, not tickets):
- **Recorded decisions, core and CLI (DEC-01, DEC-02, DEC-04…06):** the decision record (question, built, other options,
  why; the lead's mark; kept / changed), `otaq decision record` and `otaq decision mark`, `otaq show` / `prime` listing
  them, Approve marking them kept, Change… → Send back with the text (line comments included) or a fix child in an epic.
  These don't need the lead and can land first.
- **The lead session, core (LEAD-01, 02, 04, 12):** a lead per epic reusing the Project agent session kind; start (Open
  lead, or queueing an epic without a lead), resume, end at Ready for Review, resume when a child is added or reopened,
  "Ended by restart", not in Resume all, tokens on the epic, plan-limit hold, not in `max_concurrency`; the wake-ups as
  automatic messages (§9.3) after a child merges or is closed, a child asks, a child first gets stuck.
- **Context and permissions (LEAD-03, LEAD-08, CLI-07):** `otaq prime --epic`; the lead caller in `policy.rs` (own epic's
  never-started children only; refusals with the SDD wording); `otaq suggest close|delete` and its inbox group.
- **Questions and stuck children (LEAD-06, LEAD-07):** routing an epic child's `ask human` to a live lead ("asked lead ·
  waiting"), `answer`, `ask human --pass-on` with the lead's recommended option, Messages on child and epic; the first stop
  (validation failed, keeps conflicting, Idle) waking the lead once, `otaq note` counting as Answer the agent, the second
  stop to the inbox with the lead's note and view.
- **Notes and same file (LEAD-09, LEAD-10, CLI-08):** unread notes, `otaq notes`, `prime` including them, the submit
  refusal, typing an unread note in when the child goes idle; the same-file overlap (changed files per running child against
  the epic branch) and its card flag; no lead wake for it.
- **Plugin:** the new `/otaq:epic-lead` skill (plan, check the plan, merge turn, answer only from written sources and name
  them, pass on what nothing written settles, one note to a stuck child, never edit code); `work-ticket` teaching DEC-01
  (`otaq decision record` unless blocked) and `otaq notes`; the built-in Commit skill running `otaq notes` first.
- **View models and UI (LEAD-11, DEC-03, DEC-04, DEC-05):** the Lead Status (epic card, track label line, epic panel), the
  Lead tab (terminal, tokens, Changes by the lead, "Before the restart"), the epic panel's tabs; card states (asked lead ·
  waiting, Asks you · via lead, Updated by lead, new · added by lead, Lead's note sent, 1 unread note, same file, N
  decisions) and the flag order; the inbox items (passed-on question, stuck child with the lead's note, the suggestions
  group, the lead's own question); the review's Decisions list (the Decision Item), "Approve · keeps N decisions", the
  Change… and "Add a fix child" dialogs; History lines; the reboot summary line.
- **A final ticket:** hand-check pass on macOS (several checks need a real `claude`: say which), and the Windows check if
  the VMs are available (as T-0275 did).

Rules for the tickets:
- Build only what 0083, 0084, LEAD-01…12 and DEC-01…06 say. Out of scope: provider-specific delivery of notes (hooks), a
  setting for asking vs recording, waking the lead on overlaps, any new shortcut, and anything about other agent providers
  beyond keeping the lead CLI-only.
- If a ticket needs a visual that no frame in the maps shows, reuse existing components and say so in the ticket. If that
  isn't enough, list it as a design gap for the user. Don't invent a design.
- If anything in the decisions, the SDD and the design disagrees, raise it to the user as a blocker: quote the decision
  number and text, give options with a concrete SHOP-40 example and your recommendation. One topic at a time, in plain
  language.

Then STOP and give the user a short summary: the ticket list (number, title, size, depends on), the parallel lanes, and
any design gaps or questions. Wait for their approval before Phase 2. This is the one planned stop.

## 4. Phase 2: build (orchestrator, teammates)
Follow `prompts/m15-markdown-editor.md` §3–§7 exactly, replacing M15 with M17, `m15/` with `m17/`, the ticket file with
`tickets/m17-tickets.md`, the review file with `tickets/review-M17.md`, and decisions 0078/0079 with 0083/0084/0079. In
short:
- You don't write product code. Each ticket gets its own worktree on branch `m17/T-0xxx` in your scratchpad and a fresh
  teammate (general-purpose, name "m17-T-0xxx") with the M15 teammate brief.
- A ticket starts when its Depends on tickets have landed on `main`; parallel only when Shared files don't overlap; at most
  3 teammates; one Rust-changing teammate at a time unless disk allows and files differ; Cargo/npm changes run alone.
- Landing: rebase onto `main`, one squashed commit per ticket (naming the ticket, its requirement IDs and decisions
  0083 / 0084), `check.sh` green on exactly that tree (you run it, in the foreground, and read its exit code), then
  `git merge --ff-only`. If `main` moved, rebase and re-check. Landings one at a time.
- Write every parallel/sequence choice, landing, rebase and re-check down for "How it was run".
- Only blockers stop the work; everything else is deferred to `tickets/questions.md` (from Q61) as "Deferred: built as
  option N". Ping the user only for blockers, no progress summaries.

Lessons from M14–M16 (`reports/Lead and child agents - evidence from past runs.md`), apply them:
- Put what a teammate needs in its **brief** (including "notes for later tickets" from earlier teammates); a message sent
  while it works may reach it only when its turn ends.
- Trust state, not words: check worktrees, processes and exit codes yourself before acting on "the check is running" or
  "done".
- Ask teammates for reports under 600 words; take question numbers from `main` at landing.
- Name shared registration files in every brief (icons, `generated.rs`, `protocol.rs`, `mod.rs` lines, the Gallery).

## 5. Rules that always apply
- Decisions are the source of truth; conflicts are raised, quoted and confirmed by the user, never resolved silently.
- Every commit on `main` lands by fast-forward after a green `check.sh` on exactly that tree (0056, 0079).
- Never push, open a PR, rewrite `main`'s history, change git config or Claude Code settings, or modify `crewboard.pen`.
- Don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md`, except to record an answer the user
  confirmed (`decisions/README.md` rules), in its own checked commit.
- Tests use `OTAQ_HOME` in a temporary folder, never `~/.otaq/`, and never start a real `claude`.

## 6. When M17 is done
Everything in `prompts/m15-markdown-editor.md` §7, for M17: every ticket landed with a progress row, `review-M17.md` with
"How it was run" and "Platforms checked", `check.sh` green on `main`, clean status, no `m17/*` worktrees or branches, no
test-started `otaq` process. Then give the user the ticket → commit table, how it was run in a few lines, the unticked hand
checks by screen, the deferred questions, and the platforms not checked.
