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
