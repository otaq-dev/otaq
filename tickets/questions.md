# Open questions from ticket writing (v0.1)

These are the gaps, conflicts and ambiguities found while turning `SDD.md`, the decisions, the briefs and `crewboard.pen` into
tickets. **None of them is resolved here.** Where a ticket touches a question, it says so in its Notes. Decide them one at a
time; each answer may need a decision update (refinement or a new decision), per `decisions/README.md`.

Q1–Q5 are conflicts or gaps that affect tickets. Q6–Q9 are design items with no requirement, so they were left out.
Q10–Q11 are a small consistency point and sequencing choices to confirm. Q12 is a gap found while writing the git tickets.

---

## Q1 · Is the `restack` skill in v0.1 or deferred?

- **Where:** decision 0049 ("Not in v0.1 … the `restack` skill and 'Resolving restack conflict'") vs decision 0038 ("Send to
  agent to fix … returns the ticket to Dev with the `restack` skill"), decision 0044 (reopen: "resolving it is the agent's
  first task"), SDD §11.1 (`restack/` listed in the plugin) and GIT-03 step 5. Tickets: T-0090, T-0101.
- **Problem:** SHOP-18 and SHOP-19 both edit `lib/pricing.ts`. SHOP-19 is merged first. The user clicks Local merge on SHOP-18,
  the free rebase conflicts, and the panel offers **Send to agent to fix**. 0038 says this click sends the agent the `restack`
  skill. 0049 says the `restack` skill is deferred with stacking. Both can't be true.
- **Options:**
  1. Keep the `restack` skill in v0.1, used only for conflicts with the base (Local merge, reopen). Refine 0049 to say only
     the *stacking* uses of it are deferred.
  2. Drop the skill; "Send to agent to fix" types a plain instruction into the session instead ("Rebase onto `main` and
     resolve the conflicts, then submit again").
- **Recommendation:** option 1. It matches 0038, 0044 and the SDD, and keeps the agent's behaviour in Markdown (d0009). Only
  0049's wording changes. T-0090 is written this way.

## Q2 · When an agent asks a finished or reviewed ticket a question, is it auto-resumed or sent to the inbox?

- **Where:** decision 0030 (refined in the grilling session: a ticket with no live session but a stored Claude session "is
  resumed automatically to answer", setting on by default) and SDD MSG-03 / REC-01 ("questions to them follow §16") vs
  decision 0045 ("Ready for Review and Approved … Questions from other agents go to the inbox (0030)") and decision 0049
  ("Questions to a parent go to the inbox, because a merged parent has no live session (0030)"). Tickets: T-0111, T-0152.
- **Problem:** SHOP-20's agent runs `crewboard ask SHOP-18 --stdin` ("What does POST /checkout return on a declined card?").
  SHOP-18 is merged (Done) and has no live session. By 0030 and the SDD, Crewboard recreates SHOP-18's worktree, resumes its
  conversation answer-only and delivers the answer. By 0045 and 0049, the question lands in the user's inbox instead.
- **Options:**
  1. Follow 0030 / SDD MSG-03: auto-resume answer-only when "Let finished tickets answer questions" is on (default); inbox
     only when it's off. Fix the wording in 0045 and 0049.
  2. Follow 0045 / 0049: always route to the inbox when there is no live session; drop the auto-resume exception.
- **Recommendation:** option 1. 0030's changelog shows the auto-resume rule was a later, explicit user decision, and the
  SDD is built on it. T-0152 follows option 1.

## Q3 · Where does the database live?

- **Where:** decision 0055 (`~/.crewboard/` on every platform) vs `crewboard.pen → 15 · Settings → Global · Usage, notifications,
  data, appearance`, which shows `~/Library/Application Support/Crewboard/crewboard.db`.
- **Problem:** the Settings screen shows a path that the decision rules out.
- **Options:** 1. Keep 0055 and update the frame text. 2. Change 0055 to use the OS application-support folder.
- **Recommendation:** option 1 (decision wins). T-0045 shows the real path from the service (`~/.crewboard/…`).

## Q4 · What happens to worktrees when an epic is closed?

- **Where:** decision 0033 and SDD CLS-01 ("worktree removed unless it has uncommitted changes") vs
  `crewboard.pen → 05 · Ticket side panel → Epic dialogs → Close epic · confirm`: "Branches and worktrees stay until you remove
  them in Cleanup." Tickets: T-0043, T-0100.
- **Problem:** closing SHOP-40 Wishlist stops SHOP-40.2's agent. By 0033 its worktree is removed (unless dirty) and its branch
  kept. The dialog promises that worktrees stay until Cleanup.
- **Options:** 1. Follow 0033 and change the dialog text ("Branches are kept. Worktrees are removed unless they have
  uncommitted changes"). 2. Make epic closes keep all worktrees and list them in Cleanup (a refinement of 0033).
- **Recommendation:** option 1. One rule for every close is simpler. T-0043 and T-0100 follow 0033.

## Q5 · A reopened ticket dragged to Dev: resume its old conversation or start a new one?

- **Where:** decision 0044 item 5 and SDD GIT-06 / §7.3 ("Old branch reused; free rebase onto the current base first; on
  conflict, resolving is the agent's first task"). Nothing says how the agent starts. Ticket: T-0101.
- **Problem:** SHOP-26 was closed ("won't do"), then reopened and dragged to Dev a week later. Its worktree was removed at
  close; its branch and Claude session ID are kept. Crewboard can either (a) recreate the worktree at the same path and run
  `claude --resume <id>` like Park (d0052), or (b) start a fresh `/crewboard:work-ticket` session on the old branch. On a rebase
  conflict, the agent also needs to be told to resolve it first (restack, see Q1).
- **Options:**
  1. Resume the stored conversation (like Park), then type the restack instruction if the rebase conflicted.
  2. New session with `work-ticket`; the conflict, if any, is mentioned in `crewboard prime`.
  3. Resume if the conversation still exists, else a new session (no silent fallback: ask first, like Resume failed).
- **Recommendation:** option 2. A closed ticket was usually closed because the plan changed; a clean start reads the current
  ticket and base, and avoids a "Resume failed" when Claude has cleaned up old transcripts. T-0101 builds the rebase and keeps
  the start path in one function so either answer is a small change.

## Q6 · Deleting tickets (Project terminal "Undo")

- **Where:** `crewboard.pen → 17 · Project terminal → The review moment` and `Undo a created ticket` ("Trash on a summary row
  deletes that ticket after confirming; dependencies pointing at it are removed too"), brief 17 item 3. No requirement in the
  SDD and no decision covers deleting a ticket; the protocol surface (§5.4) has no delete. TKT-01 only says numbers are
  "never reused, even after deletion".
- **Problem:** Claude creates SHOP-18–SHOP-21 in the Project terminal and the user wants to remove SHOP-21. Today the only way
  is Close ticket ("won't do"), which leaves it in Done.
- **Options:** 1. Add a requirement: delete a never-started ticket (confirmation; its dependency links are removed and named;
  its number is not reused); also the review-moment summary of created tickets. 2. Leave deletion out of v0.1 and change the
  frame to use Close.
- **Recommendation:** option 1, limited to never-started tickets, as a new requirement in M9 next to T-0138. **Not ticketed
  until decided.**

## Q7 · Enterprise usage-limit banners (Request more / Retry)

- **Where:** `crewboard.pen → 11 · Banners, toasts & OS notifications → Banners → Usage limit · enterprise · reached /
  retrying / still at the limit / limit raised`. Not in SDD §18 or decisions 0020/0046.
- **Problem:** the frame adds a **Retry** button that "restarts the 8 stopped sessions" and a **Request more** link to the
  organization's request page. Retry is a user click (fine under P1), but the flow, how Crewboard knows the plan is an
  enterprise plan, and where the request URL comes from are not decided.
- **Options:** 1. Add a decision for enterprise limits (detection, Retry = the same "continue" delivery as Resume all, the
  request link). 2. Treat enterprise limits as "resets at an unknown time" (already in USG-04/USG-06), with Resume all as the
  retry, and drop these frames.
- **Recommendation:** option 2 for v0.1: it needs no new mechanism, and the user can still click Resume all. **Not ticketed.**

## Q8 · Answer options with a recommended choice in the inbox

- **Where:** `crewboard.pen → 10 · Needs-you inbox → Answer · inline` and `Answer · options, no recommendation` ("When the
  asking agent sends options, they're listed; its recommended option is marked and preselected … Press 1–4 to pick"). SDD §11.2
  defines `crewboard ask human --stdin` / `crewboard ask <KEY> --stdin` as free text only. Tickets: T-0120, T-0153.
- **Problem:** to show "OPTIONS FROM SHOP-21'S AGENT" with one marked "Recommended", the agent must send structured options.
  No command or skill defines that format.
- **Options:** 1. Extend `ask` (e.g. repeatable `--option "text" [--recommended]`, or a small Markdown convention on stdin)
  and teach it in the skills. 2. Free-text answers only in v0.1; options later.
- **Recommendation:** option 2 for v0.1 (smaller CLI surface, P8); revisit after daily use. Tickets build free text only.

## Q9 · Card flag "asked SHOP-18 · waiting"

- **Where:** brief 04 ("Asked a linked ticket, waiting for the answer · SHOP-20 · 'asked SHOP-18 · waiting'"),
  `crewboard.pen → 04 · Ticket card variants → Working · asked another agent`. SDD §7.2's flag list has "answering SHOP-40.2"
  (on the target) but no flag on the asker.
- **Problem:** SHOP-20's agent asked SHOP-18 and keeps working (asking is non-blocking, MSG-01). Should SHOP-20's card show
  "asked SHOP-18 · waiting" until the answer arrives?
- **Options:** 1. Add the flag to §7.2 (display only, low priority in the pill order). 2. Leave it out; the Messages section
  already shows pending questions.
- **Recommendation:** option 1; it is cheap and the design exists. **Not ticketed until decided** (it would join T-0154).

## Q10 · Stale text in decision 0004 (and 0039) about epics

- **Where:** decision 0004 still says epics "live only in Backlog … They move to Done automatically when all their children
  are Done"; 0039 says the same and is marked "partly superseded by 0059". 0059 (and SDD EPC-04) send the epic to Ready for
  Review instead.
- **Problem:** an implementer reading 0004 alone would build auto-Done for epics.
- **Options:** 1. Add a changelog line to 0004 pointing to 0059 (refinement). 2. Leave as is; the SDD is clear.
- **Recommendation:** option 1. Tickets follow 0059 / EPC-04.

## Q11 · Sequencing choices to confirm (no decision conflict)

These follow from the milestone table but needed a choice. Say if any should change.

1. **A GPUI window and the first components in M1, not M2.** M1 includes the service banners (UI-07) and onboarding
   (UI-08), which need a window, the design tokens and Button/Banner/Toast/Kbd. So T-0014–T-0016 are in M1; the other
   components (Status Pill, Meta Chip, inputs, Ticket Card, Epic Card) are early in M2 (T-0030–T-0032) before the screens.
2. **Onboarding split across M1 and M2.** "Add your first project" needs projects (PRJ-01, M2), so Welcome + tools
   checklist are T-0018 (M1) and the add-project step is T-0034 (M2).
3. **A temporary start trigger in M3.** Sessions (M3) come before the scheduler (M5). T-0053 starts a ticket as soon as it
   enters Queued (no readiness, no limits) so M3 and M4 can be used; T-0079 replaces it.
4. **Needs-you item derivation in M5.** Back-pressure (SCH-03, M5) needs the count of inbox items, so the derivation is
   T-0078 in M5; the inbox surface (M7) reuses it.
5. **Base fetch/fast-forward in M3.** SES-01 step 2 needs it at start, so T-0047 (M3) builds it and cites GIT-01; Local merge
   (M6) reuses it.
6. **A service restart before M7.** The restart button exists from M1 (T-0017), but Interrupted handling arrives with
   reconcile in M7 (T-0111). Until then a restart simply ends sessions.

## Q12 · Does the checkpoint commit run the user's git hooks?

- **Where:** SDD GIT-02 / decision 0005 (checkpoint commit `wip: paused by Crewboard`, rebase, `git reset --soft HEAD~1`) vs
  P4 / decisions 0012, 0055 ("the user's git config, hooks and credentials apply"). Ticket: T-0088.
- **Problem:** SHOP-26 is reopened with uncommitted files kept in its worktree. Before the free rebase, Crewboard makes the
  checkpoint commit. The user's `pre-commit` hook runs the linter, and a `commit-msg` hook rejects messages without a ticket
  prefix, so the checkpoint fails and the rebase can't run. The commit is undone a moment later and never leaves the machine.
  Note: Local merge already refuses while uncommitted files exist (GIT-03 step 1), so in v0.1 this mainly affects reopen.
- **Options:** 1. Respect hooks; a hook failure stops the operation with a clear error (current ticket default).
  2. Pass `--no-verify` for the checkpoint commit only (a narrow exception, like `--allowedTools` in 0011).
- **Recommendation:** option 1 for v0.1: it keeps P4 without exceptions, and the case is rare. Revisit if it bites in daily use.

---

## Resolutions (2026-10-09)
- **Q1 → option 1.** The `restack` skill stays in v0.1 for conflicts with the base (Send to agent to fix, reopen). Only its
  stacking uses are deferred. Decision 0049 wording fixed. T-0090 is correct as written.
- **Q2 → option 1.** Follow 0030 / SDD MSG-03 (auto-resume answer-only when the setting is on; inbox when off). Decisions 0045
  and 0049 wording fixed. T-0152 is correct as written.
- **Q3 → option 1.** Decision 0055 wins (`~/.crewboard/`). The design text is fixed by `design-briefs/CHANGES-2026-10-09-h.md`.
- **Q4 → option 1.** Decision 0033 wins (worktrees removed unless dirty). The design text is fixed by update H.
- **Q10 → option 1.** Decision 0004 now points to 0059.
- **Q5 → option 2.** A reopened ticket starts a new `work-ticket` session; a conflict is announced via `crewboard prime`. Decision 0044 and SDD GIT-06 / §7.3 updated. Adjust T-0101 accordingly.
- **Q6 → option 1.** New decision 0060 and SDD TKT-08 (M2): delete only never-started tickets. Add a ticket for it (and the Project terminal summary's delete in M9).
- **Q7 → option 2.** No enterprise banners in v0.1 (SDD USG-08, d0020 changelog). Not ticketed; the frames get an "after v0.1" label (update H).
- **Q8 → real options in v0.1** (user rejected free-text-only). New decision 0061; SDD §11.2 `ask human --option … --recommended`, PLG-06 (M4), ATT-12 (M7). Add tickets for the CLI flags + storage, the skill text, and the inbox/card/panel option UI.
- **Q9 → option 1.** Add the "asked <KEY> · waiting" flag (d0030 changelog, SDD §7.2 and MSG-06, M10); it joins T-0154.
- **Q11 → all six accepted** (the user leaves ordering to the planner, d0053).
- **Q12 → option 1.** The checkpoint commit respects git hooks; a rejection stops with a clear error (d0005 changelog, SDD GIT-02). T-0088 is correct as written.

All 12 questions are resolved. Update the tickets for Q5, Q6, Q8 and Q9 (see above) and re-run the coverage check.

---

## New open questions (round 2, 2026-10-09)

Found while adding the tickets for decisions 0060 and 0061. Both are open; the tickets are written around them.

## Q13 · Deleting an epic that still has never-started children

- **Where:** decision 0060 ("Delete is available only for tickets (and epics with no started children) that never went to
  Dev") and SDD TKT-08. Ticket: T-0161.
- **Problem:** epic SHOP-40 Wishlist has two children, SHOP-40.1 and SHOP-40.2, and neither has started. The user deletes
  SHOP-40. 0060 allows it, but doesn't say what happens to SHOP-40.1 and SHOP-40.2: their keys are child keys of SHOP-40
  (d0039), so they can't simply stay.
- **Options:** 1. Delete the children too; the confirmation lists them ("Also deletes SHOP-40.1, SHOP-40.2"). 2. Refuse until
  the children are deleted ("Delete or move its tickets first"). 3. Turn the children into standalone tickets with new keys
  (old keys kept as aliases, like Move to epic in reverse).
- **Recommendation:** option 1. It matches how Close epic works (one confirmation for the whole epic, d0044) and keeps keys
  simple. Until this is answered, T-0161 refuses (option 2), which is the safe subset.

## Q14 · Design texts that disagree with decisions 0060 / 0061, and missing option frames

- **Where:** `crewboard.pen` frames vs decisions 0060 and 0061. Tickets: T-0153, T-0165, T-0166.
- **Problems:**
  1. *10 · Needs-you inbox → Answer · inline* shows numbered options on an **agent-to-agent** item ("SHOP-21 asks SHOP-17
     (Done)…"). Decision 0061 limits options to `ask human`; `ask <KEY>` stays free text.
  2. ATT-12 puts the options on the **card** and in the **panel**, but no frame shows them there (only the inbox has them).
  3. *16 · Recovery & cleanup → Cleanup · 3 items / 20 items* show "ticket SHOP-14 was deleted · branch crewboard/SHOP-14".
     Under 0060 only never-started tickets can be deleted, so a deleted ticket never has a branch.
- **Options:** 1. Keep the decisions and ask the design agent to fix the frames: move the options example to an Asks you item,
  add card and panel option states, and reword the Cleanup example (e.g. "no ticket owns this branch"). 2. Widen 0061 to allow
  options on agent-to-agent questions.
- **Recommendation:** option 1 (decisions win). T-0153 keeps agent-to-agent answers free text; T-0166 reuses the inbox pattern
  for the card and panel until a frame exists.
- **Q14 → option 1.** Decisions win; the design fixes are in design-briefs/CHANGES-2026-10-09-i.md. T-0153 and T-0166 are correct as written.
- **Q13 → option 1.** Deleting an epic deletes its never-started children in one confirmation (d0060, SDD TKT-08). Update T-0161 from "refuse" to this.
- **Q15 (from the pen agent) → decision wins.** The single-ticket "Close ticket · confirm" text contradicted 0033; fix added to design update I (item 6).

---

## New open questions (M3 implementation, 2026-10-10)

## Q16 · Terminal shortcuts in the design that the keyboard map (d0058) doesn't have

- **Where:** decision 0058 ("**Rule:** no shortcut may appear twice across the three columns, and OS system shortcuts
  (for example ⌘` on macOS) are off-limits" and its table, the full map) vs `crewboard.pen → 06 · Terminal tab`:
  *Live · streaming* and *Narrow panel* hint "Click the terminal or press **⌘T** to type to the agent", *Annotations*
  "Click inside the grid or ⌘T focuses the terminal", *Scrolled up · find in scrollback* pill "142 new lines · **⌘↓**";
  and `05 · Ticket side panel → Close ticket · panel "…" menu`: "Copy branch name **⇧⌘C**". Tickets: T-0057, T-0058,
  T-0060.
- **Problem:** the three shortcuts are not in d0058's table. M3 built without them: the hint reads "Click the terminal to
  type to the agent", the pill reads "142 new lines" (a click jumps), and Copy branch name has no shortcut. Separately,
  ⇧⌘` (Leave terminal, in d0058) is also macOS's "previous window of this app" shortcut; it reaches Crewboard today
  because the app has no Window menu, but that is worth a look in the hand check.
- **Options:**
  1. Keep d0058 as the full map: the design drops ⌘T, ⌘↓ and ⇧⌘C (design fix).
  2. Add them to d0058 (refinement): ⌘T focus terminal (Win/Linux: Ctrl+Shift+T is taken by Project terminal, so it would
     need another key), ⌘↓ jump to latest (inside the terminal only), ⇧⌘C copy branch name.
- **Recommendation:** option 1 for v0.1 (fewer shortcuts to keep collision-free across three columns; clicking works for
  all three). If ⌘T is wanted, it needs a Win/Linux key that doesn't collide first.
- **Q16 → option 1 (2026-10-10).** Decision 0058 stays the full map; ⌘T, ⌘↓ and ⇧⌘C are not added. The design fix is
  design-briefs/CHANGES-2026-10-10-j.md. T-0057, T-0058 and T-0060 are correct as built. The ⇧⌘` (Leave terminal) note stays a
  hand check in review-M3.md.

---

## New open questions (M4 implementation, 2026-10-10)

## Q17 · May an agent's new ticket depend on an existing ticket?

- **Where:** decision 0031 ("**Create tickets in Backlog** (including `--parent` for epics), and add dependencies only
  from its own ticket or between tickets it created") and SDD CLI-02. Tickets: T-0065, T-0069.
- **Problem:** SHOP-19's agent splits off a follow-up: `crewboard create "Price table: currency column" --deps SHOP-19`.
  The link goes **from the new ticket** (created by the agent) **to SHOP-19** (its own ticket, created by the user).
  Read literally, 0031 refuses it: the link isn't from SHOP-19, and SHOP-19 wasn't created by the agent. The same
  happens with `--deps SHOP-18` (a user's ticket) or a `create -f` plan that depends on existing tickets. Such a link
  changes only the agent's own new ticket; no user ticket gets a new dependency.
- **Options:**
  1. Keep the literal rule: an agent links only from its own ticket, or between two tickets it created. Example: the
     agent must create the follow-up without `--deps` and ask the user to link it.
  2. Also allow links whose **dependent** (the child) is a ticket the agent created, to any parent in the project.
     Example: `create "currency column" --deps SHOP-19` works; `dep add SHOP-12 SHOP-13` (two user tickets) is still
     refused.
- **Recommendation:** option 2. It keeps the point of 0031 (agents can't rewire the user's tickets) and lets agents
  file follow-ups that wait for their own work. It would be a refinement of 0031.
- **Deferred: built as option 1.**

## Q18 · Do Crewboard's own env-file copies count as "uncommitted files"?

- **Where:** decision 0050 ("`crewboard review submit` **refuses while the worktree has uncommitted changes** ('2 files
  are not committed. Commit them, or discard them if they're scratch files')") and SDD REV-07, together with SES-01
  step 4 / d0010 (Crewboard copies the project's env files, for example `.env.local`, into every worktree). Tickets:
  T-0072, T-0073.
- **Problem:** in a repository whose `.gitignore` doesn't list `.env.local`, the copy shows up as an untracked file. The
  agent can't submit until it commits it (a secret in the history, which 0050 warns about) or deletes it (the app then
  runs without its env).
- **Options:**
  1. Count every file, as 0050 reads literally. Example: SHOP-19's submit is refused with "1 file is not committed"
     because of `.env.local`.
  2. Leave out the project's configured env files (Settings → Env files): they are Crewboard's copies, not the agent's
     work. Example: SHOP-19 submits; `.env.local` stays uncommitted in the worktree. Other untracked files still count.
- **Recommendation:** option 2 (a refinement of 0050). The Commit skill tells the agent never to commit env files, so
  option 1 blocks every such ticket until the user adds the file to `.gitignore`. Option 2 is one filter in
  `board/review.rs` (`submit_for_review`).
- **Deferred: built as option 1** (it follows 0050 as written); the Commit skill tells the agent to ask the user
  instead of committing an env file.

---

## New open questions (M5 implementation, 2026-10-10)

## Q19 · How does a Queued card's waiting text name a parent's progress?

- **Where:** decision 0049 ("Queued cards say what they wait for: **'waits for SHOP-18 to merge (in Review)'**") and
  ticket T-0077 ("waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"), against `crewboard.pen → 04 · Ticket card
  variants → Queued · waits for merge`, which reads "waits for SHOP-18, SHOP-19 to merge **· SHOP-18 in Review**".
  Tickets: T-0077, T-0083.
- **Problem:** the same line appears on the card, in the panel's notice, in the Stack list and in `crewboard blocked`,
  so it has one format. The decision and the ticket put the progress in parentheses; the frame uses a middle dot.
- **Options:**
  1. Parentheses, as 0049 and the ticket: "waits for SHOP-18, SHOP-19 to merge (SHOP-18 in Review)"; one parent:
     "waits for SHOP-19 to merge (in Review)"; a PR: "waits for SHOP-18 · PR #482 open" (the storyboard's own text).
  2. The frame's middle dot: "waits for SHOP-18, SHOP-19 to merge · SHOP-18 in Review"; one parent: "waits for SHOP-19
     to merge · in Review".
- **Recommendation:** option 1 (it follows the decision; the design changes one string). Either way it is one function,
  `ReadyState::reason` in `crewboard-core/src/board/readiness.rs`.
- **Deferred: built as option 1.**

---

## New open questions (M6 implementation, 2026-10-10)

## Q20 · Close epic when some children are already merged: Closed, or Ready for Review?

- **Where:** decision 0044 item 2 ("Closing an epic with unfinished tickets: a **single confirmation** lists the
  unfinished child tickets and any agents that will be stopped, then **closes them all**. Merged children stay
  merged.") and item 3 ("An epic with nothing merged: if every child ends Closed, the epic shows 'Closed · not
  merged'"), together with SDD EPC-04 ("When **every child is Done** (merged, or closed with at least one merged), the
  epic moves to **Ready for Review**"). Tickets: T-0100, T-0105 (and M2's T-0025, which already closes the epic).
- **Problem:** SHOP-40 has SHOP-40.1 merged into its branch and SHOP-40.2 still working. The user picks **Close
  epic…**: SHOP-40.2 is closed and its agent stopped. Now every child is Done with one merged, which by EPC-04 is the
  condition for Ready for Review. The decisions don't say whether the epic itself then ends Closed (the user asked to
  close it) or goes to Review (its merged work could still be delivered).
- **Options:**
  1. The epic ends **Done · Closed · not merged**: "Close epic" closes the epic too; its merged work stays on its
     branch (kept), never reaches the base. Example: SHOP-40 shows "Closed · not merged" in Done; Reopen brings it back.
  2. The epic goes to **Ready for Review** when at least one child is merged: "Close epic" only closes the unfinished
     children. Example: SHOP-40 moves to Review with SHOP-40.1's change; the user approves or closes it again.
- **Recommendation:** option 1. The user's click said "close this epic"; option 2 would make "Close epic" behave
  like "close the remaining children" and need a second close. Either way it is one line in `Review::close_epic`
  (`crewboard-core/src/review/close.rs`).
- **Deferred: built as option 1.**

## Q21 · Which round does the card's "Sent back · N comments · round K" name?

- **Where:** decision 0034 ("The card shows a **'Sent back · N comments'** marker (and the round, for example 'round
  2') while the agent works on the feedback") and ticket T-0085 ("the round number starts at 1 and is incremented by each
  Send back; `sent_back_round` on the ticket matches"). Sample SHOP-27: "Sent back · 3 comments · round 2".
- **Problem:** the first review is round 1, and its comments are sent by the first Send back. The decision's "round 2"
  can mean either the second Send back (the comments of review round 2), or the first Send back (the agent now works on
  round 2).
- **Options:**
  1. **K = the Send back's number**: the comments it carried belong to round K. Example: SHOP-27's first Send back
     shows "Sent back · 3 comments · round 1"; after a second review and Send back, "round 2".
  2. **K = the attempt the agent is on**: one more than the Send backs. Example: the first Send back shows "round 2".
- **Recommendation:** option 1: the comments and the card name the same round, and `review feedback` prints "feedback
  from round K" for exactly those comments. One expression in `board/tickets.rs` (`SentBack.round`).
- **Deferred: built as option 1.**

---

## New open questions (M7 implementation, 2026-10-10)

## Q22 · Clicking a macOS notification: the personal build has no app bundle

- **Where:** decision 0062's open item "How the personal build is installed before Crewboard is first used (PATH,
  plugin folder, **app bundle for notifications**)", SDD ATT-11 ("Clicking opens the ticket with its terminal
  focused") and ticket T-0117 ("clicking opens the app, the ticket's panel and focuses its terminal"). Packaging is out
  of scope (SDD §3.2).
- **Problem:** macOS lets a program post notifications that open it on click only from an app bundle with a bundle
  identifier (`UNUserNotificationCenter`). The personal build is plain binaries (`crewboard`, `crewboard-app`). From a
  plain binary, `osascript`'s `display notification` works, but the notification belongs to Script Editor: clicking it
  opens Script Editor, not SHOP-23.
- **Options:**
  1. **Post with `osascript` now; the click does nothing useful until installation is decided.** Example: "shop-web ·
     SHOP-23 asks a question" appears; clicking it shows Script Editor; the user switches to Crewboard (⌘J opens the
     item). Every notification already carries its ticket (`OsNotification::open`), and posting goes through one
     trait (`attention::notify::Notifier`), so a bundle-based notifier replaces one type.
  2. Decide the installation now and build a minimal `Crewboard.app` bundle around `crewboard-app` with a
     notification-click handler. Example: clicking opens Crewboard on SHOP-23 with its terminal focused. This is
     packaging work the milestones don't plan, and d0062 leaves it open.
- **Recommendation:** option 1 for v0.1's personal build; settle the bundle when installation is decided (d0062's
  open item), then add a notifier that opens `open` on click.
- **Deferred: built as option 1.**

## Q23 · Is an orange ticket interrupted by a restart part of Resume all?

- **Where:** decision 0045 ("**Dev · orange (Asks you, Needs you, Idle; 0042):** **Interrupted, but the label and question
  text are kept.** … **Resume all** covers only the Dev tickets above.") — "above" lists Dev · working and Dev · orange.
  The frame *10 · Needs-you inbox → After a reboot · answer resumes (0045)* says instead: "Answering from the inbox
  resumes the same Claude conversation and delivers the answer in one click. **It is not part of Resume all.**" Tickets
  T-0112, T-0118.
- **Problem:** after a reboot SHOP-25 (working) and SHOP-23 (asked "Should fullName be split everywhere?") are both
  Interrupted. Should "Resume all" resume SHOP-23 too?
- **Options:**
  1. **Yes (the decision):** Resume all resumes every Interrupted ticket. SHOP-23's conversation comes back without an
     answer; its question is on screen in its terminal and the user answers there (or in the inbox). The inbox shows
     "Resume all (2)".
  2. **No (the frame):** Resume all skips Interrupted tickets that kept an orange label; they resume only through their
     answer. The inbox shows "Resume all (1)".
- **Recommendation:** option 2 reads better (resuming a question without its answer spends a resume for nothing), but
  the decision says option 1, so it is built as option 1 until the decision is refined. Either way it is one filter:
  `recovery::resume_all_eligible` in core and `InboxModel::resume_all_keys` / `resume_all_confirm` in the app.
- **Note:** following the frame (and "the label is kept"), such a ticket is listed with the orange items (group 1) with
  "session ended by restart · answering resumes it", not in the interrupted group. The frame *16 → Opening after a
  reboot* agrees with option 2 too ("NOT IN RESUME ALL · Orange tickets keep their question — SHOP-22, SHOP-23,
  SHOP-12").
- **Deferred: built as option 1.**

## Q24 · Are Cleanup items preselected?

- **Where:** decision 0018 ("a **Cleanup view** lists orphaned sessions, worktrees and branches that no ticket owns, plus
  leaked ports. **The user selects them** and confirms. Nothing is deleted automatically") and ticket T-0123 ("nothing is
  checked by default"; "Nothing is preselected"). The frame *16 → Cleanup · 3 items* shows the orphaned session and the
  orphaned worktree already checked ("2 selected · 412 MB"); only "the Done ticket with uncommitted changes is flagged and
  unselected by default".
- **Problem:** opening Cleanup with SHOP-14's orphaned worktree listed: is its box already ticked?
- **Options:**
  1. **Nothing is ticked** (the ticket, and "the user selects them"): Delete selected… stays disabled until the user ticks
     a row or a group. Example: "Nothing selected"; the user ticks Worktrees → "1 selected · 412 MB".
  2. **Everything except kept worktrees is ticked** (the frame): one click on Delete selected… after opening.
- **Recommendation:** option 1: deleting is the one place where a default should do nothing, and the group checkbox makes
  ticking everything one click. One line in `CleanupModel::set_items` (`crewboard-app/src/view_models/cleanup.rs`).
- **Deferred: built as option 1.**

## Q25 · Start from scratch after a failed Send back resume: the ticket prompt or the review skill?

- **Where:** decision 0018 ("Start from scratch is a new `claude` session in the same worktree, after a confirmation") and
  the frame *16 → Note for the implementing agent* ("Start from scratch runs a new claude session in the same worktree
  **with the ticket prompt**"); decision 0034 (Send back returns the ticket with the comments for the agent). Tickets
  T-0086, T-0112.
- **Problem:** SHOP-27 is sent back; its ended conversation can't be found (Resume failed). The user picks Start from
  scratch. The new conversation starts with `/crewboard:work-ticket`, not `/crewboard:address-review`, so it isn't told
  to read the review comments first (they stay readable with `crewboard review feedback SHOP-27`). The same holds for an
  inbox answer that waited for a resume: the question it answered belonged to the old conversation.
- **Options:**
  1. **The ticket prompt (d0018, the frame):** Start from scratch drops the pending skill or answer. Example: the new
     agent runs work-ticket and sees the ticket; the comments wait in `review feedback`.
  2. **Keep the Send back's skill:** a fresh conversation started with `/crewboard:address-review`. Example: the new
     agent reads the round's comments first, without the earlier conversation.
- **Recommendation:** option 2 for a Send back (the comments are the reason it is back in Dev) and option 1 for an inbox
  answer; but the decision and the frame say "the ticket prompt", so option 1 is built. One `CASE` in
  `Review::bring_back` (`crewboard-core/src/review/resume.rs`).
- **Deferred: built as option 1.**

---

## New open questions (M8 implementation, 2026-10-10)

## Q26 · Which tokens make up "182k tokens" on a card and "1.84M tokens today"?

- **Where:** decision 0020 ("It shows **tokens used** … Tokens per card, and today's total in the board header"), SDD
  USG-01 and ticket T-0125 ("records tokens in/out/cache per session and day"). The frames show "active 40 s ago · 182k
  tokens" on a card, "Session ended · 11:02 · 41 min · 226k tokens" in the terminal and "1.84M tokens today" in the
  header. No decision says which of Claude Code's four counts (input, output, cache write, cache read) the one number
  adds up.
- **Problem:** every request of a Claude Code session re-reads the whole conversation from the prompt cache, and those
  **cache reads** are by far the largest count. A real 70-minute session (Claude Code 2.1.296, 70 requests) has about
  66k input + output tokens, 353k cache-write tokens and 20.3M cache-read tokens. Counting cache reads makes the card
  say "20.7M tokens"; leaving them out says "419k", the size the frames show.
- **Options:**
  1. **Input + output + cache writes** (cache reads left out of the shown number; they are still stored, and the
     optional dollar estimate prices all four). Example: SHOP-18's card "419k tokens"; header "1.84M tokens today" for
     a few agents' day.
  2. **All four counts** (what the API bills and tools such as `ccusage` show as "total tokens"). Example: SHOP-18's
     card "20.7M tokens"; header "90M tokens today".
- **Recommendation:** option 1: it measures the work the agent did and new context it read, it keeps the numbers the
  size the design expects, and the cost of the re-reads is visible in the dollar estimate for those who want it. Either
  way it is one function: `Tokens::shown` (`crewboard-core/src/usage/mod.rs`); the `usage` table keeps all four
  counts.
- **Deferred: built as option 1.**

## Q27 · Claude Code now continues on its own when a usage limit resets

- **Where:** decision 0020 ("After the reset, the user clicks **Resume all**. Nothing restarts on its own (0018)"), decision
  0046 item 3 ("After the reset, **Resume all** (the user's click) tells those sessions to continue"), SDD USG-06 and
  tickets T-0129, T-0131. Against: the Claude Code docs, "Interactive mode → Wait for a usage limit to reset"
  (code.claude.com/docs/en/interactive-mode, read 2026-10-10; Claude Code ≥ 2.1.234, installed 2.1.296): "When a
  claude.ai usage limit stops Claude mid-task, Claude Code waits in the open session and **continues the task on its own
  after the limit resets**. Automatic continue is on by default in interactive sessions signed in with a claude.ai
  subscription", controlled by the user's setting `autoContinueAtUsageLimit`. It does not wait on its own for a reset more
  than 24 hours away (a weekly limit), after a sleep of more than about 30 minutes, or after the user pressed Esc.
- **Problem:** the decisions were made before this Claude Code feature. At 3:45 pm, SHOP-18, SHOP-19 and SHOP-22 (all
  "Waiting for limit reset") continue by themselves, unless the user turned the setting off. Crewboard's "Resume all"
  would then type "continue" into agents that are already working again (it waits for an idle, empty input line, so it
  would arrive after their next turn ends, as a stray prompt).
- **Options:**
  1. **Respect the user's Claude Code setting; Crewboard itself still starts nothing.** The continued turn fires
     `UserPromptSubmit`, so the ticket goes back to Working by itself (Claude Code did it, not Crewboard). Resume all
     covers only the agents still waiting (a weekly limit, the setting off, a long sleep, Esc). Example: at 3:45 pm SHOP-18
     and SHOP-19 continue on their own; SHOP-22, whose user pressed Esc, stays "Waiting for limit reset" and the banner's
     "Resume all (1)" types "continue" into it.
  2. **Turn Claude Code's automatic continue off** for agents (`"autoContinueAtUsageLimit": false` in Crewboard's
     `--settings` file), so nothing continues until Resume all. Example: at 3:45 pm all three wait; the banner shows "Resume
     all (3)". This changes how `claude` behaves for the user (P5, d0012) and overrides their own setting (P4, d0011).
- **Recommendation:** option 1: the decision's intent ("Crewboard never restarts anything without a click") still holds,
  the user's own Claude Code setting decides, and no stray "continue" is typed. Either way it is small: option 2 is one key
  in `session::statusline::settings_json`; option 1 is the `UserPromptSubmit` rule for Waiting for limit reset in
  `signals::state::transition` and the eligibility filter of `usage.resume_all`.
- **Deferred: built as option 1.**

---

## New open questions (M9 implementation, 2026-10-10)

## Q28 · Does a Run's output survive a restart?

- **Where:** decision 0045 ("**Run and Shell processes:** show **"stopped by restart"**" and "Crewboard does **not** copy
  terminal output to disk"), ticket T-0133 ("stdout/stderr go to a bounded log kept by the service"). Against: the frame
  *13 · Run & Shell → Stopped by restart* shows the old output above the marker ("POST /api/checkout 200 in 212ms",
  "[crewboard] machine restarted", "[crewboard] stopped by restart").
- **Problem:** the service keeps a Run's output in memory (the session's scrollback, like every terminal). A restart
  empties that memory, so the old lines can only come back if Crewboard writes Run output to a file, which d0045 rules
  out for terminal output.
- **Options:**
  1. **Memory only** (d0045). After a reboot SHOP-19's Run tab says "Stopped by restart · was running at 08:24" with an
     empty output area; Run starts it again. While the service runs, a stopped or failed Run keeps its output (the
     frames *Stopped* and *Failed to start* are unaffected).
  2. **A log file per Run** (`~/.crewboard/logs/run/SHOP-19.log`, replaced at each Run, capped at about 1 MB), read back
     after a restart, as the setup command's log already is. The frame is matched exactly.
- **Recommendation:** option 1: the decision says Crewboard keeps no terminal output on disk, and a dev server's output
  from before a reboot rarely matters. Option 2 is small if wanted (one writer task next to `Runs::watch` in
  `crewboard-core/src/terminals/run.rs`, and `run.status` returning the file's tail).
- **Deferred: built as option 1.**

## Q29 · Does the Project terminal notice a `claude` the user started in it?

- **Where:** decision 0019 ("The user creates and splits tickets and epics there with `crewboard` commands (full
  access, 0031), runs any project command, or starts `claude` themselves"), ticket T-0138 ("It doesn't start Claude
  … the user may run `claude` there themselves"). The frames *17 · Project terminal* go further: the drawer header
  says "claude running" or "claude · waiting for you"; *Ended* shows the line "claude exited 11:20 · 38k tokens ·
  created 4 tickets · the shell stays open"; brief 17 says new tickets are "marked "created by agent" when Claude
  made them"; and the header has a "crewboard attach plan" chip. No decision or ticket asks Crewboard to watch what
  runs inside the user's own terminal.
- **Problem:** the terminal is the user's plain shell. To know that `claude` runs in it, Crewboard would have to
  look at the shell's child processes (or read Claude Code's files for that folder). Callers inside it are the user
  (CLI-03), so a ticket Claude created there is recorded as created by the user.
- **Options:**
  1. **A plain terminal.** The header says Shell / Ended / Stopped by restart; tickets created there are the user's
     (created by `nevzat`), listed in the drawer's summary (T-0167). Example: the user runs `claude`, asks it to
     split the checkout feature; four tickets appear in Backlog with the short highlight and in the summary; the
     header keeps saying "Shell".
  2. **Notice `claude` by process ancestry.** When a caller's ancestry between it and the terminal's shell contains
     a `claude` process, the ticket is marked "created by agent" (shown as Claude, still with the user's full
     access), the header shows "claude running" while such a process lives, and an "claude exited · created 4
     tickets" line is added when it ends (tokens from the conversation file, like USG-01). Example: the same split
     shows "created by agent" on the four cards and "claude running" in the header.
- **Recommendation:** option 1 for v0.1: it matches the decision's "plain terminal" and the ticket's "Done when";
  option 2 is additive later (one ancestry check in `caller::project_terminal_of` and a 1 s check of the shell's
  children). The attach chip is left out either way (`crewboard attach` takes a ticket key).
- **Deferred: built as option 1.**

## Q30 · Does Park stop a ticket's Run?

- **Where:** decision 0052 ("Park: drag Dev → Backlog … its ports are freed (0026)"), decision 0026 ("Run and Shell get a
  second one (for example 4020) … both are freed at Done"), decision 0035 ("Run … stops at Done") and SDD RUN-01 /
  CLS-06. No decision or frame (*13 · Run & Shell*) says what happens to a running Run when its ticket is parked;
  the frames show a toast only for *Reaching Done stops it*.
- **Problem:** Park returns the Run port to the pool. A Run that keeps listening on it holds a port the pool may hand
  to another ticket, whose Run then fails with "Port 4020 is in use".
- **Options:**
  1. **Park stops Run.** Before the ports are freed, the Run stops like at Done (its process group ends; the log's
     last line "[crewboard] stopped · SHOP-19 was parked"; the Run tab "Stopped at 11:48 · SHOP-19 was parked"). No
     toast (the Park confirmation already says the ticket's work stops). The Shell holds no port and stays open.
     Example: SHOP-19 runs `pnpm dev` on 4020; the user parks it; 4020 is free and SHOP-24's Run can get it.
  2. **Park leaves Run running.** The user stops it from the Run tab or the header's processes menu; Cleanup lists
     it as a leftover session of a Backlog ticket. Example: after parking SHOP-19, `pnpm dev` still answers on 4020
     while 4020 is back in the pool.
- **Recommendation:** option 1: freeing the ports means nothing still uses them.
- **Deferred: built as option 1 (Park stops Run).**

## Q31 · Which icon stands for GitHub / Azure DevOps on cards and in the PR dialog?

- **Where:** *04 · Ticket card variants → Approved · PR open, Review · PR rework* and *07 → Open PR · confirmation* use a
  `github` icon next to "GitHub" (and the PR provider row in *15 · Settings*). No decision names an icon; d0050 only says
  the provider is detected from the remote.
- **Problem:** the `github` brand icon no longer exists in Lucide (brand icons were removed), and Lucide never had an
  Azure DevOps one. Crewboard's icon set is Lucide only (T-0015).
- **Options:**
  1. **`git-pull-request` for every provider.** The meta line reads "⟟ GitHub" / "⟟ Azure DevOps"; the word names the
     provider. Example: SHOP-26's card shows the pull-request glyph and "GitHub".
  2. **Bundle the providers' own marks** (GitHub's Octicon mark, the Azure DevOps logo) as extra SVGs next to Lucide.
     Example: SHOP-26 shows the Octocat; BILL-5 the Azure DevOps logo. Needs their brand terms checked.
  3. **`github` from an older Lucide release** for GitHub, `git-pull-request` for Azure DevOps. Example: two different
     glyphs for the same idea.
- **Recommendation:** option 1: neutral, no brand assets to license, and the label already says which provider.
- **Deferred: built as option 1.**

## Q32 · Does the Send back dialog count a PR's comments?

- **Where:** decision 0017 ("The agent then pulls the PR comments **on demand** through get_review_feedback"; "PR
  feedback never re-queues a ticket automatically"), T-0148 ("no PR data is fetched except during this command"), and
  *07 → Send back · ticket with an Azure DevOps PR (BILL-5)*, which shows "PR !212 on Azure DevOps · 4 comments" and a
  checkbox list "Azure DevOps PR comments 4" in the dialog.
- **Problem:** a count in the dialog means Crewboard fetches the PR's comments when the user opens Send back, which is
  a second place that reads PR data (T-0148 allows only `review feedback`).
- **Options:**
  1. **No count, no fetch in the dialog.** The dialog says "PR #212 on Azure DevOps · its unresolved comments are
     fetched when the agent reads its feedback (`crewboard review feedback`) and added below your line comments."
     Example: BILL-5's Send back shows that note; the agent's `review feedback` lists the 4 comments.
  2. **Fetch once when the dialog opens** (the user's click) to show "4 comments" and the checkbox; the agent still
     fetches them again later. Example: opening Send back on BILL-5 runs `az devops invoke` once.
- **Recommendation:** option 1: it keeps "fetched on demand, by the agent" literal and adds no second network call;
  the frame's count can come later if wanted.
- **Deferred: built as option 1.**

## New open questions (M11 implementation, 2026-10-10)

## Q33 · Which shell runs the user's commands and terminals on Windows?

- **Where:** T-0156 ("The agent launch, Shell, Project terminal and Run run the user's default shell where
  relevant"), SDD §19 (RUN-01 "a project-level run command, e.g. `pnpm dev --port $PORT`"; RUN-05/06 "the user's
  shell"), review-M9 note 2. No decision names a Windows shell; Windows has no single "login shell" like `$SHELL`.
- **Problem:** on macOS and Linux the setup and Run commands go to `/bin/sh -c` and the Shell / Project terminal run
  `$SHELL -l`. Windows has `cmd.exe` (`%COMSPEC%`), Windows PowerShell, PowerShell 7 (`pwsh`) and, for Claude Code
  users, Git Bash. `$PORT` in a Run command means nothing to `cmd` (it writes `%PORT%`) or PowerShell (`$env:PORT`).
- **Options:**
  1. **`cmd.exe /d /c` for the setup and Run commands (as `npm` runs scripts), PowerShell for the Shell and the
     Project terminal (`pwsh` when installed, else Windows PowerShell), Git Bash for the status line (as Claude Code
     runs it).** Crewboard fills in `$PORT` / `${PORT}` in a Windows Run command itself (the variable is also set).
     Example: SHOP-19's Run `pnpm dev --port $PORT` runs `pnpm dev --port 4020` in `cmd`; its Shell tab opens
     PowerShell in the worktree.
  2. **Git Bash everywhere** (setup, Run, Shell, Project terminal). Commands are written like on macOS. Example: the
     Shell tab opens `bash` in the worktree; a user without Git Bash gets no Shell.
  3. **A per-project setting** (cmd / PowerShell / bash) for commands, and a global one for terminals.
- **Recommendation:** option 1: the Windows defaults a Windows developer expects, nothing to configure; option 3 can
  come later if wanted.
- **Deferred: built as option 1.** (The command runs as `cmd /d /s /c "<command>"`, so `%PORT%` works as well.)

## Q34 · How do OS notifications look and open the ticket on Windows and Linux?

- **Where:** T-0159 ("notifications fire only for blocking items … clicking opens the ticket with its terminal
  focused"), frame *11 · Banners, toasts & OS notifications → OS notifications* (Windows 11 toast titled "Crewboard"
  with "Open terminal" / "Dismiss"; Linux "Uses the desktop's notification service (freedesktop)"), T-0159 "Crewboard
  still installs nothing", and Q22 (macOS: a click can't open a ticket without an app bundle).
- **Problem:** a Windows toast shows the name of a registered app identity (AppUserModelID). Showing "Crewboard"
  needs a Start-menu shortcut or a registry entry (`HKCU\Software\Classes\AppUserModelId\…`), i.e. Crewboard writing
  into the user's system. A click must also reach Crewboard: the app may not even be running.
- **Options:**
  1. **Windows: a toast shown through Windows PowerShell's own identity** (no registration; the toast's header says
     "Windows PowerShell", not "Crewboard"), with "Open terminal" and "Dismiss". **Linux: `notify-send` with an
     "Open" default action.** The service waits for the click (up to an hour) and then tells the running app to come
     forward on the ticket with its terminal focused, or starts the app on it (`CREWBOARD_OPEN=SHOP-22`) when no app
     is connected. A grouped notification opens the inbox. Example: clicking "shop-web · SHOP-22 needs permission"
     brings the window forward on SHOP-22's Terminal tab.
  2. **Register a "Crewboard" identity on Windows** (one `HKCU` registry key, written on first notification) so the
     toast says "Crewboard" exactly as the frame. Same click handling.
  3. **No click handling** on Windows and Linux, as on macOS (Q22).
- **Recommendation:** option 1 for v0.1: nothing written outside the data folder; the header name is the only
  difference from the frame. Option 2 belongs with packaging (installers register an identity anyway, d0021's open
  item).
- **Deferred: built as option 1, with one change found in the Windows VM:** Windows hands a toast's click only to
  the app registered for its identity (an AUMID with a COM activator), so with PowerShell's identity a click never
  reaches Crewboard. The Windows toast therefore shows the title and text without "Open terminal" / "Dismiss" (a
  button that can't act would mislead), and, as on macOS (Q22), the user opens the ticket from the app. Linux
  clicks open the ticket as described. The click on Windows comes with option 2 (or packaging).

## Q35 · VM access for M11 (blocker, 2026-10-10)

- **Where:** the M11 assignment ("Take a snapshot of each VM before you change anything … Don't revert or delete
  snapshots, stop or force-off VMs"), T-0155…T-0157 and T-0160 (tests must run in the Windows and Linux VMs).
- **Problem:** neither snapshot can be taken. Windows sits at the sign-in screen (nevzatopcu's password), and VMPal
  only runs commands and takes snapshots once someone is signed in (`executionAvailable: false`). The Ubuntu VM uses
  GPU acceleration, so VMPal refuses a live snapshot: "Shut Ubuntu VM down to take a snapshot".
- **Options:**
  1. **The user signs in to Windows once** (and turns off the lock on idle), **and allows a normal shutdown of the
     Ubuntu VM** (`vmpal_stop_vm` without force) for its snapshot, after which the agent starts it again.
  2. As 1, but the user takes the Ubuntu snapshot by hand.
  3. Skip the Ubuntu snapshot (the agent only installs packages with apt and Rust with rustup in `~`).
- **Recommendation:** option 1.
- **Blocker:** no Windows or Linux build or test can run until Windows is signed in and the Ubuntu question is
  answered. The Mac-side code for T-0155…T-0159 is written and `scripts/check.sh` is green on macOS, uncommitted.
- **Resolved (2026-10-11, user): no snapshots needed; the VMs are clean and hold no data. The user signed in to Windows.**

## Q36 · The remaining M12 UI tickets after decision 0074 (blocker, 2026-10-11)

- **Where:** decision 0074 (the UI moves to Tauri before v0.1, GPUI dropped), raised by the M12 implementer after T-0192.
- **Problem:** T-0193–T-0197 and T-0203–T-0205 are written for the GPUI app, and T-0192 was already built in GPUI.
- **Options:** 1. keep finishing M12's UI in GPUI; 2. build only the core, CLI and plugin work now and hold the UI parts
  for the Tauri rebuild.
- **Resolved (2026-10-11, user): option 2.** Core, CLI and plugin work goes first (T-0198–T-0202, plus the core parts of
  T-0193–T-0197 and T-0205; brief: `prompts/m12-core.md`). After that comes the UI migration. The UI parts and T-0206
  are done on the new UI. Recorded in 0074 and 0053.

## New open questions (M14 ticket writing, 2026-10-12)

Raised while writing the M14 tickets (T-0207–T-0247) from the test build's report (`m14-test-build-report.md`, branch
`m14-test-build`) and decisions 0074 / 0075. Each one is built as its option 1 until the user answers.

## Q37 · Which terminal renderer does Linux use by default?

- **Where:** decision 0075 point 4 ("xterm.js with its WebGL renderer"), SDD UIT-05, and the test build's report: in the
  Ubuntu VM, WebGL on the VM's virtual GPU drew the terminals **blank or as garbage, without any error**; WebGL on
  software GL drew correctly but used 3.5 cores; xterm's DOM renderer drew correctly at 1.3 cores but fell to about 30 fps
  (worst second 22) while the board scrolled and a card was dragged, with 45 ms median typing echo. Real Linux hardware
  wasn't tested. Ticket: T-0216 (the renderer setting), T-0247 (the real-hardware run).
- **Problem:** the page can't tell that WebGL is drawing garbage, so an automatic fallback can't catch it. A Linux user
  whose driver is bad would see empty terminals for SHOP-22 and not know why.
- **Options:**
  1. **DOM on Linux, WebGL on macOS and Windows** by default; the setting switches either way. Example: on a ThinkPad
     with Ubuntu, SHOP-22's terminal draws with the DOM renderer; a user with a good GPU can pick WebGL.
  2. **WebGL everywhere** (0075 as written); a Linux user with garbage switches to DOM in Settings. Example: the same
     ThinkPad shows WebGL; if the terminal is blank the user must find the setting.
  3. **Decide after T-0247's run on real Linux hardware**, building option 1 until then.
- **Recommendation:** option 1: a slower but correct terminal is better than a fast blank one, and the setting keeps WebGL
  one click away. Revisit with T-0247's numbers (option 3).
- **Deferred: built as option 1.**

## Q38 · Where is the terminal-renderer setting shown?

- **Where:** T-0216 / T-0234 (the setting from Q37). No decision, brief or frame has a renderer setting; *15 · Settings →
  Global · Usage, notifications, data, appearance* has the Appearance section with the theme picker.
- **Problem:** the test build recommends a setting, but nothing says where it lives or whether the user sees it.
- **Options:**
  1. **A global setting stored with the other settings (SET-01), one row under Appearance:** "Terminal renderer: WebGL
     (faster) · DOM (safer)", drawn in the style of the section's other rows. Example: a user whose SHOP-22 terminal is
     blank opens Settings → Appearance and picks DOM; every terminal redraws.
  2. **An environment variable only** (`CREWBOARD_TERMINAL_RENDERER=dom|webgl`), no UI. Example: the same user has to
     relaunch the app from a shell with the variable set.
- **Recommendation:** option 1: a user with a broken driver needs a visible fix; it is one row and needs a design check
  later.
- **Deferred: built as option 1.**

## Q39 · When does the app set `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux?

- **Where:** the test build's report: in the Ubuntu VM the window showed its first frame and then never repainted until
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` was set (a known WebKitGTK problem with some GPU drivers, VMs among them). Tickets:
  T-0214 (sets it), T-0247 (real hardware).
- **Problem:** the variable fixes the freeze but turns off WebKitGTK's DMABUF path, which may cost speed on real hardware
  where it works. "When needed" can't be detected from inside the page.
- **Options:**
  1. **Always on Linux, unless the user's environment already sets the variable** (so `WEBKIT_DISABLE_DMABUF_RENDERER=0`
     turns it off). Example: the app starts on Ubuntu, sets the variable, and the window repaints in the VM and on a laptop.
  2. **Only in a detected VM** (for example `systemd-detect-virt`), otherwise unset. Example: a laptop whose driver
     also has the bug would freeze.
  3. **Never by default;** document it. Example: a frozen window on first launch until the user reads the notes.
- **Recommendation:** option 1: a frozen window is the worst first impression, and the user can opt out; revisit with
  T-0247's real-hardware run.
- **Deferred: built as option 1.**

## Q40 · What does the app show when the service can't start (for example a socket path that is too long)?

- **Where:** the test build's report ("the service fails to start when `CREWBOARD_HOME` is a long path ('path must be
  shorter than SUN_LEN'), and the app then just waits"), SVC-04 / UI-07, *11 · Banners, toasts & OS notifications →
  Banners → Background service stopped* (the only frame for a missing service; it shows Start service, no reason).
  Tickets: T-0211, T-0231.
- **Problem:** no frame shows a reason why the service couldn't start. Example: `CREWBOARD_HOME=/Users/me/Library/…/a/very/long/path`
  makes the socket path exceed macOS's 104-byte limit; the user sees "Reconnecting…" forever.
- **Options:**
  1. **The "Background service stopped" banner with one more line giving the reason** from the failed start, e.g.
     "Couldn't start: the data folder's path is too long for a socket (macOS allows 104 characters). Use a shorter
     CREWBOARD_HOME." and Start service to try again.
  2. **A full-window error** in place of the board, with the same text.
- **Recommendation:** option 1: it reuses the existing banner and keeps the board visible; the wording needs a design
  check later.
- **Deferred: built as option 1.**

## Q41 · When do the view models move into `crewboard-view`?

- **Where:** decision 0075 point 8 ("Groundwork: the new crate `crewboard-desktop` … and the `crewboard-view` crate";
  "Screens, one ticket per design-brief area, each moving its view models"), SDD UIT-04, the M14 ticket brief
  ("the `crewboard-view` crate (the view models moved out of `crewboard-app`; the GPUI app now depends on it)").
  Tickets: T-0207 and every screen ticket.
- **Problem:** "each [screen] moving its view models" can be read as each screen ticket moving its own files out of
  `crewboard-app`. Done that way, every screen ticket would also edit the frozen GPUI app, and screens built side by side
  would collide in the same GPUI files and module lists.
- **Options:**
  1. **All view models move at once in the groundwork (T-0207);** a screen ticket then makes its view models reach the page
     (the `ts-rs` export, its request module, the React drawing). Example: `drag.rs` moves in T-0207; T-0220 exports
     its verdict type and draws it.
  2. **Each screen ticket moves its own view models** from `crewboard-app` to `crewboard-view` and updates the GPUI app's
     imports. Example: T-0220 moves `drag.rs` and `board.rs` and fixes `crewboard-app/src/board.rs`.
- **Recommendation:** option 1: the frozen GPUI app is touched once, and screens can be built side by side.
- **Deferred: built as option 1.**
- **Resolved (2026-10-12, user): option 1.** All view models move in T-0207; screen tickets connect them to the page. Recorded in 0075 point 8.

## Q42 · Does the delete confirmation's title name the ticket's title?

- **Where:** TKT-08, decision 0060 ("A confirmation is required (a deletion, 0018). If other tickets depend on it, the
  confirmation names them and those links are removed."; for an epic: "Delete SHOP-40 and its 2 tickets (SHOP-40.1,
  SHOP-40.2)? None of them started."). Frames: *05 · Ticket side panel → Delete ticket · confirm* (and its Dark copy,
  *Dark · options & delete*) say "Delete SHOP-21?"; *17 → Undo a created ticket*, as T-0162 built it, says "Delete
  SHOP-21 Order confirmation email?". `design-briefs/CHANGES-2026-10-09-i.md` asks for "Delete SHOP-21?". Tickets:
  T-0162 (done, with the title), T-0221 ("Delete asks with the frame's text"), T-0243.
- **Problem:** the one delete dialog (the card menu, the panel's "…" menu and the Project agent's summary all open it)
  can only match one of the two frames. The decision doesn't say.
- **Options:**
  1. **Key and title:** "Delete SHOP-21 Order confirmation email?" everywhere, as `delete.rs` words it since T-0162.
     The user sees which ticket goes without remembering numbers (the Project agent's summary lists several new ones).
  2. **Key only:** "Delete SHOP-21?" everywhere, as the 05 frames and the design brief; the epic's title stays as 0060
     words it.
- **Recommendation:** option 1: the title costs nothing and guards against deleting the wrong number; the 05 frames
  would get the title in a later design pass. Option 2 is one line in `delete.rs` and its tests.

## Q43 · Where does the panel's Park confirmation live?

- **Where:** T-0222 ("the dialogs: Close ticket …, Submit for review, Park, Reopen on a Closed ticket"; "Redoes the hand
  checks of … T-0107 (Park confirm)"); T-0107 and decision 0052 ("Dragging a Dev card to Backlog is allowed and opens the
  Park confirmation"); T-0220 ("Drops: Park (Dev → Backlog, with its confirmation)").
- **Problem:** the panel has no Park button or menu item: `panel_actions.rs` and *05 → Action bar · every state* offer
  none, and d0052 / T-0107 reach Park only by dragging a Dev card to Backlog, which is the board's drop (T-0220). The
  M12 *Close / Park · while validating* dialog is T-0236's.
- **Options:**
  1. **Park stays on the board's drop** (T-0220 opens the confirmation with `close::park_text`); the panel adds no Park
     trigger. Example: SHOP-25 open in the panel; dragging its card to Backlog asks "Park SHOP-25?", the panel has no
     Park button.
  2. **The panel gets a "Park…" item in its "…" menu** for Dev tickets, opening the same confirmation. Example: SHOP-25's
     "…" menu lists Park… above Close ticket….
- **Recommendation:** option 1: it follows d0052 and the drawn action bar and menu; option 2 adds an item no frame shows.
- **Deferred: built as option 1.**
