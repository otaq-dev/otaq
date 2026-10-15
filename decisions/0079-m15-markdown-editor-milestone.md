# 0079. M15 · Markdown description editor: small tickets, built by an orchestrator that runs teammates in parallel

- **Status:** Accepted
- **Date:** 2026-10-15
- **Related:** 0053, 0056, 0062 (refined), 0075, 0078

## Context
0078 decided the rich Markdown editor for the ticket description. Its design (update P with decisions D1–D11) is in
`crewboard.pen` and was checked in `design-briefs/AUDIT-2026-10-15-p.md`. It is the first feature after v0.1 (M0–M14).

0062 says Otaq itself is used for the work after v0.1. The personal build's install method is decided
(`scripts/install-local.sh`, 0062 changelog, 0072), but the script isn't built yet, so Otaq can't build its own features
today.

Until now, milestones were built by one implementing teammate at a time, on `main`. M14's tickets already had a "Parallel
lanes" table, but its orchestrator still ran teammates one by one. The user wants this milestone cut into small tickets so
several teammates can work at once. The orchestrator decides, ticket by ticket, what runs in parallel and what runs in
sequence. Everything that changes must be easy to follow afterwards.

## Decision
- **The work is milestone M15 · Markdown description editor**, built outside Otaq the M14 way, with the orchestrator and
  teammates. This **refines 0062**: the work after v0.1 moves into Otaq once the personal build can be installed
  (`scripts/install-local.sh` exists and works). Until then, milestones run outside Otaq, and their tickets keep Otaq's
  ticket format so they can be imported later.
- **Requirements:** SDD TKT-10 (the editor), TKT-11 (copy and paste) and TKT-12 (the full-window view), plus the M15 part of
  UI-05.
- **Tickets:** `tickets/m15-tickets.md`, T-0248–T-0261.
  - Each ticket is small enough for one teammate session.
  - Each names its frames, its **Depends on** tickets and its **shared files**.
  - The file ends with a "Parallel lanes" table.
  - T-0248 is a gate. It adds every npm package once and runs the round-trip test of 0078. If Tiptap changes
    agent-written Markdown, the milestone stops for the user (0078's Milkdown fallback).
- **The orchestrator decides parallel or sequence**, from these rules (`prompts/m15-markdown-editor.md`):
  - A ticket can start when all its **Depends on** tickets have landed on `main`.
  - Two tickets run at the same time only if they don't edit the same folder or file. The exception is the one-line
    registration files that the tickets list.
  - It runs **at most 3 teammates at once**, and fewer when the Mac is short on disk or memory: one Rust-building
    teammate at a time below 60 GB free, and none without the user's go below 30 GB. Two tickets that both change Rust code
    run together only when they touch different files.
  - A ticket that changes `package.json` / `package-lock.json` or `Cargo.toml` / `Cargo.lock` runs alone.
  - It writes down each choice ("T-0252 ∥ T-0256: different folders" or "T-0259 after T-0256: same file Modal.tsx") in
    `tickets/review-M15.md`.
- **How parallel work lands on `main` (keeping 0056's "checks green before every commit"):**
  - **Every ticket** gets its own git worktree on a branch `m15/T-0xxx`, which the orchestrator creates in its scratchpad
    directory, so a ticket that runs alone doesn't hold up the next one.
  - When a ticket is done, its branch is rebased onto the current `main`, squashed into **one commit per ticket**, and
    `scripts/check.sh` must be green on exactly that tree.
  - Only then does the orchestrator land it, with a **fast-forward only** (`git merge --ff-only`).
  - If `main` moved in the meantime, the branch is rebased and checked again.
  - Landings go one at a time, so every commit on `main` has had a green check.
  - Bookkeeping (the `progress.md` row, ticked boxes and deferred questions) is added on the rebased branch just before the
    check, so parallel branches don't collide on those files.
- **Traceability:**
  - Every M15 commit names its ticket, its requirement IDs and decisions 0078 / 0079.
  - `tickets/progress.md` gets one row per ticket.
  - `tickets/review-M15.md` adds a "How it was run" section: the phases, what ran in parallel and why, and every rebase or
    re-check.
  - Deferred questions continue in `tickets/questions.md`.
  - Changes to decisions follow `decisions/README.md` as always.

## Alternatives considered
- **One teammate at a time, as in M14:** simpler and safer, but slower. The user asked for parallel work.
- **Teammates committing in parallel straight to `main`:** two teammates would build on each other's half-finished work,
  and a commit could land without a green check on the tree it creates.
- **Fixed phases written into the prompt:** the orchestrator couldn't adapt to a slow ticket, low disk or a failed check.
  The "Parallel lanes" table is a guide; the **Depends on** lists and the shared files are the rules.
- **Building M15 with Otaq (0062 as written):** waits on `scripts/install-local.sh` being built.

## Consequences
- Each worktree needs its own `npm ci` (about a minute). Rust builds in parallel worktrees cost disk, so the orchestrator
  checks free disk before starting each teammate.
- `check.sh` takes 15–25 minutes and every landing needs it, so landings are the bottleneck. Tickets that only touch the
  frontend can still be built side by side.
- `prompts/m15-markdown-editor.md` is the orchestrator's brief. `prompts/implementer.md` stays the teammates' runbook,
  with the worktree and landing rules above added by the brief.

## Changelog
- 2026-10-15: created (user decision: small tickets, an orchestrator that picks parallel or sequence; M15 built outside
  Otaq, refining 0062)
