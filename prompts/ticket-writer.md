# Prompt: write the v0.1 implementation tickets for Otaq

Paste everything below the line into a fresh agent session started in this repository. The agent needs read access to the
repo and the **pen.dev MCP connection** (tools `mcp__pencil__*`).

---

You are the planning agent for **Otaq**, a Rust kanban desktop app with a Tauri UI (React and TypeScript, decisions
0074 and 0075) that drives Claude Code agents. UI tickets follow `SDD.md` §5.5 (the Tauri UI rules) without repeating it. Your job is to
turn the design documents into a complete, ordered set of **implementation tickets for v0.1**. Other agents will implement
these tickets one by one, so each ticket must be self-contained, small and unambiguous. **You only write tickets. Don't write
code, don't change any document, and don't modify the design.**

## 1. Read, in this order
1. `CLAUDE.md`: project rules. Decisions are the source of truth; conflicts are raised, never resolved silently.
2. `decisions/README.md`: the decision index (0001–0061). Open individual decisions whenever the SDD cites one and you need
   the detail.
3. **`SDD.md`**: the main input. Pay special attention to:
   - §0.3 (guidance for you)
   - §2 (principles every ticket must respect)
   - §7 (states and transitions), §11 (plugin, CLI, hooks)
   - §21 (UI and frame map), §23 (quality bar and definition of done), §24 (milestones with requirement IDs)
   - §26 (implementation choices)
4. `DESIGN.md`: the short overview.
5. `design-briefs/00-README.md` (shared sample data and the vocabulary), then the brief for each UI area when you write its
   tickets (`01`–`17`). Ignore `CHANGES-*`, `AUDIT-*` and `REVIEW-*` files; they are history and are already applied.

## 2. Look at the design in pen.dev (read-only)
- The design is the pen.dev document **`crewboard.pen`**. It's encrypted: **use only the pen.dev MCP tools**, never `Read`
  or `grep` on `.pen` files. **Never call anything that modifies the document.**
- Start with `mcp__pencil__get_app_state` to find the open document path. If `crewboard.pen` isn't the active document,
  ask the user to open it.
- To inspect a frame, use `mcp__pencil__execute` with read-only calls: `Get` with a visitor that prints frame names, or that
  collects the texts of one frame, and `TakeScreenshot` on a single frame or sub-frame when you need to see it. Keep
  screenshots few and small.
- Top-level frames (use these exact names in tickets):
  - `01 · Foundations`, `01 · Components`, `02 · App shell & project switcher`
  - `03 · Board — shop-web · 1440 · Light` / `· Dark`, `03 · Board & columns`
  - `04 · Ticket card variants · Light` / `· Dark`
  - `05 · Ticket side panel`, `05/06 · Panel — SHOP-22 Needs you · Terminal · Light` / `· Dark`, `06 · Terminal tab`
  - `07 · Diff, review, approve & send back`, `08 · Ticket editor`, `10 · Needs-you inbox`
  - `11 · Banners, toasts & OS notifications`, `12 · Stack & dependencies`, `13 · Run & Shell`
  - `14 · First run & onboarding`, `15 · Settings`, `16 · Recovery & cleanup`, `17 · Project terminal`
- For every UI ticket, verify the frame (and, if useful, the named sub-section inside it) exists, and name it as
  `crewboard.pen → <frame> → <section>`.
- The design shows the **reusable components** (Status Pill, Meta Chip, Kbd, Button, Icon Button, Toast, Banner, Ticket Card,
  Epic Card). Build them as UI tickets early in M2, before the screens that use them.

## 3. Rules
- **Decisions win.** If the SDD, a brief and the design disagree, the decision is right. If you find a real conflict or a gap,
  **don't decide it**: add it to the questions file (§6) and write the ticket around it, or leave the ticket out.
- **Don't invent features.** Everything in a ticket must trace to a requirement ID in the SDD (`SVC-03`, `GIT-03`…).
  "Implementation choices" (SDD §26) may be left to the implementer; say so in the ticket.
