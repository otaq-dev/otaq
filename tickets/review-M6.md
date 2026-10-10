# M6 · Review & Local merge — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0084 | Diff engine (base_sha, epics by child) | dfe382d |
| T-0085 | Review comments and rounds | 5858d09 |
| T-0086 | Send back (core) | 323e07a |
| T-0087 | `review feedback` and the address-review skill | fcb2b75 |
| T-0088 | Checkpoint commit around Crewboard rebases | e63c2e8 |
| T-0089 | Local merge in a scratch worktree | a7a73a2 |
| T-0090 | Send to agent to fix (restack skill) | 0cf26ca |
| T-0091 | Diff tab: file list and unified view | d10e404 |
| T-0092 | Diff tab: side-by-side and edge cases | fc79b9f |
| T-0093 | Line comments UI | f5f68e4 |
| T-0094 | Send back dialog | c375d84 |
| T-0095 | Approve ▾ and Local merge UI | 3616825 |
| T-0096 | Early conflict flag | a05173a |
| T-0097 | Changed since review (core) | 41684d8 |
| T-0098 | Conflict flag and changed-since-review UI | 255b621 |
| T-0099 | Done cleanup: worktree, ports, Run/Shell hooks | 80165cb |
| T-0100 | Close ticket (any state), bulk close for epics | 6c60f1c |
| T-0101 | Reopen Closed tickets, free rebase on the way to Dev | 7ae205d |
| T-0102 | Closed parent | 4dcb6bc |
| T-0103 | Submit for review by the user (core) | 84263a4 |
| T-0104 | Park and resume the same conversation (core) | a795fd2 |
| T-0105 | Epic lifecycle: to Review, Closed · not merged, Local merge of the epic | 788e063 |
| T-0106 | Close / Reopen / Submit for review UI | ba2ff0d |
| T-0107 | Park UI | e5c3672 |
| T-0108 | Parent closed on cards | 7315f9b |
| T-0109 | Epic review UI | 643099c |

