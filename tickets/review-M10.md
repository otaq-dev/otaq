# M10 · PR flow & agent questions — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0139 | Workflow skills settings and resolution | 7bc836f |
| T-0140 | Built-in push, open-pr and answer-question skills | da76ee5 |
| T-0141 | PR provider detection and login status | 8362fbe |
| T-0142 | CLI: `pr propose`, `pr opened`, `branch rename` | 4408ce1 |
| T-0143 | Open PR flow (core): proposal, validation, apply edits, push | ebb513f |
| T-0144 | Push to PR and PR rework (core) | 3fbeee7 |
| T-0145 | Pushed-content verification | 491a5f1 |
| T-0146 | Delivery session for epics | a0ea8dc |
| T-0147 | Mark merged / Mark closed | 0e2bdab |
| T-0148 | PR comments in `review feedback` | 24e4e52 |
| T-0149 | Open PR / Push to PR dialog UI | 0886eb7 |
| T-0150 | PR open state UI and PR rework drag | 3b06b01 |
| T-0151 | Agent-to-agent questions: ask, answer, questions, live delivery | c949e95 |
| T-0152 | Finished tickets answer automatically | 709f52d |
| T-0153 | "Let finished tickets answer questions" and inbox routing | 146b54b |
| T-0154 | Messages section, "answering" and "asked · waiting" card flags | f354eea |
| T-0120 / T-0154 follow-up | "Logged in SHOP-21 Messages" under the inbox's reply box; answered `ask human` questions in Messages | b995a6b |

