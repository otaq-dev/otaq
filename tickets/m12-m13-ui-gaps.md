# M12 / M13 UI gaps left after M14

Found on 2026-10-13 by a read-only audit of `main` at b0d6381, after M14 finished. Every M12 and M13 UI ticket was compared
with the Tauri app. The M13 theme tickets (T-0168–T-0174) are fully built. M12's held UI tickets were built in M14 as
T-0236–T-0244 (see each ticket's "UI part:" line in `tickets/v0.1-tickets.md`), except for the gaps below. Nothing here is
built yet.

**For the agent who builds this:** follow `prompts/implementer.md` (the runbook) and SDD §5.5. Each gap is its own commit,
naming its ticket. Run `scripts/check.sh` green before each commit (Node 24 first on PATH:
`PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`). Tests never run a real `claude` and set `OTAQ_HOME` to a temp
folder. Compare with the named frames in `crewboard.pen`, which is read-only. Leave the hand checks unticked and add them
to `tickets/review-M14.md` §3. Sizes: S ≈ 30–45 min, M ≈ 1–1½ h, counting the checks.

Suggested split for two parallel teammates: **lane 1** takes gaps 4 and 5 (board and inbox), **lane 2** takes gaps 1, 2, 3
and 6 (Project agents, restart, terminals drawer). One teammate can do them in order 1 → 6.

## Gaps

### 1 · A restart that ended only Project agents shows no banner or summary (T-0205 / T-0244) · S
- **Now:** after a restart, the banner and the after-restart summary sheet appear only when a ticket was interrupted. When
  only Project agents ended, nothing appears.
- **Wanted:** the banner and the sheet also appear when only Project agents ended. The sheet already draws an
  agents-only version (`crates/otaq-view/src/view_models/after_restart.rs`); only the trigger is missing.
- **Where:** the trigger counts only `interrupted_at_start` tickets (`crates/otaq-core/src/service/mod.rs`,
  `notices.rs` `on_startup_agents`).
- **Decision first:** this is deferred question **Q47** in `tickets/questions.md`. It was built as option 1 (no banner);
  building this gap means option 2. **The user must confirm option 2 before it is built.** Then record the answer in
  `tickets/questions.md` and update the Q47 row in `tickets/review-M14.md` §5.

### 2 · The Project agent tab has no answer box (T-0204 / T-0243) · M
- **Now:** when a Project agent asks a question or a permission prompt waits, the user answers in its terminal or in the
  inbox. The agent's tab has no answer box.
- **Wanted (ticket text):** T-0204 asks for "the same answer UI as the inbox" on the agent tab. The inbox already has it
  (T-0241: the inbox answer goes to `project_agent.answer`).
- **Where:** `crates/otaq-desktop/ui/src/screens/terminals/` has no answer UI. Commit 8b947ea left it out because
  the frames don't show one.
- **Decision first:** the ticket asks for it but the frames don't show it. **Ask the user whether to build it.** If yes,
  reuse the inbox's answer component and its options.

### 3 · "Created and changed" lists no dependency rows (T-0204 / T-0243) · S–M
- **Now:** the Project agent's "Created and changed" list counts the dependencies it added ("2 dependencies") but doesn't
  list them.
- **Wanted:** a row per dependency, for example "SHOP-36 depends on SHOP-35", as frame 18 shows.
- **Where:** the core sends only a number: `dependencies` in `crates/otaq-core/src/board/created.rs` (around line
  130). Change the core to send the pairs (with a test), then the view model and the screen.

### 4 · No board edge chips for columns scrolled out of view (T-0179) · S
- **Now:** when the board is scrolled sideways, nothing shows that columns are hidden, or which ones.
- **Wanted:** a chip at the board's left and right edges naming the hidden columns (and their counts), as the GPUI app
  had. This is the only GPUI-era M12/M13 feature with no Tauri version.
- **Where:** `hidden_columns` in `crates/otaq-view/src/view_models/board.rs` (around line 126) already works out the
  hidden columns, but only its own test uses it. The GPUI version for reference:
  `git show e1dfd92^:crates/otaq-app/src/board.rs` (`edge_hint`, around lines 138–141 and 1793). Compare with the
  board frames in *03 · Board*.

### 5 · The inbox review row has no diff stat (T-0196 / T-0239) · S–M
- **Now:** an inbox row for a ticket waiting for review shows no change size.
- **Wanted:** "+412 −38 · 9 files" on the row, as the inbox frame shows.
- **Where:** the service's inbox item has no diff stat (no match in the inbox and `attention` code). Commit 32ea4c5 left
  it out. Add the stat to the inbox item in the core (from the ticket's branch against its base, with a test), then
  draw it in the inbox row.

### 6 · After a restart, the empty terminals drawer doesn't name the closed terminals (T-0203 / T-0242) · S
- **Now:** after a restart, the Terminals & agents drawer is empty and doesn't say which terminals were closed.
- **Wanted:** the empty drawer names the terminals the restart closed, as frame 17's "After a restart" scene shows.
- **Where:** `crates/otaq-desktop/ui/src/screens/terminals/`; commit bb8f2a9. The frame also shows a resizable
  drawer, but the ticket text doesn't ask for it. Leave resizing out unless the user asks.

## Bookkeeping (about 20 min, no code)

M14 built the held tickets' UI parts, but their own records were never updated:
- The 9 held M12 tickets (T-0193, T-0194, T-0195, T-0196, T-0197, T-0201, T-0203, T-0204, T-0205) still have all 40 of
  their Done-when boxes unticked, including "checks green". Tick the boxes their M14 ticket (T-0236–T-0244) ticked, and
  add a line to each saying "UI built in T-02xx (M14), commit …". Hand-check boxes stay unticked.
- `tickets/progress.md`: add rows for T-0193 and T-0197, and rows of their own for T-0203 and T-0204. The rows for
  T-0194, T-0195, T-0196, T-0201 and T-0205 still say "UI held (0074)"; point them to their M14 rows.
- `tickets/review-M12-core.md` §6 still lists all 9 as held. Add a line saying they were built in M14 (see
  `tickets/review-M14.md`).
- The 10 GPUI-era tickets (T-0169–T-0174, T-0176, T-0179, T-0180, T-0192) don't say which M14 screen ticket rebuilt them.
  Some of their ticked boxes name code that T-0245 deleted (for example T-0176's "no criteria code remains in
  otaq-app"). Add a note to each naming the M14 ticket that rebuilt it.
- Three of their unticked boxes were dropped by decision 0070 (one theme, no Light / Dark / System mode): T-0169's OS
  switch, T-0172's "In use" tag, and the "In use" half of T-0174's second box. Mark them "dropped (0070)".
- About five of their unticked boxes describe behaviour that is now built and tested in the Tauri app: the gallery's
  palette switch, the Theme credits Source and Copy notices, terminals recolouring live, a picker click saving, and the
  onboarding Theme step saving. Tick only their automated parts; the visual parts stay hand checks.

## Not decided (ask the user before building)

- **The terminal tab's validation lines:** frame 06 shows a token split on the end marker and a "Validated · round 2"
  line. Neither T-0193 nor T-0194 asks for them, and the GPUI app didn't have them (noted in `tickets/review-M14.md` §4).
- **The Project agent header:** frame 18 shows the agent's current action in the header and in the Close mid-turn dialog.
  T-0204 asks only for the name, state and tokens.

## Related, but not M12 / M13 gaps

These are listed in `tickets/review-M14.md` and belong to M14's own tickets or the Windows run:
- Windows run fails W8–W15 (§4). Two touch these screens: **W10**, the ticket card shows no External ID chip, and
  **W11**, Dev's "WORKING · VALIDATING N" label sits below the orange cards and counts them.
- Not built in M14's own tickets: the card's ▶ chip menu (T-0232), the "kept" counts in Start from scratch (T-0235), the
  PR success toast's ↗ link and the Push line counts (T-0227).