Also: **aa392b1** (T-0046) — the ports tests pick their ranges below the OS's ephemeral ports;
`allocations_survive_a_restart` flaked once under the full suite (the same class as the M3 flake). Test-only.

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)        # a short path: the socket lives in it
```

**The review loop without claude.** A repository with a ticket in Ready for Review, made by hand (the agent's part is a
commit and `review submit`, which works from any shell with `CREWBOARD_TICKET` set):

```sh
mkdir -p $CREWBOARD_HOME/repo && (cd $CREWBOARD_HOME/repo && git init -q -b main && echo a > a.txt && git add . && git commit -qm init)
PATH=/usr/bin:/bin target/debug/crewboard daemon start
echo '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"'$CREWBOARD_HOME/repo'"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock
B=$PWD/target/debug/crewboard
$B -P REPO create "Checkout API" --done-when "works"       # REPO-1
$B -P REPO create "Checkout page" --done-when "works"      # REPO-2
$B dep add REPO-2 REPO-1
PATH=/usr/bin:/bin target/debug/crewboard-app
```

In the app drag REPO-1 to Dev: it stops at the launch step (Start interrupted, no claude) but has its branch and
worktree (`$CREWBOARD_HOME/wt/repo/REPO-1`). Then, in a shell:

```sh
cd $CREWBOARD_HOME/wt/repo/REPO-1 && printf 'x\ny\n' > api.ts && git add . && git commit -qm api
sqlite3 $CREWBOARD_HOME/crewboard.db "UPDATE tickets SET state='working', board_column='dev' WHERE number=1"
CREWBOARD_TICKET=REPO-1 $B review submit --stdin <<< "Adds the API"
```

(switch projects or restart the app so the board refetches). REPO-1 is in Ready for Review:

- **Diff tab**: agent summary, "diff vs main at …", file list, unified / side-by-side (side-by-side needs the
  panel expanded, ⇧⌘E, or wider than 900 px); click a line number to comment; "1 pending comment".
- **Send back** (⌘⌫): the dialog lists the comments; with no live session the resume needs claude, so without it the
  ticket ends Start interrupted. `crewboard review feedback REPO-1` prints the round.
- **Approve** (⌘↵, or drag the card onto Approved): Local merge → confirm → main gets one squash commit; your
  checkout of `repo` (clean, on main) is fast-forwarded; REPO-1 is Done · Merged, its worktree removed, its branch
  kept; REPO-2 starts if it was queued. With an edit in `repo/a.txt` the merge is refused ("repo has uncommitted
  changes on main. Clean up and retry.").
- **Conflict**: make REPO-1's branch and main change the same line → "Rebase onto main stopped — conflict" with Send to
  agent to fix; other Review tickets touching it get "Conflicts with main".
- **Changed since review**: commit again in the worktree after `review submit` → Approve shows "REPO-1 changed since you
  reviewed it · 1 new commit" with only that commit's diff; Approve anyway continues.
- **Close / Reopen / Submit for review / Park**: the panel's "…" menu; drag a Dev card to Backlog to park; drag a
  Closed card to Backlog to reopen.

**With the real claude (once):** Send back on a ticket whose session is live (the agent gets
`/crewboard:address-review`, runs `crewboard review feedback`, fixes, submits); Send back after the session ended
(`claude --resume <id> "/crewboard:address-review"`); Send to agent to fix (`/crewboard:restack`); Park and drag back
(`claude --resume <id>`); a reopened ticket (new `--session-id`, prime's "Resolve first" section after a conflict).

**Cleaning up:** `target/debug/crewboard daemon stop`, then remove `$CREWBOARD_HOME` (worktrees and the scratch
worktree live inside it; the repo only got branches `crewboard/<KEY>`; `git worktree prune` in it afterwards).

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0091 · Diff tab**
- [ ] `07 · Diff, review, approve & send back → Diff tab · SHOP-18 · unified (panel expanded)` and `Dark · diff tab`:
  the agent summary (sparkles, "no tests are run by Crewboard", folds and stays folded for that ticket), the toolbar
  ("diff vs main at a1b2c3d · 9 files · +412 −38", "4 / 9 viewed"), the 280 px file list (A/M/D/R letters coloured,
  renames "old → new", counts, viewed square → square-check), file headers ("new file", counts, Viewed), hunk headers,
  add/remove backgrounds, syntax colours (keywords purple, strings teal, functions blue, numbers amber). The tab reads
  "+412 −38". Known differences: the panel keeps its own header and action bar instead of the frame's review bar; the
  file list narrows to 200 px in a panel under 820 px; TypeScript is highlighted with the JavaScript grammar.

**T-0092 · Side-by-side and edge cases**
- [ ] `07 → Side-by-side`: old left, new right, removed runs facing added runs, blanks opposite; below a 900 px panel it
  shows Unified (the toggle keeps the choice).
- [ ] `07 → Large diff · 54 files`: "Filter files" field, folders with counts, "generated · collapsed" last; files over
  400 changed lines and lock/generated files start collapsed with "Load diff"; scrolling stays smooth.
- [ ] `07 → Binary, renamed, empty`: "Binary file changed · 184 KB → 96 KB", "Renamed with 3 changed lines (98 %
  similar). Showing only the changes.", the empty card "No changes · vs main at …" and its text.

**T-0093 · Line comments**
- [ ] `07 → Diff tab · SHOP-18 · unified` (thread and composer) and `Side-by-side`: a gutter click opens the box under the
  line; "Markdown · comments are sent to the agent only on Send back", Cancel / Add comment, ⌘↵ saves, Esc cancels;
  the thread shows You · age · pending (sent ones "sent · round N", read-only); pencil/× on pending ones; Reply. The
  "2 pending comments" pill and "Send back (2)". No gutter clicks on Done tickets or epics.

**T-0094 · Send back dialog**
- [ ] `07 → Send back · review & confirm`: ⌘⌫ and the button open "Send SHOP-18 Checkout API back", the teal intro,
  THE AGENT WILL RECEIVE (file · L9 + text), General note, "These keep waiting until SHOP-18 is merged"; Send back →
  the Terminal tab; with typed text in the terminal the unsent-text prompt appears first.

**T-0095 · Approve and Local merge**
- [ ] `03 · Board & columns → Approve choice on drop` and `07 → Local merge · confirm`, `Local merge · success`,
  `Local merge · rebasing onto main… (0038)`, `Merge conflict during Local merge`, `Local merge · refused, uncommitted
  changes (0038)`, `Uncommitted files · refused`: each state's texts and buttons. Known differences: the menu is picked
  by click or ↵ (no 1 / 2 keys); "Merging…" lists the steps without live progress; refused files show paths without
  git's M / ?? codes; the panel button reads "Approve · Local merge" without a separate ▾.

**T-0098 · Conflict flag and changed since review**
- [ ] `07 → Approve · changed since review (0043)`: the title, the new commits (short sha, subject) and only their
  diff; Approve anyway / Send back.
- [ ] `05 · Ticket side panel → SHOP-18 · conflicts with main` and `04 · Ticket card variants · Light / Dark → Review ·
  conflicts with main`: the amber strip "Conflicts with main · 2 files (…)" with Send to agent to fix, and the card
  line. Known difference: the strip doesn't say which merge found it ("checked after SHOP-17 merged").

**T-0106 · Close / Reopen / Submit for review**
- [ ] `05 → Close ticket · panel "…" menu`, `Close ticket · confirm`, `Submit for review · dialog`, `Closed · not
  merged · Reopen`, `Dark · closed ticket`; `04 → Done · closed, not merged`, `Review · submitted by you`: menu items
  per state (Submit disabled with "agent is working"), the confirmation's lines ("Stop agent & close"), the submit
  dialog with the inline refusal, Reopen on the card and in the panel, drag Closed → Backlog reopens, Merged →
  Backlog is refused, "Submitted by you · “note”" on the Review card.

**T-0107 · Park**
- [ ] `03 → Park · Dev → Backlog (allowed)` and `Park · after, and back to Dev`; `04 → Backlog · parked · has work`
  (light/dark): "Park SHOP-25?" with what is kept; Cancel leaves the card in Dev; the parked pill; "Resuming
  conversation…" after dragging back. Known difference: no commit / uncommitted-file counts in the confirmation.

**T-0108 · Parent closed**
- [ ] `04 → Child · parent closed` (light/dark) and `12 · Stack & dependencies → 6 · Closed parent`: the card text and
  red outline, its place after the orange cards in Dev; the panel's Reopen parent / Remove dependency (which asks
  "Remove the dependency SHOP-20 → SHOP-19?").

**T-0109 · Epic review**
- [ ] `05 → Epic panel · SHOP-40 Wishlist`, `Epic in Review · Diff by child (SHOP-40)`, `Approve ▾ on an epic`; `07 →
  Epic child · Approve offers only Local merge`; `04 → Row · epics as delivery units`: PAY-123, "1 / 3 merged", the
  local branch, the children list, the Review note, the diff line "… vs main at … · 3 children · 21 files" and the
  file list by child, the two Approve menus. Known differences: the epic keeps all six tabs; no "Add ticket to epic"
  button in the panel (the epic card has it).

**With the real claude (once):** as listed in §2. Earlier milestones' hand checks stay as listed in review-M1…M5.

## 4. Implementation choices (SDD §26)

- **Diff** (`git::diff`, `board::diff`): always `base_sha...branch` from the project's repository (works for Done
  tickets without a worktree); one file per `diff.file` message, a file cut after 20 000 lines (`truncated`); binary
  files carry old/new sizes, renames git's similarity. Epics group by the children's squash commits
  (`tickets.merge_commit_sha`, migration 0007). `diff.file` with `since` gives only the commits after a reviewed head.
- **Comments and rounds** (`board::comments`): a thread is every comment on the same file/line/side (no new column);
  comments only on a ticket in Ready for Review; round = Send backs + 1; the card's "round K" is the K-th Send back
  (**Q21**).
- **Send back / Send to agent to fix** (`review::Review`): live session → the skill typed through the user-click
  path; gone → `claude --resume <id> "/crewboard:address-review"` (the skill as the resumed conversation's first
  prompt rather than typed into a terminal that may not be ready); agent limit on and full → Queued with
  `tickets.resume_skill` (migration 0008), typed later as an automatic delivery of the new kind `skill`
  (pending_deliveries rebuilt with that CHECK). Back-pressure doesn't hold the click. The start sequence treats a
  returning ticket (resume skill or parked) as a resume: its worktree is kept and base sync, env files and setup are
  skipped.
- **Checkpoint** (`git::checkpoint`): the index is saved as a tree and re-applied after `reset --soft HEAD~1`, so staged,
  unstaged and untracked files come back exactly; rebases use `--no-autostash`; any conflict restores branch, index
  and files; a hook rejection is an error (GIT-02 text), never bypassed.
- **Local merge** (`git::merge`): scratch worktree `<data>/scratch/<repo-name>` (detached, reused); the dirty-checkout
  check runs before any ref moves; a base that exists only on the remote gets a local branch; the squash commit runs the
  user's hooks; the clean checkout is fast-forwarded with `merge --ff-only`, else the ref is updated. One merge at a
  time. After a merge: Done cleanup, the epic re-evaluated, the early conflict check, then
  `Scheduler::on_parent_merged` (its started tickets give "N waiting tickets started").
- **Early conflict flag**: `git merge-tree --write-tree` (git 2.38 or newer; nothing checks the version — no decision
  sets one); conflicts live in `tickets.conflicts_with / conflict_files / conflict_reason` (migration 0009).
- **Changed since review**: the reviewed head moves to its rebased counterpart after a Crewboard rebase by counting the
  commits added after the review (a rebase keeps them in order).
- **Done cleanup / Close / Park**: one `Review::finish` (session stopped, `on_ticket_finished` hooks — none until M9 —,
  ports freed, worktree removed with `git worktree remove` when clean, else `tickets.worktree_kept`, migration 0010).
  Closing never touches Merged tickets; closing an epic closes the epic (**Q20**). Park of a Dev ticket that never got
  a branch returns it to plain Backlog.
- **Reopen**: `tickets.reopened` (migration 0011) makes the next start rebase the old branch onto the current base in
  step 3 and launch a new session; a conflict is recorded with reason `reopen` for `crewboard prime`.
- **Epics**: `reevaluate_epic` after a child's merge or close; a new, moved or reopened child sends an epic in Review or
  Approved back to Backlog; the epic's Local merge rebases its branch in the scratch worktree.
- **New flags**: `submitted_by_you` (from the latest review submit by a person) and `resuming_conversation`.
- **App**: `DiffView` uses a gpui `list` (files drawn on demand fetch their hunks); viewed ticks, folded summaries, the
  Unified / Side-by-side choice live in the app's memory (not stored); `panel/review.rs` holds the panel's review
  dialogs; ⌘⌫ and ⌘↵ are workspace bindings; the unsent-text prompt remembers which click asked.
- **New protocol requests**: `diff.files`, `diff.file`, `review.comment_add/_reply/_edit/_delete`, `review.comments`,
  `review.feedback`, `ticket.send_back`, `ticket.merge_proposal`, `ticket.local_merge`, `ticket.send_to_fix`,
  `ticket.approve_check`, `ticket.conflict`, `ticket.close`, `ticket.reopen`, `ticket.submit_by_user`, `ticket.park`;
  `epic.close` takes `confirm`. Every one has its permission row (writes are user-only; reads are allowed to agents).
- **Plugin**: skills `address-review` and `restack` (not model-invocable), lint-tested.
- **New crate**: syntect 5.3.0 in crewboard-app (default-syntaxes, parsing, regex-fancy). **New icons** (Lucide shapes,
  1.5 stroke): square-check, message-square, git-compare, columns-2.

## 5. Open questions

**Q20 and Q21 added** (`tickets/questions.md`), both deferred:
- **Q20** — Close epic with some children already merged: the epic ends Closed · not merged (option 1, built) or goes
  to Ready for Review (option 2). Decision 0044 item 2 vs EPC-04.
- **Q21** — "Sent back · N comments · round K": K is the Send back's number (option 1, built) or the attempt the agent
  is on (option 2). Decision 0034's "round 2".

Notes for later milestones:
1. **M7 Resume / Resume failed** (T-0112): resumes go through `session::launch::resume_agent(_with)`; a parked ticket
   whose conversation is gone currently starts normally if no Claude session ID is stored, else fails at launch.
2. **M7 Cleanup view** (T-0116): kept worktrees are `tickets.worktree_kept = TRUE` (set by `Review::finish`).
3. **M9 Run / Shell** (T-0133, T-0137): register with `Review::on_ticket_finished` to stop Run and close the Shell on Done.
4. **M10 Mark merged / Mark closed** (T-0147): Mark merged must set Done · Merged, call `Review::finish`,
   `Board::reevaluate_epic` for children, `Review::flag_conflicts` and `Scheduler::on_parent_merged`; Mark closed is
   `Review::close` with `confirm`.
5. **M10 Open PR / Push to PR** (T-0143, T-0144): call `Board::approve_check` first and honour `approve_anyway`, as
   `ticket.local_merge` does; the app's `MergeView::Changed` dialog is reusable.
6. **M10 review feedback** (T-0148): `ReviewFeedback` grows the PR comments.