One commit per ticket; a ticket's `tickets/progress.md` row gets its hash with the next ticket's commit. Every commit
was checked with `scripts/check.sh` on exactly the staged state (a separate worktree with its own target folder).

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=/tmp/cbm10-$$                 # short: the socket lives in it
S=$CREWBOARD_HOME; mkdir -p $S
git init -q --bare $S/origin.git && git clone -q $S/origin.git $S/shop-web
(cd $S/shop-web && git commit -q --allow-empty -m init && git branch -M main && git push -q -u origin main)
B=$PWD/target/debug/crewboard
$B daemon start
target/debug/crewboard-app                          # Add project → $S/shop-web
```

Turn Settings → Notifications off first if OS notifications are unwanted while trying.

**Workflow skills and provider (T-0139, T-0141).** Settings → Workflow skills: three fields with the built-in skills as
placeholders; type `my-pr` (no slash) → flagged "my-pr isn't a skill name; use something like /my-pr". A project page
shows the override fields ("use global"), the "Let finished tickets answer questions" toggle (on) and the PR provider
row: with the local bare origin above, "Not detected: use your own Open PR skill" and "origin is /…/origin · no gh or
az provider matches." (`git remote set-url origin git@github.com:acme/shop-web.git` in the repository makes it "GitHub
(detected from origin) · gh: …" — Crewboard only runs `gh auth status` for that; set it back afterwards.)

**The PR flow with a real `claude` (T-0140, T-0143…T-0150).** This needs a scratch GitHub repository you own (the
built-in skills push and open a real PR there) and `gh auth login`. Add it as a project, drag a ticket to Dev, let
the agent work and submit, then Approve ▾ → **Open PR on GitHub**: "Open PR skill is preparing…", then the proposal
(branch, one field per commit, title, description); edit the branch to an existing name to see "taken"; Push &
open PR → pushing steps → toast "PR opened #N"; the card is Approved · PR open with "GitHub". In the panel: the PR
box, Open PR in browser, Mark merged / Mark closed. Drag the card back to Ready for Review (PR rework: "Back for
rework · Send back to start the agent"), Send back (the dialog explains the PR comments), let the agent fix and
resubmit; the card offers **Push to PR**; the dialog locks the branch and lists only the new commits.

Without GitHub, the same dialogs run with a custom Open PR skill (Settings → project → Open PR `/my-pr`, a skill you
write in your own Claude Code setup that follows the open-pr skill's two turns and pushes to the local bare origin).

**Agent questions (T-0151…T-0154).** Two linked tickets (SHOP-2 depends on SHOP-1). From SHOP-2's agent terminal:
`crewboard ask SHOP-1 --stdin <<< "Which status code?"`. SHOP-1 never started → refused (MSG-04). With SHOP-1's agent
live, the question waits for it to be idle ("1 message waiting · Deliver now"); SHOP-1's agent answers with
`crewboard answer <ID> --stdin`. SHOP-2's card says "Asked SHOP-1" until the answer was typed into its session. Once
SHOP-1 is merged, a new question resumes it answer-only (card "answering SHOP-2"); with the project's "Let finished
tickets answer questions" off, it lands in the inbox ("SHOP-2 asks SHOP-1 (Merged): …") with Answer and Resume
SHOP-1. The panel's Messages section (under Stack) shows every thread; `crewboard show SHOP-2 --long` prints them.

Stop with `$B daemon stop` and remove `$CREWBOARD_HOME`.

## 3. Hand checks

**T-0139 Workflow skills**
- [ ] *15 · Settings → Global · Usage, notifications, data, appearance*: the Workflow skills section (three fields,
  built-in placeholders), light and dark.
- [ ] *15 → Project · configured + invalid value*: the Workflow skills rows ("use global", `/my-pr`); a value without a
  slash flagged in red under its field; the nav marks the project.

**T-0140 Skills (real `claude`)**
- [ ] On a scratch GitHub repo: `/crewboard:open-pr` proposes with `crewboard pr propose --stdin`, ends its turn, and
  pushes / runs `gh pr create` only after Crewboard types "The user confirmed the pull request"; it reports with
  `crewboard pr opened <url>`.
- [ ] `/crewboard:push` proposes only the new commits and pushes after the confirmation.
- [ ] `/crewboard:answer-question` in a resumed finished ticket answers with `crewboard answer <ID> --stdin` and
  changes no file.

**T-0141 PR provider**
- [ ] *15 → Project · configured + invalid value*: "Azure DevOps (detected from origin) · az: logged in" row (with an
  Azure DevOps origin and `az login`), and *15 → PR provider · not detected (0050)* with a non-GitHub origin; the "use
  your own Open PR skill" link focuses the Open PR field.
- [ ] Approve ▾ on such a project with the built-in skill: Open PR disabled, "PR provider not detected".

**T-0145 Pushed content**
- [ ] After a push whose content differs from the review (push one extra commit by hand before `pr opened`): the card
  callout "Changed since you reviewed it · the pushed content differs" and the toast. No frame names this state.

**T-0146 Epic delivery (real `claude`)**
- [ ] Open PR on an epic whose children are all merged (SHOP-40 / PAY-123): one short `claude` session in the
  epic's worktree, one commit per child in the dialog, "PAY-123 Wishlist"; the session ends after `pr opened`.

**T-0148 PR comments (real `gh` / `az`)**
- [ ] `crewboard review feedback` on a ticket whose PR has resolved and unresolved review comments lists only the
  unresolved ones (GitHub and Azure DevOps); logged out → the one-line note.

**T-0149 Open PR dialog** (*07 · Diff, review, approve & send back*)
- [ ] Open PR · skill is preparing…; Open PR · confirmation (SHOP-18, 0050); Open PR · pushing; Open PR · success
  (toast); Open PR · invalid or taken branch name (live, Push & open PR disabled); Open PR · not logged in (exact
  command, Retry); Approve · Push to PR #481 (0041); Push to PR · confirmation (branch locked, 0050); Open PR · epic
  SHOP-40 (PAY-123); *05 → Approve ▾ on an epic*. Light and dark.

**T-0150 PR open state**
- [ ] *05 → SHOP-26 · Approved, PR open*: PR box, Open PR in browser, Mark merged, Mark closed with "Mark PR #481 as
  closed?".
- [ ] *07 → Send back · ticket with an Azure DevOps PR (BILL-5)*: the PR note (no comment count, Q32).
- [ ] *03 → PR rework · Approved → Ready for Review (allowed)*: allowed feedback; other drags out of Approved
  forbidden.
- [ ] *04 → Approved · PR open, Review · PR rework, Review · after rework*, light and dark (provider label with the
  `git-pull-request` icon, Q31; "after rework" shows a Push to PR button without the frame's "+6 −2 · 1 new commit"
  line).

**T-0153 Inbox routing**
- [ ] *10 → Agent question for you · new item type*, *Answer · inline* (free text), Resume SHOP-17.
- [ ] *15 → Project · configured*: "Let finished tickets answer questions" toggle.

**T-0154 Messages and flags**
- [ ] *05 → Messages · agent-to-agent (SHOP-40.2)*, *Message states*, *Dark · Messages*.
- [ ] *10 → Answer · inline*, *Answer · options, no recommendation*, *Dark · answer with options*: the "Logged in
  SHOP-21 Messages" line under the reply box; after answering, the ticket's Messages section shows the question and
  the answer ("SHOP-21 → you", "Answered by you").
- [ ] *04 → Done · answering a linked ticket*, *Working · asked another agent*, light and dark.

**T-0152 (real `claude`)**
- [ ] A merged parent is resumed answer-only, answers, and its session and worktree go away.

## 4. Implementation choices (SDD §26)

- **Workflow skills** (`board::settings::resolve_skill`): project override → global → built-in; blank falls through;
  a value must be `/name` (letters, digits, `-_.:`), flagged per field otherwise.
- **Provider** (`pr::provider`): host of origin's URL (HTTPS, `ssh://`, scp-like); `github.com` / `*.github.com`,
  `dev.azure.com` / `*.dev.azure.com` / `*.visualstudio.com`. `project.pr_provider` runs only that provider's login
  check; the PR flow itself detects from the URL alone (no CLI run). No token is stored anywhere.