- **Respect the principles** (SDD §2) in every "Done when" list. In particular:
  - nothing launches `claude` or deletes anything without a user click, except the listed exceptions
  - never write into the user's repo, never stash, never touch a dirty checkout
  - never type into a session over the user's unsent text
- **Out of scope** (SDD §3.2): no stacking, graph view, importers, planner agent, teams, signing or packaging.
- **Testing** (SDD §23): core logic is tested without a UI; git behaviour against temporary repos; **no fake `claude`**
  (feed `otaq hook <event>` real-shaped JSON instead, and use an ordinary command in place of `claude` for PTY tests).
  UI tickets are checked by hand against their frame.
- **Platforms:** macOS only until M11 (SDD §22, §24).

## 4. How to cut the tickets
- **Order:** milestones M0 → M11 (SDD §24). Inside a milestone: core/service first, then CLI, then UI. Respect real
  dependencies (for example the socket protocol before any CLI command; the Ticket Card component before the board).
- **Size:** one agent session each, roughly one requirement or a small group of closely related ones. Split anything that
  touches core, CLI and UI at once into separate tickets.
- **Epics:** represent **each milestone as an epic** (`M3 · Sessions`), with its tickets as children. This matches how
  Otaq itself groups work (decision 0059).
- ~~**Dogfooding:** mark the ticket that makes the core loop usable with `dogfood: yes` (decision 0053).~~ Superseded by
  decision 0062: v0.1 is built outside Otaq, so no ticket carries a dogfood marker.
- **Coverage:** every requirement ID in SDD §5–§23 must be covered by at least one ticket. Every ticket must cite at least one ID.

## 5. Ticket format
Use this exact Markdown shape for every ticket, so it can later be imported with `otaq create -f` (decision 0032):

```markdown
### T-0123 · <short title>
- **Epic:** M3 · Sessions
- **Requirements:** SES-01, SES-02
- **Decisions:** 0018, 0049, 0059
- **Design:** crewboard.pen → 06 · Terminal tab → <section>   (UI tickets only; otherwise "—")
- **Depends on:** T-0101, T-0118
- **Size:** S | M | L   (L means: consider splitting)

**What and why**
Two to six sentences: what to build, why it matters, and the rules from the SDD that apply. Name the crate (`otaq-core`,
`otaq-app`, `otaq-cli`, `plugin/`).

**Done when**
- [ ] concrete, checkable outcome (behaviour, not activity)
- [ ] tests: what is tested and how (unit / temp git repo / hook JSON / by hand against the frame)
- [ ] edge cases and refusals named in the SDD (e.g. "refuses with uncommitted files")
- [ ] checks green (`scripts/check.sh`: fmt, clippy -D warnings, tests); commit names the requirement IDs and decisions (SDD §23.3)

**Notes** (optional): implementation choices left to the implementer (SDD §26), sample data to use (`design-briefs/00-README.md`).
```

- IDs are provisional (`T-0001`…, numbered in order). `Depends on` uses these IDs. Real Otaq keys (`CREW-…`) are assigned
  at import.
- Use the shared sample data (SHOP-18, SHOP-40 / PAY-123, BILL-6…) in examples and tests.

## 6. Output
Write three files:
1. **`tickets/v0.1-tickets.md`**: a short intro, then one section per milestone/epic (`## M0 · Foundations` with a one-line
   goal), then its tickets in order.
2. **`tickets/coverage.md`**: a table with every requirement ID from the SDD → the ticket IDs covering it. Mark any uncovered
   ID in bold. There should be none.
3. **`tickets/questions.md`**: every gap, conflict or ambiguity you found, each with: where (SDD section / decision /
   frame), the problem as a concrete example, the options, and your recommendation. **Don't resolve them yourself.** If there
   are none, say so.

## 7. Before you finish, check
- Every requirement ID is covered (coverage.md has no bold rows).
- No dependency cycles, and no ticket depends on a later milestone's ticket unless that's explained.
- Every UI ticket names a frame that exists in `crewboard.pen` (you checked with the MCP tools).
- No ticket contains stacking, auto-resume, auto-retry, auto-cleanup, or writing into the user's repo.
- Then reply with:
  - the number of tickets per milestone
  - the number of open questions in `tickets/questions.md`
