# 0083. Epic lead: one read-only agent directs an epic, and Otaq runs its children as tickets

- **Status:** Accepted
- **Date:** 2026-10-16
- **Related:** 0011, 0016, 0084, 0018 (new exception), 0022, 0030 (partly superseded), 0039 (partly superseded), 0042, 0044,
  0045, 0047, 0048, 0049, 0059 (partly superseded), 0060, 0061, 0063, 0064, 0068 (partly superseded), 0079,
  0080 (partly superseded), 0081
- **Supersedes:** from 0039, 0059 and 0080, "an epic has no agent"; from 0068, "It can't … edit started tickets" (for an
  epic lead and its own epic's children that are queued but not started); from 0030, "Only between tickets linked by a
  direct dependency" (a child's questions go to its epic's lead). It adds a new exception to 0018's click rule.

## Context
Otaq runs an epic by rules only: dragging the epic queues its children (0080), each child starts when its
dependencies are merged (0049), is validated (0063) and can be Local-merged into the epic branch without a click (0064).
Nothing thinks about the epic as a whole.

When the user built M15 by hand (0079), an orchestrator session did that thinking: it chose what runs in parallel and
what waits, passed advice learned from one teammate to the next, answered teammates' questions itself and sent only
blockers to the user, changed the plan after a ticket landed, and watched for files several tickets touch. Otaq can't do
any of this today. For example, SHOP-40.2 (Wishlist UI) learns nothing from 40.1 (Wishlist API) beyond its submit
summary, and every `otaq ask human` from 40.2 goes to the user's inbox.

## Decision
- **An epic gets a lead agent.** One session for the epic, read-only like a Project agent (0068): it reads the code and
  the children's worktrees, never edits or commits, and works only through `otaq` commands. It plans, advises,
  answers and re-plans.
- **Children stay normal Otaq tickets.** Each keeps its own card, worktree, live terminal, validator (0063), Auto-merge
  (0064) and token count. The lead never runs the children's work inside its own session.
- **No provider orchestration features.** The lead doesn't use Claude Code subagents or agent teams to run children.
  Otaq will support other agent providers soon (Codex, OpenCode…), and there is no guarantee they have subagents. Otaq
  stays the runtime for children, and the lead only needs a CLI it can call, so any provider can be a lead.

- **Starting the lead.** The user opens it from the epic card at any time, also while the epic is still in Backlog, to
  plan with it ("split this into tickets of about a day"). Opening it is the go-ahead for its tokens (0018). It stays
  with the epic when the epic is dragged and keeps the same conversation from the plan to the last merge, so the user
  never explains the epic twice. **Dragging an epic that has no lead starts one**: the drag is already the go-ahead for
  the children (0080) and covers the lead too. Its first job is then to check the plan before the first child starts.
  Planning without a lead stays possible with a Project agent (0068).

- **What wakes the lead.** Between events it is idle and spends nothing. Otaq wakes it **after each child merges into
  the epic branch or is closed**, with an automatic message (0048 rules: it waits until the lead is idle and its input
  line is empty), for example "SHOP-40.1 merged: Wishlist API". In that turn the lead reads what landed and updates the
  plan: it edits children that haven't started ("40.2: the API returns `items[]`, not `list`"), adds a missing child,
  or flags files two upcoming children both touch. These turns are a **new exception to 0018's click rule**, like the
  validator (0063): opening the lead, or dragging the epic, is the go-ahead. A child's question also wakes it (below).
  Other child events (start, submit, validation) don't. The user can always type to it.
  - **What it does in a merge turn.** Besides re-planning, the lead reads the merged child's submit summary, its notes
    for later tickets and its recorded decisions (0084), and:
    - **passes them on** to children that haven't started, by writing them into their descriptions ("40.1 returns
      items newest first (recorded decision); keep that order on the page");
    - **marks a recorded decision that a written source contradicts**: the item in the review list gets "Lead: PAY-123
      says 'in the order added'". It never overrules a decision itself and never adds a fix child for one: keeping or
      changing it is the user's (0084).
  - **Notes to running children wait at checkpoints.** A note from the lead to a child that is working is stored on the
    child's ticket as unread. The built-in skills pick it up at steps the agent passes anyway: before each commit (the
    Commit skill runs `otaq notes`) and in `otaq prime`. **`otaq review submit` is refused while a note is unread**,
    so no child finishes without reading it. If the child goes idle first, the note is typed in as before (0048). This
    needs only the `otaq` CLI, so it works for any provider; a provider's own hooks may be added later to deliver it
    sooner. (In M14, 15 of 49 notes arrived more than 30 minutes late when they waited for the end of a turn.)
  - **Children changing the same file.** Otaq compares the files each running child has changed so far (its branch
    against the epic branch). When two overlap, both cards show a quiet "same file as SHOP-40.4" flag, with the file on
    hover; it doesn't wake the lead. At its next wake the lead sees the overlaps in its context and can make a child
    that hasn't started wait for one that is running ("40.5 waits for 40.4: both change `lib/wishlist.ts`"). That is an
    ordinary dependency (0049), added only when the child could otherwise start before the other merges (no wait
    that existing dependencies already imply): the waiting child starts on its own when the other merges, from an epic branch that
    already holds its changes; the user can remove it at any time. Running children are never paused for an overlap;
    if they conflict at merge, the usual fix applies (0064).
  - **It answers only from what is written.** Every answer, note or mark from the lead names its source (a decision,
    the epic's plan, a ticket's description, a sibling's merged code or recorded decision). What nothing written
    settles goes to the user; the lead never guesses.

- **Recorded decisions (0084).** Like every ticket's agent, a child decides and records its choice unless the question
  blocks it; only a blocking question is asked. The epic's review lists every child's recorded decisions together, so
  the user sees them even when the children were merged automatically (0064).
- **Children's questions go to the lead first.** A child's `otaq ask human` wakes the lead instead of making an inbox
  item. The child's card shows "asked lead · waiting" (display only, as in 0030), not turquoise.
  - If the lead can answer from the plan or the code, it replies with `otaq answer`; the answer is typed into the
    child's session (0048 rules) and the user gets no inbox item.
  - If the question is the user's call (a product choice, something outside the plan), the lead **passes it on**: an
    "Asks you" inbox item "SHOP-40.2 asks · passed on by Wishlist lead", with the lead's suggested answer as the
    recommended option (0061). The child turns turquoise as today.
  - Every question, answer and hand-over is logged on the child and on the epic (0030's Messages).
  - If the lead has no live session (for example "Ended by restart"), the question goes straight to the user, as
    today.
  - **Permission prompts always go to the user**; the lead never answers them (0011, 0016). The user can type to a
    child at any time.

- **Stuck children go to the lead first, once.** When a child stops with "Validation failed N times" (0063), "Keeps
  conflicting with `<epic branch>`" (0064) or Idle (0042), the lead is woken and gets **one attempt**: it reads the
  validation report, the conflict or the agent's last message, and sends the child one note, the same as the user's
  "Answer the agent" (the validation round counter restarts at 0). For example "The API returns `items[]`; fix the
  test, don't change the API." If the child gets stuck again, it goes to the user's inbox as today, with the lead's note
  and its view of the cause. "Move to Review anyway" and "Give it another round" stay the user's. The lead's note to an
  Idle child is part of this decision's exception to 0018's click rule (Otaq itself still never nudges, as in 0063).

- **What the lead may change.** Only its own epic, and never code.
  - **On its own:** edit the description, dependencies and priority of children that haven't started (Backlog or
    queued); add dependencies between children to decide what runs side by side and what waits ("40.5 waits 40.4: both
    rewrite `routes.ts`"); add children (they join the queue under 0080, flagged "new · added by Wishlist lead"); send
    notes to running children. Each change is a line in the epic's history and the child's.
  - **As a suggestion only:** closing or deleting a child ("Wishlist lead suggests closing 40.6: export is covered by
    40.2"). The suggestion is an inbox item the user confirms with one click (the usual 0044 / 0060 confirmation); it
    blocks no agent, so it has no OS notification (0016). In the inbox, suggestions are **their own group right after
    group 1** (before Interrupted and Ready for Review): a queued child the lead wants closed may start soon.
  - **Never:** move a ticket between columns, queue, take back or park one, edit a started child, approve, merge, or
    touch tickets outside its epic. The `otaq` CLI refuses these for the lead, as it does for ticket agents.

- **Where the lead lives on the board: on its epic.**
  - In Backlog, the epic card shows a small lead status and **Open lead**.
  - In flight, the lead sits at the start of the epic's track in the "Epics in flight" strip (0080): "Wishlist lead ·
    idle", "· re-planning", or turquoise "· asks you".
  - Opening it shows its terminal in the epic panel, like a ticket's terminal; the user can type to it at any time.
  - When the lead itself waits for the user (its own `otaq ask human`, a permission prompt, a suggestion), it is an
    inbox item like any agent's (0016) and its track turns turquoise.
  - It is **not** listed under "Terminals & agents" (0068); that stays for project-wide sessions.

**Lifecycle** (as for Project agents, 0068): the background service owns the lead (0022). After a restart it shows
"Ended by restart · Resume", resumed on the user's click; until then the epic runs under the old rules (questions go
straight to the user). The lead ends when the epic reaches Ready for Review: from there the user reviews the whole epic
(0059). Reopening or adding a child after that resumes the same conversation. The lead's tokens count on the epic and
in today's total (0020); the plan-limit hold applies. It doesn't count toward the agent limit, which counts tickets
(0016).

**Unchanged:** children are ordinary tickets with their own agent, worktree, validator and Auto-merge; dependencies
(0049), the optional limit (0016) and start order (0065, 0080) still decide when a child starts; Backlog never starts by
itself; epics never auto-merge, and Open PR / Push on an epic still run in a short-lived delivery session (0059), not
in the lead; Send back on an epic still isn't available.

## Alternatives considered
- **Waking the lead when two children change the same file:** rejected; it costs a lead turn per overlap and most
  overlaps merge cleanly. No detection at all was chosen first and then reversed, once it was clear that a wait only
  delays the later child until the earlier one merges.
- **One session per epic that runs the children as Claude Code subagents or agent teams:** rejected. It ties epics to
  one provider. In Claude Code, agent teams are experimental, teammates share the lead's folder (no worktree each) and
  aren't restored by `/resume`; subagents return only a summary, with no card, validator, live terminal or per-ticket
  tokens (against 0022, 0030, 0045, 0048, 0063).
- **Keep the rule-based scheduler and add only small fixes** (questions go to a Project agent first, a "lessons so far"
  note passed to the next child): not chosen; it can't re-plan an epic while it runs.

## Consequences
- A new Otaq skill, `/otaq:epic-lead`, and an epic context (`otaq prime --epic`: the epic, every child with status,
  summary, worktree and branch, the epic branch, open questions, the user's Agent instructions). The CLI gets a lead
  role in its permission rules (`policy.rs`) and a way to suggest closing or deleting a child.
- `otaq ask human` from an epic child is routed to the lead when it has a live session. The asking card's "asked lead ·
  waiting" and the inbox item "passed on by <lead>" are new.
- The scheduler wakes the lead after a child merges or is closed and when a child gets stuck for the first time.
- Design (update S): the lead status on the epic card and the track, the lead tab in the epic panel, the passed-on
  question and the suggestion in the inbox. `design-briefs/CHANGES-2026-10-16-s.md`.
- Because the lead only uses the `otaq` CLI, it works the same for any agent provider Otaq supports later.

## Changelog
- 2026-10-16: created as Proposed (user decision: an epic lead that directs, with Otaq running children as tickets;
  provider subagents and agent teams rejected because other providers are coming)
- 2026-10-16: the user opens the lead from the epic card, also in Backlog, and it keeps one conversation from plan to
  last merge; dragging an epic without a lead starts one (user decision)
- 2026-10-16: the lead wakes only after a child merges or is closed, to re-plan; other child events don't wake it; a
  new exception to 0018 (user decision)
- 2026-10-16: children's questions go to the lead first; it answers or passes them on with a recommended answer;
  permission prompts stay with the user (user decision)
- 2026-10-16: stuck children (validation failed, keeps conflicting, Idle) get one note from the lead before they reach
  the user (user decision)
- 2026-10-16: the lead edits never-started children, their dependencies and priority, adds children and sends notes;
  closing or deleting a child is only a suggestion to the user; it never moves, approves, merges or edits started
  children (user decision)
- 2026-10-16: the lead lives on its epic (card status and Open lead, start of the track, terminal in the epic panel),
  not in Terminals & agents; accepted (user decision)
- 2026-10-16: the lead's suggestions are their own inbox group right after group 1 (design audit S, user decision)
- 2026-10-16: a child decides unless the question blocks it; recorded decisions are stored on the ticket and listed in
  the ticket's and the epic's review (user decision, after the evidence report on past runs)
- 2026-10-16: the decide-and-record rule moved to its own decision, 0084, and applies to every ticket (user decision)
- 2026-10-16: in a merge turn the lead reads the child's summary, notes and recorded decisions, passes them on to
  never-started children, and marks decisions a written source contradicts; it never overrules one; it answers only
  from written sources (user decision)
- 2026-10-16: notes to running children are stored as unread, read at commit and prime checkpoints; submit is refused
  while one is unread (user decision)
- 2026-10-16: no detection of children changing the same file; conflicts at merge are enough (user decision)
- 2026-10-16: reversed the same day: Otaq flags children that change the same file on both cards, without waking
  the lead; the lead may make never-started children wait (user decision)
- 2026-10-16: stuck children keep the one-note rule; past runs gave no reason to change it (user decision)
- 2026-10-16: the lead adds a same-file wait only when the child could otherwise start first; no implied waits
  (design audit T, user decision)