- **Proposal** (`pr::proposal`): one JSON object `{branch, commits: [{sha, message}], title, description}` on stdin,
  documented in the open-pr and push skills; it must name exactly the commits to push, in order (short IDs resolved).
- **Flow** (`pr::flow`, `pr::service::Prs`; migration 0018 `pr_flows`, `tickets.branch_pushed`): preparing →
  proposed → pushing; the ticket stays in Ready for Review until `pr opened` moves it to Approved · PR open. The skill
  runs in the live agent session through the user-click path (unsent text asks first) or a resumed conversation; for
  an epic, a `delivery` session (new conversation, skill as first prompt) in `wt/<repo>/<EPIC>` on the epic branch.
  `branch_locked` already meant the base lock (d0051), so the GIT-07 lock is `branch_pushed`.
- **Applying edits**: a `git commit-tree` chain (trees and authors kept; an unchanged prefix keeps its IDs), trees
  compared, `update-ref`, `git branch -m` (the worktree follows; a failed rename restores the old commits); the
  reviewed head moves with the rewording. After that Crewboard types "The user confirmed the pull request. Run
  `crewboard prime` …"; `prime` gains a "Pull request" section with the confirmed branch, title and description.
- **Problems while pushing**: `crewboard ask human` from a ticket whose PR flow runs is the dialog's reported problem
  (the ticket stays in Ready for Review); Retry (`ticket.pr_retry`) types the skill or the confirmation again.
- **Push to PR**: commits reachable from `origin/<branch>` (fetched) are pushed/read-only; only the new ones are
  proposed and reworded.
- **Pushed content** (migration 0019 `tickets.pushed_differs`): after `pr opened`, origin's branch tree vs the
  reviewed tree; a difference sets `changed_since_review`, a callout and a toast (event `pushed_content_differs`).
- **Mark merged / closed** (`review::pr_state`): Done · Merged → `Review::finish` → `reevaluate_epic` →
  `Scheduler::on_parent_merged` (fetch, then start) → `flag_conflicts` against the fetched base; Mark closed always
  asks, then Close ticket / Close epic.
- **PR rework** (migration 0020 `tickets.pr_rework`): the drag Approved → Ready for Review keeps the PR; the flag lasts
  until the ticket is submitted again (card: rework callout, then Push to PR).
- **PR comments** (`pr::comments`): GitHub `gh api graphql` (reviewThreads, `isResolved`); Azure DevOps `az devops
  invoke --area git --resource pullRequestThreads` (az repos has no thread command); the CLIs come from the
  service's tools PATH; failures become one note line. Fetched only during `review feedback`.
- **Agent questions** (`messages`; migration 0021): linked = a direct dependency either way; never-started = no agent
  session ever; live target → an `agent_message` delivery tied to its message (`pending_deliveries.message_id /
  message_part`), so `delivered_at` / `answer_delivered_at` are set when typed (event `message_delivered`); message
  IDs are UUIDv7. `crewboard prime` lists pending questions.
- **Answer-only resume** (`messages::answering`): `route_waiting` after every ask and answer and every 30 s; worktree
  recreated at `<data>/wt/<repo>/<KEY>` on the kept branch; session kind `answer` with `claude --resume <id>
  "/crewboard:answer-question"` and the status-line settings; one answer session per ticket; the plan limit holds the
  question; at ask time the conversation's transcript must still exist (read-only), else "…conversation is no longer
  available".
- **Inbox** (MSG-05): questions routed to the inbox are group-1 items keyed by the asker (id = the question's), no OS
  notification (the ask blocks nobody), counted for back-pressure; Answer = `message.answer`, Resume =
  `message.resume`.
- **Questions to the user in Messages** (follow-up for M7's inline answer): the inbox's reply box says "Logged in
  SHOP-21 Messages", and the Messages section also lists the ticket's answered `ask human` questions with the answer
  (`ticket.human_answers`, read-only).
- **Flags**: `pr_rework`, `answering`, `asked` + `asked_question` + `asked_more`; "asked" ranks 7th (pill "Asked
  SHOP-40.1 (+N)", else meta "asked SHOP-40.1 · waiting"); `Ticket.work` gains `branch_locked`, `pr_url`;
  `Ticket.pushed_differs`.
- **New protocol**: `project.pr_provider`, `ticket.pr_propose / pr_opened / branch_rename / open_pr / pr_confirm /
  pr_cancel / pr_retry / pr_flow / pr_branch_check / mark_merged / mark_closed / ask / questions / messages`,
  `message.answer / resume`; events `pushed_content_differs`, `message_delivered`; History events branch_renamed,
  pr_proposed, pr_opened, pr_confirmed, pr_cancelled, question_asked, question_answered. Every request has its
  permission row (agents: own ticket for pr/branch/ask/answer; user-only for the clicks; reads allowed).
- **CLI**: `pr propose --stdin`, `pr opened <url>`, `branch rename <name>`, `ask <KEY> --stdin`, `answer <ID> --stdin`,
  `questions`; `show --long` adds messages; `review feedback` adds PR comments.
- **Stack size**: with M10's queries the ticket assembly future grew enough to overflow a tokio worker's stack in a
  debug build (`create --deps` in the CLI tests); `Board::tickets` / `ticket_by_id` now box it (T-0154's commit).
- **Tests never touch a real remote or CLI**: local bare origins, stand-in `gh` / `az` scripts on a temporary PATH,
  `sh` / `cat` stand-ins for `claude`. No new crates.

## 5. Open questions

Added to `tickets/questions.md`, both deferred:
- **Q31** — Lucide has no GitHub icon any more: `git-pull-request` for every provider (built as option 1).
- **Q32** — the Send back frame counts the PR's comments, which needs a fetch outside `review feedback` (d0017 "on
  demand"): built as option 1, a note without a count.

Notes for later milestones:
1. **M11 Windows / Linux**: the PR flow runs `git commit-tree` / `update-ref` / `branch -m` (portable); `gh` / `az`
   are looked up with `tools::find_on_path` (Windows needs `.exe` / `.cmd` handling there); tests use `/bin/sh`,
   `/bin/cat` and Unix permissions for stand-ins; answer / delivery sessions use the same launch as agents.
2. The design's "Review · after rework" card shows "+6 −2 · 1 new commit"; the card has the Push to PR button but no
   diff stat (the board has no per-ticket diff numbers yet).
3. A question whose answer reaches an asker without a live session stays "answered" (shown in prime only when asked
   again); it is typed only into a live session.
4. Smoke run (agent): a temporary daemon with no `claude`, `gh` or `az` on PATH: project add, provider "not detected",
   two linked tickets, Start interrupted at launch, `ticket.ask` refused for a never-started target, `ticket.pr_flow`
   / `open_pr` answers, the inbox; the app ran 8 s without errors; no process left, no `~/.crewboard`. Visuals were not
   checked by the agent.
