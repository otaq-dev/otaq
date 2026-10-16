# M17 · Epic lead and recorded decisions — tickets

M17 gives an epic a read-only **lead** agent. The lead plans the epic with the user, answers its children's questions
first, leaves notes for running children, sends one note to a stuck child and re-plans after each merge. It also lets
every ticket's agent **record** a product choice instead of stopping to ask, and lists every recorded decision in the
review. The sources are:
- `decisions/0083` (the epic lead) and `decisions/0084` (recorded decisions). With their changelogs, they override
  anything older that disagrees. `decisions/0079` says how the milestone is built: an orchestrator, teammates, one
  worktree and one squashed commit per ticket.
- `SDD.md` §6.6 LEAD-01…LEAD-12 and §6.7 DEC-01…DEC-06, CLI-07 and CLI-08 (§11.3), the M17 rows of §11.2 and §12.2. Also
  §9.3, §11, §12, §14 (validation, Send back, Auto-merge), §15, §16, §17, §19.1 (Project agents: the lead reuses that
  session kind) and §23.
- The design, updates S and T in `crewboard.pen`. The frame maps and audits are in
  `design-briefs/CHANGES-2026-10-16-s.md` and `CHANGES-2026-10-16-t.md`.

**18 tickets, T-0276–T-0293, one epic: M17 · Epic lead and recorded decisions.**

- **Format:** the same as `tickets/m16-tickets.md` (the shape for `otaq create -f`, 0032). The IDs continue that file's.
- **Definition of done** (0056): `scripts/check.sh` is green, new behaviour has tests, the "Done when" items are met, and the
  commit names the ticket, its requirement IDs and decisions 0083 / 0084 / 0079. **A conflict with a decision → defer or
  stop as `prompts/implementer.md` says.**
- **Hand checks** compare the running app with the named frames. Agents never tick them. Run the app with
  `OTAQ_HOME=/tmp/m17 npm run tauri dev` in `crates/otaq-desktop`, in Otaq Light and Otaq Dark, at 1440 × 900 and
  1280 × 800. Use the test project in § "The example" below. Checks that need a real `claude` say so.
- **Build only what 0083, 0084, LEAD-01…12 and DEC-01…06 say.** Out of scope:
  - provider-specific delivery of notes (hooks);
  - a setting that chooses between asking and recording;
  - waking the lead on an overlap;
  - any new shortcut;
  - anything about other agent providers beyond keeping the lead CLI-only.
- **Build from the frame maps only.** Never build from rows labelled "before 0083" / "before 0084", or from the older
  SHOP-23 "Asks you" frames. Where a frame and 0083 / 0084 differ, the decision wins. Known leftovers in the frames that
  don't change behaviour:
  - Frames 21–22 use a filled-heart question that doesn't block. Under 0084 that question would be recorded. The flow
    itself (the lead answers a question) is right.
  - 04's "1 decision" card is still tagged "PROPOSAL". The user kept it (0084).
  - The inbox note still calls the suggestions group "a proposal". It was decided in 0083.
  - Notes (T) Q4 and Q8 describe older drafts.
- **Tests never start a real `claude`.** Sessions use the ordinary stand-in command (`LaunchContext.claude`, SDD §23.1),
  and `OTAQ_HOME` points to a temporary folder.

## Words used in these tickets
- **Lead:** the epic's lead agent (LEAD-01). It is named after the epic: "Wishlist lead". It is stored as a Project agent row
  with the epic set (T-0278) and runs as a `project_agent` session with the role `epic_lead`.
- **Live lead:** a lead with a running session. "Ended by restart", "ended · epic in review" and a lead whose `claude`
  exited are not live.
- **Never-started child:** as in M16, a child that has never been to Dev (`has_been_to_dev` is false), whether it is in
  Backlog or in Ready for Dev.
- **Running child:** a child in Dev with an agent session (working, turquoise, background or validating).
- **Wake-up:** an automatic message the service types into a live lead (§9.3 rules: only when the lead is idle and its input
  line is empty). There are three kinds:
  1. a child merged into the epic branch or was closed ("SHOP-40.1 merged: Wishlist API");
  2. a child asked ("SHOP-40.2 asks (question 7): …");
  3. a child got stuck for the first time ("SHOP-40.3 stopped: Validation failed 3 times …").

  Nothing else wakes it (LEAD-04).
- **Lead status:** what the lead is doing. Otaq works it out from the lead's state and the reason for its current turn
  (T-0278):
  - `none` · "Open lead";
  - `idle`;
  - `checking the plan`: its first turn after a drag started it;
  - `planning`: a turn the user started by typing;
  - `re-planning`: a merge, close or stuck wake-up;
  - `answering SHOP-40.2`: a question wake-up;
  - `asks you`: its own `ask human` or a permission prompt;
  - `ended by restart` · Resume;
  - `ended · epic in review`;
  - `ended` · Resume: its `claude` exited for another reason. This reuses the short form's "ended | Resume", and no frame
    draws the full form.
- **First stop / second stop:** a child stops when it reaches "Validation failed N times" (VAL-06), "Keeps conflicting with
  `<epic branch>`" (AMG-03) or Idle (ATT-01).
  - The **first stop** of a child whose lead is live wakes the lead and makes no inbox item. It happens once per child.
  - Every **later stop** goes to the inbox as today.
- **Hand-back:** a question or a first stop that was sent to the lead, but whose turn ended without an answer, a pass-on or
  a note for it. It then goes to the user as if no lead were live: a plain "Asks you" item, or the stop's own inbox item.
  The same happens if the lead ends while one is open. This rule keeps a child from waiting on a lead that did nothing.
- **Pass-on:** `otaq ask human --pass-on <QUESTION-ID>`. The child's question becomes an "Asks you" item "SHOP-40.2 asks ·
  passed on by Wishlist lead", with the lead's option recommended and its reason shown.
- **Unread note:** a note from the lead to a running child, stored as unread until `otaq notes` prints it or the service
  types it in.
- **Overlap:** two running children of one epic that have changed the same file so far (LEAD-09).
- **Recorded decision:** a row on a ticket with the question, what was built, the other options and why (DEC-02). Its ID
  is `<KEY>#<n>`, numbered per ticket (`SHOP-40.1#1`). The ID is printed by `decision record`, `show` and `prime`.
- **Lead's mark:** the lead's one-line note on a recorded decision that a written source contradicts, with the source
  (`PAY-123`).

## Where the code goes
This list is what makes parallel work possible, so keep to it. If a ticket really needs another place, name it in its
commit and in `tickets/review-M17.md`.
- **Core** (`crates/otaq-core`), tested without a UI (SDD §23.1):
  - **Migrations:** each ticket that adds one names it by a placeholder (`00NN_decisions.sql`). It **takes the next free
    number on `main` when it lands**, and the file is renamed on the rebase, like question numbers. M16 ended at 0030.
  - `src/decisions/` (new): recorded decisions (T-0276), keep and change (T-0277), the lead's mark (T-0280).
  - `src/project_agents/`: the lead's storage and lifecycle, in a new `lead.rs` (T-0278, T-0279). `mod.rs` changes only
    where a Project agent list or the summary must leave leads out.
  - `src/epic_lead/` (new):
    - `wake.rs`: wake-ups and the hand-back (T-0279; T-0284 and T-0285 add to it);
    - `suggest.rs`: suggestions (T-0280);
    - `notes.rs`: notes (T-0281);
    - `overlap.rs`: overlaps (T-0282);
    - `prime.rs`: `prime --epic` (T-0283);
    - `questions.rs`: question routing (T-0284);
    - `stuck.rs`: stuck children (T-0285).
  - `src/caller.rs`, `src/policy.rs`, `src/service/permissions.rs`: the lead caller and its rules (T-0278 adds the caller;
    T-0280 owns the rules).
  - `src/session/launch.rs`: the lead's launch and resume (T-0278).
  - `src/board/questions.rs`, `src/messages/mod.rs`: routing and logging (T-0284).
  - `src/validation/mod.rs`, `src/review/auto_merge.rs`, `src/signals/`: where a stop is decided; T-0285 hooks the first
    stop there.
  - `src/board/review.rs`: the submit check (T-0281, CLI-08).
  - `src/board/needs_you.rs`, `src/attention/inbox.rs`, `src/attention/notify.rs`: the inbox items (T-0280 adds the
    suggestions group; T-0284 and T-0285 add their items).
  - `src/board/epics.rs`, `src/review/mod.rs` (`local_merge`), `src/review/close.rs`, `src/review/pr_state.rs`: the merge
    and close wake-ups, and ending the lead at Ready for Review (T-0279).
  - `src/prime.rs`: decisions and unread notes in `otaq prime` (T-0276, T-0281).
  - `src/recovery/mod.rs`, `src/usage/limit.rs`: the restart and the plan limit (T-0279).
- **CLI** (`crates/otaq-cli`): `src/decision.rs` (new; T-0276, T-0280), `src/lead.rs` (new; `note`, `notes`, `suggest`,
  the `--pass-on` path; T-0280, T-0281, T-0284), `src/prime.rs` (T-0283), `src/read.rs` (`show`, T-0276).
- **View models** (`crates/otaq-view/src/view_models/`): every wording and every rule the screen shows (SDD §5.5).
  - `decision_item.rs` (new): the Decision Item and the Decisions list (T-0287).
  - `decision_change.rs` (new): the Change… and "Add a fix child" dialogs (T-0288).
  - `diff_page.rs`, `review_page.rs`, `approve.rs`, `panel_actions.rs`: the list in the review and the Approve label
    (T-0287).
  - `lead_status.rs` (new): the Lead Status in full and short forms (T-0289).
  - `epic_card.rs`, `epic_strip.rs`, `after_restart.rs`: the lead on the board and in the reboot summary (T-0289).
  - `lead_tab.rs` (new), `panel.rs` (the `Lead` tab), `messages.rs`: the epic panel (T-0290).
  - `card.rs`, `card_page.rs`, `card_fixtures.rs`: the children's card states and the flag order (T-0291), and the "N
    decisions" count (T-0288).
  - `inbox.rs`, `inbox_page.rs`: the lead's inbox items (T-0292).
  - `history.rs`: one row wording per new History event. **Each core ticket adds the rows for the events it creates.**
- **Desktop** (`crates/otaq-desktop`):
  - `src/api/review.rs` and `ui/src/screens/review/`, `ui/src/screens/diff/DiffTab.tsx`: Decisions and the dialogs
    (T-0287, T-0288).
  - `src/api/board.rs`, `ui/src/cards/EpicCard.tsx`, `ui/src/screens/board/strip/`: Lead Status (T-0289).
  - `src/api/panel.rs`, new `src/api/lead.rs`, new `ui/src/screens/lead_tab/`: the Lead tab (T-0290). It reuses
    `terminal/Terminal.tsx` and the recovery body of `screens/terminal_tab/`.
  - `ui/src/cards/TicketCard.tsx`, `cards/look.ts`: card states (T-0291).
  - `ui/src/screens/inbox/`: inbox items (T-0292).
  - `ui/src/screens/sections/`: Decisions in the Ticket tab (T-0288), Messages on the epic (T-0290).
- **Plugin** (`plugin/skills/`): `epic-lead/SKILL.md` (T-0278 adds a stub; T-0286 writes it), `work-ticket`, `commit`
  (T-0286).
- **No new npm package or crate.** If a ticket needs one, that is a blocker: it changes `package.json` / `Cargo.toml`, which
  runs alone.

## Shared files
A ticket may add **one entry** to each of these registration files. Conflicts there are one-line merges that keep both
sides:
- `crates/otaq-core/src/protocol.rs`: one `Request` variant and its name per request. Also its arm in
  `service/permissions.rs` (an exhaustive match) and its dispatch (`board_requests.rs` or `agent_requests.rs`), and its
  row in `tests/every_request.rs`;
- `crates/otaq-core/src/domain/flags.rs`: one field per card flag a ticket adds;
- `crates/otaq-core/src/board/tickets.rs` `EventKind`: one variant per new History event, and its row in `otaq-view`
  `history.rs`;
- `crates/otaq-cli/src/cli.rs` and `main.rs`: one subcommand and its dispatch line;
- `crates/otaq-desktop/src/generated.rs` (`export_all`, one line per new type) and the files it writes in
  `ui/src/generated/`;
- `crates/otaq-view/src/view_models/mod.rs` (one `pub mod` line);
- `crates/otaq-view/src/icons.rs`, its SVG assets and the generated `IconName.ts`. This is the M15 lesson: icons are a
  shared list. New icons: `signpost` (Decision Item, T-0287), `notebook-pen` or the existing agent icon (notes, T-0291).
  Check what exists first;
- `crates/otaq-desktop/src/api/mod.rs` (one area or command line);
- `crates/otaq-desktop/ui/src/app/registry/panel.ts` (`PANEL_TAB_NUMBER`, T-0290 only);
- `crates/otaq-desktop/ui/src/screens/components/Gallery.tsx` (one section per ticket).

Bookkeeping (`tickets/progress.md`, the ticked boxes here, `tickets/questions.md`) is added on the rebased branch just before
the check (0079). **Question numbers are taken from `main` at landing time** (Q61 onwards), not from the branch.

## The example
A test project `shop-web` (key SHOP), validation on, Auto-merge "Epic children only". Use it for every hand check. The keys
and texts are the samples of updates S and T:

| Key | Title | Notes |
|---|---|---|
| SHOP-40 | Wishlist (External ID PAY-123) | epic; children 40.1 Wishlist API, 40.2 Wishlist button (depends on 40.1), 40.3 Wishlist page (depends on 40.2), 40.4 Wishlist share link, 40.6 Wishlist export (CSV) in Backlog, added by hand |
| SHOP-45 | Search (PAY-130) | epic, children 45.1 Search index API, 45.2 Search results page; no lead |
| SHOP-23 | Split user full name | standalone; reaches Ready for Review with one recorded decision |
| SHOP-18 | Checkout API | standalone; Ready for Review with no decisions |

- **The lead's work, for hand checks without a real agent:** the lead is an ordinary session. Run the `otaq` commands it
  would run from a terminal **inside** that session (the stand-in `cat` session lets you type; or use `otaq attach` with
  the lead's role set). Examples: `otaq update SHOP-40.3 --stdin`, `otaq note SHOP-40.4 --stdin`, `otaq answer 7 --stdin`.
- **Checks that need a real `claude`:** the lead planning from the epic card (flow A), a real merge turn (flow C), and a real
  child running `otaq decision record` and `otaq notes` (T-0286).

| Epic | Tickets |
|---|---|
| M17 · Epic lead and recorded decisions | T-0276 – T-0293 (18) |

## M17 · Epic lead and recorded decisions
Goal: an epic runs with a lead that thinks about the whole epic and sends the user only what is theirs to decide; and no
agent's product choice is silent or blocks the work.

### T-0276 · Core: recorded decisions, `otaq decision record`, and `show` / `prime` listing them
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** DEC-02, DEC-01 (the storage the rule needs)
- **Decisions:** 0084, 0023, 0031, 0032
- **Design:** none (no UI). The History wording comes from *05 · Ticket side panel → Row · Recorded decisions (0084)*.
- **Depends on:** none
- **Size:** M
- **Shared files:** migration `00NN_decisions.sql` (new), creates `src/decisions/`, `src/prime.rs` (one section),
  `otaq-cli` `src/decision.rs` (new), `src/read.rs` (`show`), one request in `protocol.rs` and its permission arm, one
  `EventKind` and its `history.rs` row, `cli.rs` / `main.rs`

**What and why**
- **Migration:** a `decisions` table. Columns:
  - `id`, `ticket_id`, `ordinal` (per ticket, from 1), `question`, `built`, `other_options` (JSON list, at least one),
    `why`;
  - `recorded_by` (actor), `recorded_at`;
  - the lead's mark: `mark_text`, `mark_source`, `marked_by`, `marked_at` (empty until T-0280);
  - the outcome: `outcome` (`open` / `kept` / `changed_sent_back` / `changed_fix_child`), `outcome_text`,
    `fix_child_id`, `outcome_at` (written by T-0277).
  - There is no file in the user's repo (0023).
- **`src/decisions/`:** `record`, `list(ticket)`, `list_for_epic(epic)` (every child's decisions, grouped by child in
  child order), `get(id)`, and an ID parser for `SHOP-40.1#1`.
- **`otaq decision record --stdin`** (one request, e.g. `decision.record`):
  - It is allowed for an agent caller on **its own ticket** only (CLI-02 style refusal for anyone else). The user's
    terminal may name a ticket with `otaq decision record SHOP-23 --stdin`.
  - Stdin holds labelled lines; each field may wrap onto following lines:
    ```
    Question: Split fullName everywhere, or only in the profile form?
    Built: only in the profile form
    Other option: everywhere
    Why: the description only mentions the profile form
    ```
  - `Other option:` may repeat. A missing field is refused with what's missing ("Recorded decisions need Question, Built,
    at least one Other option, and Why. Missing: Why.").
  - It replies "Recorded SHOP-23#1. Carry on; the user sees it in the review." (`--silent` prints the ID).
  - History: "Recorded a decision: Split fullName everywhere, or only in the profile form? · Built: only in the profile
    form · Other option: everywhere" (05 · T row).
- **`otaq show [KEY]`** lists a Decisions section (ID, question, built, other, why, mark, outcome). `--json` includes them.
- **`otaq prime`** lists the ticket's own decisions under "Decisions you recorded", so a resumed or sent-back agent sees
  them.
- Recording doesn't change the ticket's state, make an inbox item or notify anyone (0084: information, not an alarm).

**Done when**
- [x] the migration applies on an M16 database and keeps every row
- [x] tests: record (all fields, several other options, wrapped lines), refusals (missing field, another ticket's agent,
  a validator), ordinal numbering, `list_for_epic` grouping, the History line, `show` and `prime` output, `--json`
- [x] checks green; the commit names DEC-01, DEC-02 and decisions 0084, 0079

### T-0277 · Core: Approve keeps the decisions; Change… sends back or adds a fix child
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** DEC-05, DEC-06 (core)
- **Decisions:** 0084, 0034, 0059, 0080, 0083
- **Design:** none (no UI). The wordings are from *07 → Row · SHOP-23 review · Decisions (0084)* (the Send back message)
  and *Row · SHOP-40 epic review · Decisions (0084)* (the fix child), and from *05 → Row · Recorded decisions (0084)*
  (History).
- **Depends on:** T-0276
- **Size:** M
- **Shared files:** `src/decisions/`, the approve paths in `src/review/` (one call each), `src/review/send_back` (or
  wherever REV-04 lives: one optional "decision" argument), one request in `protocol.rs`, `EventKind` rows

**What and why**
- **Approve keeps them (DEC-05):**
  - Local merge or Open PR on a ticket or an epic marks every `open` decision of the ticket (for an epic, of every child)
    `kept`.
  - It writes one History line per ticket: "Kept by you · 1 decision · Split fullName … → only in the profile form" (05 ·
    T row). An epic writes it on each child.
  - Auto-merge (0064) is not an approval and keeps nothing. Its decisions stay `open` and show in the epic's review.
- **Change… on a ticket in review (DEC-06)**, `decision.change { id, text }`:
  - It runs the **Send back** path (REV-04, 0034) with that text as the note, **pending line comments included**.
  - The message typed in names the decision: "Review feedback on a recorded decision: “Split fullName everywhere, or only
    in the profile form?” You built: only in the profile form. Change it to: Split fullName everywhere." (07, the "back
    in Dev" frame). Line comments follow as Send back sends them today.
  - The decision becomes `changed_sent_back` with the text. History: "Changed by you → sent back · “Split fullName
    everywhere”". The Sent back marker counts it ("Sent back · 1 decision changed · round 2").
  - Refused when the ticket isn't in Ready for Review, or when it is an epic child already merged.
- **Change… in an epic's review:** for a decision of a merged child,
  - `decision.change { id, title, description }` creates a new child of the epic. The actor is the user, and the title
    and description come from the dialog (T-0288 prefills them).
  - The child joins the queue as any child added to the epic under 0080 / EPC-13. The epic leaves Ready for Review and
    becomes a track again (EPC-11).
  - The lead resumes idle (T-0279's resume on child added).
  - The decision becomes `changed_fix_child` with the new key. History on the merged child: "Changed by you → SHOP-40.7 ·
    “Change: list wishlist items in the order they were added” · added in SHOP-40's review".
- **The count for the card (DEC-03):** `decisions::open_count(ticket)` (an epic sums its children), used by T-0287 and
  T-0288.

**Done when**
- [x] tests with temporary repositories (stand-in command for the resumed session): Local merge keeps every open decision
  with one History line; auto-merge keeps nothing; Change… on SHOP-23 sends back with the message and pending comments;
  Change… on a merged child creates SHOP-40.7, moves the epic to its Track place and records the outcome; the refusals
- [x] checks green; the commit names DEC-03, DEC-05, DEC-06 and decisions 0084, 0079

### T-0278 · Core: the lead session — storage, caller, Open lead, Resume, status
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-01, LEAD-02 (Open lead), LEAD-11 (the status, core), LEAD-12 (tokens, plan-limit hold, not in
  `max_concurrency`)
- **Decisions:** 0083, 0068, 0022, 0018, 0020, 0016
- **Design:** none (no UI). The status words are from *01 · Components · Epics on the board → Row · Lead Status (0083)*
  and frame 15 ("planning").
- **Depends on:** none
- **Size:** L
- **Shared files:** migration `00NN_epic_leads.sql` (new), `src/project_agents/` (new `lead.rs`, and the lists in
  `mod.rs`), `src/session/launch.rs`, `src/caller.rs`, `src/naming.rs` (the role constant), one request per action in
  `protocol.rs` / `permissions.rs`, `plugin/skills/epic-lead/SKILL.md` (a stub), `EventKind` rows

**What and why**
- **Storage:** `project_agents.epic_id` (nullable, unique while set: one lead per epic). A lead keeps the existing states
  (running / ended / ended_by_restart).
  - Its **name** is worked out from the epic's title ("Wishlist lead"), not stored, so renaming the epic renames the lead.
  - `otaq agent name` is refused for a lead ("The lead is named after its epic").
  - Leads are **left out** of every Project agent list: "Terminals & agents", the 10 most recent ended, and
    `project_agent` drawer requests.
- **Launch:** `launch_epic_lead(epic)` and `resume_epic_lead`, like `launch_project_agent`:
  - a `project_agent` session in the project folder, `OTAQ_ROLE=epic_lead` (a new constant next to `ROLE_PROJECT_AGENT`),
    and `OTAQ_EPIC=<key>`;
  - the skill `/otaq:epic-lead` with the first message, the user's own permission mode (0011), and pre-approved
    `Bash(otaq:*)`.
  - Add `plugin/skills/epic-lead/SKILL.md` as a short stub (read-only, run `otaq prime --epic`, never edit code) so the
    launch has a skill. T-0286 writes the full skill.
- **Caller:** `Caller::EpicLead { agent, epic }`, resolved like `Caller::ProjectAgent` when the agent row has an epic.
  Until T-0280, `policy.rs` treats it **exactly as a Project agent** (one match arm), so nothing is wider than today.
- **Actions** (one request each; the user's click is the go-ahead, 0018):
  - **`lead.open { epic }`:** "Open lead". It starts the lead with no first message; the user types in the Lead tab. It
    is refused when the epic already has a live lead, or is in Ready for Review / Approved / Done ("SHOP-40 is in review;
    its lead has ended"). An ended lead is resumed instead of starting a second one.
  - **`lead.resume { epic }`:** the same conversation (`claude --resume`). It clears "ended by restart".
  - **`lead.start_for_queue(epic)`** (a function, called by T-0279): it starts the lead with the first message "SHOP-40
    was queued. Check the plan before the first child starts: `otaq prime --epic`." The status is `checking the plan`.
- **Status (`LeadStatus`):** the enum in "Words used", worked out from the row, the session's turn state (`Signals`) and
  the **reason for the current turn**. The reason is set by whoever types: `UserTyped` → planning; `CheckPlan`;
  `Wake(merge|close|stuck)` → re-planning; `Wake(question, key)` → answering KEY. Its own `ask human` or permission prompt
  → asks you. An `Event::LeadChanged { epic }` fires on each change.
- **Hooks:** as for a Project agent (no Idle, `signals/mod.rs`). A lead's own `ask human` or permission prompt is a
  group 1 inbox item like a Project agent's (`AgentNeedsYou`), naming the epic so T-0292 can open the Lead tab.
- **Tokens:** `lead_tokens(epic)` (the existing usage SUM by `project_agent_id`), added to the epic's token total and to
  today's total (0020). **Not** counted in `max_concurrency` (0016).
- **Plan-limit hold:** `lead.open` and `lead.resume` are refused while the limit is active (`refuse_during_limit`), as
  for Project agents.
- **Changes by the lead:** History events the lead causes carry its actor (`agent:<lead id>`, CLI-04), so
  `created_and_changed` works for it unchanged. T-0290 lists them.

**Done when**
- [x] the migration applies on an M16 database; one lead per epic is enforced
- [x] tests (stand-in command): open, resume, refusals (live lead, epic in review, plan limit); the role and `OTAQ_EPIC` are
  in the environment; the caller resolves to `EpicLead`; leads are absent from every Project agent list; `agent name`
  refused; every status from its inputs; tokens count on the epic and not toward the limit
- [x] checks green; the commit names LEAD-01, LEAD-02, LEAD-11, LEAD-12 and decisions 0083, 0079

### T-0279 · Core: the lead's lifecycle and the merge / close wake-up
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-02 (drag starts one), LEAD-04, LEAD-12
- **Decisions:** 0083, 0080, 0048, 0045, 0018, 0022, 0059
- **Design:** none (no UI). The wordings are from *03 → epic lead flows* 17 (the toast), 19 (the wake-up message), 27
  (the end), and *16 → Row · Epic lead after a restart (0083)* ("1 epic lead ended").
- **Depends on:** T-0278
- **Size:** M
- **Shared files:** creates `src/epic_lead/` (`mod.rs`, `wake.rs`), `src/board/epic_flight.rs` (one call in `queue_epic`),
  `src/board/epics.rs` (one call in `reevaluate_epic`, one in reopen), `src/review/mod.rs` / `close.rs` / `pr_state.rs`
  (one call each), `src/recovery/mod.rs`, `src/usage/limit.rs`, `src/session/deliveries.rs` (a `LeadWake` delivery kind)

**What and why**
- **Dragging starts one (LEAD-02):** `queue_epic` with mode Whole (the drag, ⇧⌘D, the menu) on an epic with **no lead at
  all** calls `start_for_queue`. An ended lead is not restarted by a drag; the user resumes it.
  - The queue outcome gains `lead_started: bool`. T-0289 words the toast "Queued 2 children of SHOP-45 · its lead started
    · Undo".
  - Undo of the queue doesn't stop the lead: its tokens were the go-ahead.
  - Queueing one child row (Single) starts no lead.
- **Wake-ups (`wake.rs`):**
  - `wake(epic, reason, text)` queues a `LeadWake` delivery on the live lead's session, typed by §9.3 rules (idle, input
    empty), and sets the turn reason for the status.
  - No live lead → nothing is queued; the reason is dropped.
  - Called on:
    - a child **merged into the epic branch**: Local merge, Auto-merge, or Mark merged on a child PR. Message: "SHOP-40.1
      merged: Wishlist API";
    - a child **closed**: "SHOP-40.6 closed without merging: Wishlist export (CSV)".
  - Nothing else wakes it (start, submit, validation, child added).
- **The hand-back frame** (used by T-0284 and T-0285):
  - `wake.rs` keeps a list of **open handovers** per lead (question or stop, child, the turn it was typed in).
  - When the lead's turn **ends** (Stop hook) with a handover it didn't settle, or the lead ends while one is open, the
    handover is **handed back** through a callback the owning ticket registers.
  - This ticket adds the list, the turn-end check and a test with a dummy handover.
- **End at Ready for Review:** when `reevaluate_epic` moves the epic to Ready for Review, the lead's session ends
  normally, with `state = ended` and History "Lead ended · epic in review". Its status is then `ended · epic in review`.
- **Resume when a child is added or reopened:** adding a child to an epic whose lead is ended, or reopening a child, which
  makes the epic a track again, resumes the same conversation with **no message**. The lead is idle until a wake-up.
  "Ended by restart" doesn't resume this way; it needs the user's click.
- **Restart:** a running lead becomes `ended_by_restart` in `startup()`.
  - It is not in Resume all; the existing Project agent rule stays.
  - The reboot summary counts leads separately: `StartupAgents` gains `leads_ended`, worded by T-0289 ("1 epic lead
    ended").
  - Until it is resumed, there is no live lead, so questions and stops go straight to the user (LEAD-12).
- **Plan limit:** `LeadWake` deliveries wait while the limit is active, as other deliveries do. Resume all after the
  limit continues a limit-stopped lead, as for Project agents.

**Done when**
- [x] tests (stand-in command, temporary repositories): dragging SHOP-45 starts its lead with the check-the-plan message and
  `lead_started`; Single starts none; an ended lead isn't restarted by a drag; merge and close queue one wake-up each, with
  the right text, typed only when idle; no wake-up for start, submit, validation or added child; ended at Ready for Review;
  resumed with no message when a child is added or reopened; `ended_by_restart` after a restart, not in Resume all, counted
  in `leads_ended`; the hand-back fires for a dummy handover at turn end and when the lead ends
- [x] checks green; the commit names LEAD-02, LEAD-04, LEAD-12 and decisions 0083, 0079

### T-0280 · Core: what the lead may change, `otaq suggest`, and `otaq decision mark`
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** CLI-07, LEAD-08, DEC-04 (the mark)
- **Decisions:** 0083, 0084, 0068, 0044, 0060, 0016
- **Design:** none (no UI). The wordings are from *10 → Row · Epic lead items (0083)* (the suggestion) and *05 → Row ·
  Recorded decisions (0084)* ("Marked by Wishlist lead").
- **Depends on:** T-0276, T-0278
- **Size:** M
- **Shared files:** `src/policy.rs`, `src/service/permissions.rs`, creates `src/epic_lead/suggest.rs`, `src/decisions/`
  (mark), `src/board/needs_you.rs` (the group), migration `00NN_lead_suggestions.sql` (new), `otaq-cli` `src/lead.rs` (new),
  `src/decision.rs`, requests in `protocol.rs`, `EventKind` rows

**What and why**
- **The lead's rules in `policy.rs` (CLI-07):**
  - A lead may **read anything**.
  - It may **change, link and prioritise its own epic's never-started children**: `update` (title, description, priority),
    `dep add` / `dep remove` where both tickets are children of its epic and the changed one hasn't started.
  - It may **create children of its epic**: `create --parent SHOP-40`, landing per EPC-13 / 0080, with `joined_queue_by`
    = the lead's actor so the card can say "added by Wishlist lead".
  - It may run `note`, `answer`, `ask human` (also `--pass-on`), `suggest` and `decision mark` for its epic's children.
  - Everything else is refused with "Wishlist lead can only change SHOP-40's children that haven't started" (0083). That
    covers: move, queue, take back, park, approve, merge, close, delete, editing a started child, the epic itself, other
    tickets, and `agent name`.
  - Each allowed change writes History on the child **and** on the epic with the lead's actor ("Updated by Wishlist lead ·
    description", "SHOP-40.3 waits for SHOP-40.4 · by Wishlist lead").
  - Mark the child `updated_by_agent` naming the lead, as for Project agents (0068). The card wording is T-0291's.
- **`otaq suggest close|delete <KEY> --stdin`** (LEAD-08): the reason is on stdin.
  - It stores a suggestion (migration: `lead_suggestions` with id, epic, child, kind, reason, state open / confirmed /
    dismissed, timestamps).
  - It is an inbox item in a **new group right after group 1** (`NeedsYouGroup::Suggestions`): "Wishlist lead suggests
    closing SHOP-40.6 Wishlist export (CSV)", with the reason.
  - It sends **no OS notification**: `notify.rs` skips the group. It **blocks no agent**: it doesn't change the lead's
    status and doesn't turn the track turquoise.
  - Confirm runs the usual close (0044) or delete (0060) of the child, as the user, and marks the suggestion confirmed.
    Dismiss marks it dismissed.
  - Both write History on the child and the epic ("Suggested closing SHOP-40.6 · closed by you").
  - One open suggestion per child and kind. It disappears if the child starts, closes or is deleted otherwise.
  - **Dismiss and the lead** (0083 changelog 2026-10-16): Dismiss doesn't wake the lead; `prime --epic` lists
    dismissed suggestions so it doesn't suggest the same again.
- **`otaq decision mark <ID> --source "<source>" --stdin`** (DEC-04):
  - Lead only, for decisions of its epic's children. It writes `mark_text` / `mark_source` and History on the child:
    "Marked by Wishlist lead · contradicts PAY-123: “items are listed in the order they were added”".
  - A second mark replaces the first. Refused for anyone else ("Only an epic's lead can mark a recorded decision").

**Done when**
- [x] tests for every allowed and refused lead action (a table test in `policy.rs` plus service tests with a stand-in lead
  session): never-started vs started child, its own epic vs another, the refusal text, History on both tickets with the
  lead's actor
- [x] tests: suggest close and delete; the group sits after group 1 with no notification; confirm runs the 0044 / 0060
  path; dismiss; the item disappears when the child starts
- [x] tests: decision mark by the lead, replaced by a second mark, refused for a ticket agent and for another epic's lead
- [x] checks green; the commit names CLI-07, LEAD-08, DEC-04 and decisions 0083, 0084, 0079

### T-0281 · Core: notes to running children, `otaq notes`, and the submit refusal
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-10, CLI-08, CLI-07 (`note`)
- **Decisions:** 0083, 0048
- **Design:** none (no UI). The wordings are from *06 → Row · Unread notes (0083 · T)* and *04 → Row · Unread notes & same
  file*.
- **Depends on:** T-0280
- **Size:** M
- **Shared files:** migration `00NN_lead_notes.sql` (new), creates `src/epic_lead/notes.rs`, `src/board/review.rs` (the
  submit check), `src/prime.rs` (one section), `src/session/deliveries.rs` (deliver at idle), `otaq-cli` `src/lead.rs`,
  `domain/flags.rs` (`unread_notes`), requests in `protocol.rs`, `EventKind` rows

**What and why**
- **`otaq note <KEY> --stdin`** (the lead, for a **running** child of its epic; T-0285 adds the stuck case):
  - It stores a `lead_notes` row (id, child, lead, text, created_at, read_at, `read_how` = notes / prime / typed /
    with_stop).
  - It replies "Note left for SHOP-40.4. Its agent reads it before its next commit or submit."
  - A never-started child is refused: "SHOP-40.5 hasn't started; edit its description instead (`otaq update`)". A finished
    child is refused too.
  - History on the child and the epic: "Note from Wishlist lead".
- **`otaq notes`** (the child's agent): it prints every unread note ("1 unread note from Wishlist lead (14:20): …") and
  marks them read. With none: "No unread notes."
- **`otaq prime`** includes unread notes at the top and marks them read.
- **Submit refused (CLI-08):** `otaq review submit` with an unread note is refused with "Submit refused: 1 unread note from
  Wishlist lead. Run `otaq notes`, then submit again." The user's Submit for review (CLS-05) is **not** refused.
- **Typed in when idle:** when the child goes Idle (Stop with nothing pending) with an unread note, the note is delivered
  as an automatic message (§9.3) "Note from Wishlist lead: …" and marked read. That delivery counts as the agent getting
  input, so the child is working again and doesn't become an Idle stop.
- **Flag:** `Flags.unread_notes { count, from, latest_text, latest_at }` for the card (T-0291). It clears once read.

**Done when**
- [ ] tests: note to a running child (stored unread, History on both), refused for a never-started or finished child and
  for a non-lead; `otaq notes` prints and marks read; `prime` includes and marks read; submit refused with the exact text and
  accepted after `notes`; user Submit not refused; idle delivery marks read and doesn't make an Idle stop
- [ ] checks green; the commit names LEAD-10, CLI-07, CLI-08 and decisions 0083, 0079

### T-0282 · Core: children changing the same file
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-09
- **Decisions:** 0083, 0049, 0064
- **Design:** none (no UI). The wording is from *03 → Row · same file (0083 · T)*, frame 28.
- **Depends on:** none
- **Size:** M
- **Shared files:** creates `src/epic_lead/overlap.rs`, `src/git/` (one helper if none fits), `domain/flags.rs`
  (`same_file`), one call from the signal path or the scheduler tick

**What and why**
- For each epic, the **running children's changed files** are the committed and uncommitted changes of the child's
  worktree against the merge base with the epic branch (`git diff --name-only <merge-base>` plus untracked files that
  aren't ignored).
- **When:** they are recomputed after a child's `otaq` command or hook turn end, after a merge into the epic branch, and on
  the slow tick. The result is cached per child and is cheap when nothing changed (check HEAD and the index mtime).
- **Overlaps:** each pair of running children of the same epic with a common file. `overlaps(epic) -> Vec<Overlap { a, b,
  files }>` for `prime --epic` (T-0283).
- **Flag:** `Flags.same_file: Vec<{ other: key, files }>` on each child of a pair. The card reads "same file as SHOP-40.4",
  with the files on hover (T-0291).
- It **doesn't wake the lead**, pause anyone or change dependencies. A child that stops running drops out.

**Done when**
- [x] tests with temporary repositories: two running children editing `lib/wishlist.ts` (one committed, one uncommitted) flag
  each other; different files don't; a child that merged or parked drops out; no wake-up is queued; the cache skips
  recomputing when nothing changed
- [x] checks green; the commit names LEAD-09 and decisions 0083, 0079

### T-0283 · Core: `otaq prime --epic`
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-03
- **Decisions:** 0083, 0084, 0040, 0068
- **Design:** none
- **Depends on:** T-0281, T-0282, T-0284
- **Size:** S
- **Shared files:** creates `src/epic_lead/prime.rs`, `otaq-cli` `src/prime.rs` (the `--epic` flag), one request in
  `protocol.rs`

**What and why**
- **`otaq prime --epic`** (the lead; `$OTAQ_EPIC`, or `--epic SHOP-40` from the user's terminal) prints:
  - the epic: title, description, External ID, branch and its base, place, queue mode;
  - every child, in start order: key, title, status, priority, dependencies, whether it has started, "added by", submit
    summary, notes for later tickets, recorded decisions (ID, question, built, other, why, mark), worktree path and
    branch;
  - the overlaps (T-0282);
  - open questions: the children's questions waiting for the lead, by question ID, and the questions the user hasn't
    answered;
  - unread notes it left, and suggestions with their state, including dismissed ones;
  - the user's Agent instructions, global and project (0040);
  - one line of rules: "You may change only never-started children of SHOP-40; name a written source for every answer,
    note and mark."
- `--json` gives the same data.

**Done when**
- [ ] a test renders SHOP-40 mid-flight (merged 40.1 with a decision and a mark, running 40.2 / 40.4 overlapping, 40.3
  waiting, an open question, an unread note, a dismissed suggestion) and checks each section; `--json`
- [ ] checks green; the commit names LEAD-03 and decisions 0083, 0079

### T-0284 · Core: a child's question goes to the lead first
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-06, LEAD-04 (question wake-up)
- **Decisions:** 0083, 0030, 0061, 0011, 0016, 0048
- **Design:** none (no UI). The wordings are from *03 → epic lead flows* 21–23, *05 → Row · Messages · the lead (0083)*
  and *10 → Row · Epic lead items (0083)*.
- **Depends on:** T-0279, T-0280
- **Size:** L
- **Shared files:** creates `src/epic_lead/questions.rs`, `src/board/questions.rs` (routing columns), `src/messages/mod.rs`
  (log on the epic), `src/board/needs_you.rs`, `src/attention/inbox.rs`, migration `00NN_lead_questions.sql` (new),
  `otaq-cli` `src/agent.rs` (`--pass-on`), `domain/flags.rs` (`asked_lead`, `via_lead`), `src/epic_lead/wake.rs`
  (register the hand-back), `EventKind` rows

**What and why**
- **Routing:** `otaq ask human` from a child whose epic has a **live lead** stores the question as today, with
  `with_lead = lead id`.
  - It makes **no inbox item** and no notification.
  - The child's status is **not** turquoise. `Flags.asked_lead { question }` makes the card show "asked lead · waiting"
    (rank as MSG-06, T-0291).
  - It wakes the lead: "SHOP-40.2 asks (question 7): Can guests save items without logging in? Options: 1. Logged-in users
    only 2. Guests too, kept in the browser. Answer with `otaq answer 7 --stdin`, or pass it on with `otaq ask human
    --pass-on 7 …`." The status is `answering SHOP-40.2`.
  - It registers a handover.
  - With no live lead, the question goes to the inbox as today (also after a restart until Resume).
  - **Permission prompts never go to the lead** (0011): the hook path doesn't change.
- **`otaq answer <QUESTION-ID> --stdin` by the lead:**
  - It records the answer and types it into the child (§9.3): "Answer from Wishlist lead: Yes: filled heart means saved,
    outline means not saved (plan, step 2)."
  - It clears `asked_lead` and settles the handover. The inbox count doesn't change.
- **`otaq ask human --pass-on <QUESTION-ID> [--option …] [--recommended N] --stdin`** by the lead:
  - Stdin is the lead's reason ("PAY-123 only mentions signed-in users"). The options default to the child's; `--option`
    replaces them.
  - The question becomes an "Asks you" item: "SHOP-40.2 asks · passed on by Wishlist lead", with the lead's option
    recommended (0061) and "Recommended by the lead" plus the reason.
  - The child **turns turquoise** ("Asks you", `Flags.via_lead`), with an OS notification as for any Asks you. The
    handover is settled.
  - The user's answer is delivered to the child as today, naming the choice (ATT-12).
- **Hand-back:** the lead's turn ends without `answer` or `--pass-on` for that question, or the lead ends. The question
  becomes a plain "Asks you" item (no "via"), as if no lead were live.
- **Messages (MSG-01):** the question, the lead's answer, the pass-on ("passed on to you") and the user's answer are logged
  on **the child and the epic**. The epic's Messages list shows them; T-0290 draws them.
- `otaq questions` for the lead lists the questions waiting for it.

**Done when**
- [ ] tests (stand-in sessions): routed question makes no inbox item and sets `asked_lead`; the wake-up text; `answer`
  delivers and clears; `--pass-on` makes the item with the recommended option and reason and turns the child turquoise;
  hand-back at turn end and when the lead ends; no live lead → inbox as today; Messages on both tickets; a permission prompt
  never reaches the lead; only the lead of that epic may answer or pass on
- [ ] checks green; the commit names LEAD-04, LEAD-06 and decisions 0083, 0079

### T-0285 · Core: a stuck child goes to the lead once
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-07, LEAD-04 (stuck wake-up)
- **Decisions:** 0083, 0063, 0064, 0042, 0018
- **Design:** none (no UI). The wordings are from *03 → epic lead flows* 24–25 and *10 → Row · Epic lead items (0083)*
  (the stuck item).
- **Depends on:** T-0281, T-0284
- **Size:** M
- **Shared files:** creates `src/epic_lead/stuck.rs`, `src/validation/mod.rs` (one call where the last round fails),
  `src/review/auto_merge.rs` (one call at Keeps conflicting), `src/signals/` (one call when a child turns Idle),
  `src/board/needs_you.rs` (the note and view on the item), migration `00NN_lead_stuck.sql` (new, or columns on
  `lead_notes`), `otaq-cli` `src/lead.rs` (`--view`), `domain/flags.rs` (`lead_note_sent`), `EventKind` rows

**What and why**
- **The first stop:** a child of an epic with a live lead, which has **never had its lead attempt**, reaches Validation
  failed N times, Keeps conflicting or Idle.
  - **No inbox item, no notification, no turquoise.** The child shows its usual working-card state, with the stop held.
  - The lead is woken ("SHOP-40.3 stopped: Validation failed 3 times. Last report: orders.test.ts expects `list` …. Send it
    one note with `otaq note SHOP-40.3 --stdin`, or leave it to the user."). The status is `re-planning`.
  - The attempt is marked used on the child, and a handover is registered.
  - With no live lead, or on any later stop, the stop goes to the inbox as today.
- **`otaq note <KEY> [--view "<text>"] --stdin` to a stuck child:** it counts as **Answer the agent**.
  - Validation failed: the user action `AnswerAgent`; the note is delivered with the findings and the round counter
    restarts at 0 (VAL-06).
  - Keeps conflicting: Retry merge with the note delivered to the agent, and the conflict counter restarts.
  - Idle: the note is typed in (§9.3).
  - The note is stored (`read_how = with_stop`). `Flags.lead_note_sent { text, view }` makes the card show "Lead's note
    sent · validation round 0 of 3: …" (T-0291). The handover is settled.
  - **`--view`** is the lead's view of the cause ("The agent keeps changing the API instead of the test"). It is stored
    with the note and shown only if the child reaches the user later. The design shows a "Lead's view" at the second stop,
    but the lead isn't woken then, so the view travels with the note.
- **The second stop** goes to the inbox as today. The item carries the lead's note ("LEAD'S NOTE · SENT AFTER THE FIRST 3
  FAILURES") and its view. The user's actions don't change: Move to Review anyway stays the main one (0063). The lead isn't
  woken again for this child.
- **Hand-back:** the lead's turn ends without a note to that child, or the lead ends. The held stop goes to the inbox as
  today, with no lead's note.
- Otaq itself still never nudges; only the lead's note does (0063, 0083).

**Done when**
- [ ] tests (stand-in sessions, temporary repositories): each of the three stops wakes the lead once with no inbox item; the
  note restarts the validation counter (and the conflict counter) and sets `lead_note_sent`; the second stop goes to the
  inbox with the note and view; hand-back at turn end; no live lead → inbox at once; a child whose attempt is used never
  wakes the lead again
- [ ] checks green; the commit names LEAD-04, LEAD-07 and decisions 0083, 0079

### T-0286 · Plugin: `/otaq:epic-lead`, recording decisions in `work-ticket`, and notes before a commit
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-01 (read-only by skill), LEAD-05, DEC-01, LEAD-10 (the Commit skill)
- **Decisions:** 0083, 0084, 0050, 0040
- **Design:** *06 → Row · Unread notes (0083 · T)* (what the child does)
- **Depends on:** T-0276, T-0280, T-0281, T-0283, T-0284, T-0285
- **Size:** M
- **Shared files:** `plugin/skills/epic-lead/SKILL.md`, `plugin/skills/work-ticket/SKILL.md`, `plugin/skills/commit/SKILL.md`,
  the skill tests (`crates/otaq-core/tests/plugin.rs`, `crates/otaq-cli/tests/skills.rs`)

**What and why**
- **`/otaq:epic-lead`:** the full skill.
  - **Start:** run `otaq prime --epic`. **Never edit, create or delete files, never commit**, and work only through
    `otaq`. Never use subagents or agent teams to do children's work (0083).
  - **Planning** with the user: split the epic into children of about a day with `otaq create --parent`, dependencies and
    priorities.
  - **Check the plan** (first turn after a drag): read the children against the epic and the code; fix never-started
    children before the first one starts.
  - **The merge turn (LEAD-05):**
    - read the merged child's summary, its notes for later tickets and its recorded decisions;
    - write what matters into never-started children's descriptions (`otaq update --stdin`) and name the source;
    - `otaq decision mark` a decision a written source contradicts, with `--source`;
    - add or re-order children; add a same-file wait only when the child could otherwise start before the other merges
      (no implied waits);
    - send at most one note to a running child when it matters now.
  - **Questions:** answer only from written sources and name them ("plan, step 2", "SHOP-40.1's merged code",
    "decision 0061", "PAY-123"). Pass on with a recommended option and its reason anything nothing written settles.
    Never guess.
  - **A stuck child:** one note, with `--view`. Don't retry.
  - **Suggestions:** `otaq suggest close|delete` for a child it would remove.
  - **Never:** move, queue, park, approve or merge; answer permission prompts; edit a started child. If refused, don't
    work around it.
  - End every turn idle; it is woken when needed.
- **`work-ticket`:**
  - The DEC-01 rule: a product question the code and documents don't settle → `otaq ask human` **only if you can't go on
    without the answer**. Otherwise build one option and `otaq decision record --stdin` (show the format), then carry on.
    The user's Agent instructions may add to this (0040).
  - Run `otaq notes` when you start a new step, and act on what it says.
- **`commit`:** run `otaq notes` first and apply any note before committing (LEAD-10).
- **Skill tests:** each skill names only commands that exist (the existing test pattern); `epic-lead` contains no
  file-editing instruction.

**Done when**
- [ ] skill tests updated and green; `otaq` commands named in the skills exist (checked by the test)
- [ ] *Hand check (needs a real `claude`):* a child meets a non-blocking product question and records it; a lead's note
  waits until the child's next commit (06 · T row); a lead plans SHOP-40 from "Split PAY-123 into tickets of about a day"
  without editing files
- [ ] checks green; the commit names LEAD-01, LEAD-05, LEAD-10, DEC-01 and decisions 0083, 0084, 0079

### T-0287 · Decisions in the review, and "Approve · keeps N decisions"
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** DEC-04, DEC-05 (the button)
- **Decisions:** 0084, 0059, 0063
- **Design:** *07 · Diff, review, approve & send back → Row · Decision Item (0084)* (the master, its variants and notes);
  *Row · SHOP-23 review · Decisions (0084)* (review with 1 decision; a review with no decisions); *Row · SHOP-40 epic review
  · Decisions (0084)* (grouped by child); the dark rows
- **Depends on:** T-0277
- **Size:** M
- **Shared files:** creates `otaq-view` `decision_item.rs`, `diff_page.rs` (a `decisions` field), `review_page.rs` /
  `approve.rs` / `panel_actions.rs` (the label and the ▾ menu), `src/api/review.rs`, `ui/src/screens/diff/DiffTab.tsx`, new
  `ui/src/components/DecisionItem.tsx`, `screens/panel/Panel.tsx` (the Approve label only), icon `signpost`, Gallery

**What and why**
- **View model `decision_item.rs`:**
  - `DecisionItemView { id, question, built, other, why, mark: Option<{ text, source }>, status, change: bool }`.
  - `status` is one of: none, "Kept by you · Approve, 16:40", "Changed by you → sent back · “…”", "Changed by you →
    SHOP-40.7".
  - `change` is true only in review, while the decision is open.
  - `DecisionsSection { count, hint, groups: [{ child: Option<key + title + "merged">, items }] }`. The hint reads "Approve
    keeps them all · Change… sends it back", or "· Change… adds a fix child" for an epic.
  - No decisions → **no section** (no empty list).
- **Placement:** under the agent summary, above the validation report, on the Diff tab of a ticket in review and on an
  epic's review (07 frames).
- **Approve label (DEC-05):** with N > 0 open decisions, the main button reads "Approve · keeps N decisions" ("1
  decision"), and Local merge / Open PR move into its ▾ menu. With none, it reads as before ("Approve · Local merge").
- **React:** `DecisionItem` follows the master: signpost icon, neutral colours, the lead's mark as a violet row with its
  source chip (never red), and Change… as a quiet button. Change… opens T-0288's dialog; until then the button calls a
  no-op handler that T-0288 wires.
- Gallery: every Decision Item variant.

**Done when**
- [ ] Rust tests: section absent with none; grouping for an epic; the hint per kind; the Approve label for 0, 1 and 3
  decisions and its menu; `change` only in review and open
- [ ] Vitest: the item renders each variant from a hand-written view
- [ ] matches the Decision Item row and the SHOP-23 / SHOP-40 review frames, light and dark
  - *Hand check:* SHOP-23 in review shows one decision under the summary and "Approve · keeps 1 decision" with Local merge
    in ▾; SHOP-18 shows no section; SHOP-40's review groups 40.1, 40.3, 40.4 with the violet mark on 40.1.
- [ ] checks green; the commit names DEC-04, DEC-05 and decisions 0084, 0079

### T-0288 · Change… and "Add a fix child", Decisions in the Ticket tab, History, and the card count
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** DEC-06, DEC-03, DEC-02 (History)
- **Decisions:** 0084, 0034, 0080
- **Design:** *07 → the Change… on SHOP-23 dialog*, *After: SHOP-23 back in Dev*, *Change… on 40.1's decision: add a fix
  child*, *State · Result: SHOP-40 is a track again*; *05 → Row · Recorded decisions (0084)*; *04 → Row · Unread notes &
  same file → "1 decision"*; the dark rows
- **Depends on:** T-0287
- **Size:** M
- **Shared files:** creates `otaq-view` `decision_change.rs`, `card.rs` (the count, one meta-row entry), `history.rs` (if
  rows are missing), `src/api/review.rs`, `ui/src/screens/review/Dialogs.tsx` (two dialogs), `screens/review/index.tsx`,
  `ui/src/screens/sections/` (a Decisions section in the Ticket tab), Gallery

**What and why**
- **Change… on a ticket:** the "Change this decision" dialog shows the question and "Built: … · Other option: …". Its text
  area "SEND BACK TO THE AGENT WITH" is prefilled with the first other option and editable, with the help line and "Send
  back to the agent". It calls `decision.change` (T-0277); line comments go with it.
- **Change… in an epic's review:** the "Add a fix child" dialog.
  - It shows the question, Built / Other option, and the lead's mark if any.
  - The title is prefilled "Change: <other option, as an instruction>". T-0277 writes the description as the 07 frame does:
    the decision, the mark's source text, then "Change … to that order."
  - Both fields are editable, with the help line "Creates SHOP-40.7, added by you. The epic becomes a track again…" and
    "Add fix child".
- **Ticket tab (DEC-03):** while the ticket works, a read-only **Decisions** section sits under "What and why", with the
  count and "recorded by the agent · you keep or change them in review". There is no Change…. After approval it shows the
  items with their status.
- **Card count (DEC-03):** a ticket or an epic in **Ready for Review** with N > 0 open decisions shows a neutral "N
  decisions" in its meta row. Nothing in other columns, and nothing after approval.
- **History rows** for "Recorded a decision", "Kept by you", "Changed by you → sent back" / "→ SHOP-40.7" and "Marked by
  Wishlist lead", if T-0276 / T-0277 / T-0280 didn't word them all.

**Done when**
- [ ] Rust tests: both dialogs' prefill (with and without a lead's mark), the count only in Ready for Review, the Ticket tab
  section read-only while working
- [ ] Vitest: the two dialogs render from hand-written views
- [ ] matches the 07 dialog and result frames, the 05 · T row and the "1 decision" card, light and dark
  - *Hand check:* Change… on SHOP-23 → back in Dev with "Sent back · 1 decision changed · round 2" and the message in its
    terminal; Change… on 40.1 in SHOP-40's review → SHOP-40.7 queued "added by you", SHOP-40 a track (5/6); Approve SHOP-18
    and SHOP-23 and read History.
- [ ] checks green; the commit names DEC-02, DEC-03, DEC-06 and decisions 0084, 0079

### T-0289 · The Lead Status on the epic card, the track label and the reboot summary
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-11 (the board), LEAD-02 (Open lead, the queue toast), LEAD-12 (Resume, the reboot summary)
- **Decisions:** 0083, 0080, 0068, 0045
- **Design:** *01 · Components · Epics on the board → Row · Lead Status (0083)*, *Row · Track Label with a lead*, *Row ·
  Epic Card with a lead*, and the dark rows; *03 → epic lead flows* 14, 16, 17, 18, 19, 26 and the *Interactions (0083)*
  note; *Row · 1280 × 800 check (0083)*; *16 → Row · Epic lead after a restart (0083)* (the board)
- **Depends on:** T-0279
- **Size:** M
- **Shared files:** creates `otaq-view` `lead_status.rs`, `epic_card.rs`, `epic_strip.rs` (the label's lead line),
  `epic_toast.rs` (one wording), `after_restart.rs`, `src/api/board.rs`, `src/api/notices.rs`, new
  `ui/src/components/LeadStatus.tsx`, `ui/src/cards/EpicCard.tsx`, `ui/src/screens/board/strip/Strip.tsx` (the Label), Gallery

**What and why**
- **`lead_status.rs`:** the full form ("Wishlist lead · re-planning") and the short form (icon + state; "answering 40.2")
  for every status in "Words used".
  - The tone is violet for working and idle, turquoise for "asks you", quiet for ended, with "Resume" as an action.
  - None → the labelled button "Open lead".
  - The tooltip holds the full form, what woke it ("SHOP-40.1 merged") and its tokens today (Interactions note).
  - "planning" isn't in the master row; it uses the same component with that word (frame 15).
- **Epic card (Backlog):**
  - A Lead row under the branch line, or "Open lead" when the epic has none.
  - Child rows the lead created carry "added by Wishlist lead".
  - A lead that asks you turns the card's border turquoise.
- **Track label:** the lead on its own line under the epic's name, in the short form (tracks with a lead grow from 30 to
  56 px, 0080's cap still holds at 1280). When the lead asks you, the whole label is turquoise, and no child is.
- **Actions:**
  - Click a Lead Status → opens the epic panel on its Lead tab (T-0290 adds the tab; until then, the panel).
  - "Open lead" → `lead.open`, then the panel.
  - "Resume" → `lead.resume`.
  - Double-click or ↵ on the lead line → the Lead tab.
  - The lead line is not a drag handle.
- **Queue toast:** "Queued 2 children of SHOP-45 · its lead started · Undo" when `lead_started` (frame 17).
- **Reboot summary:** "4 sessions were interrupted · 1 epic lead ended", counted separately and not under "Resume
  selected" (16).
- Gallery: the Lead Status in full and short forms, the track label and the epic card states.

**Done when**
- [ ] Rust tests: every status in both forms and tones; the card's Lead row / Open lead; the label line and turquoise rule;
  the toast with and without `lead_started`; the summary line
- [ ] Vitest: the Lead Status renders each form
- [ ] matches the 01 rows, flows 14 / 16–19 / 26 and the 1280 × 800 check, light and dark
  - *Hand check:* SHOP-40 in Backlog shows Open lead; click it (the lead starts, stand-in or real); drag SHOP-45 (toast and
    "Search lead · checking the plan"); restart the service (ended by restart · Resume on the track, the summary line);
    at 1280 × 800 the label fits.
- [ ] checks green; the commit names LEAD-02, LEAD-11, LEAD-12 and decisions 0083, 0079

### T-0290 · The epic panel: Ticket · Lead · History, the Lead tab, and Messages on the epic
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-11 (the Lead tab), LEAD-12 ("Before the restart", ended states), LEAD-06 (Messages on the epic)
- **Decisions:** 0083, 0068, 0045, 0048, 0059
- **Design:** *05 · Ticket side panel → Row · Epic panel · Lead tab (0083)* (idle, re-planning, asks you), *Row · Lead tab
  · ended* (by restart, epic in review), *Row · Messages · the lead (0083)* (on the epic, on SHOP-40.2), and the dark rows;
  *16 → Row · Epic lead after a restart* (the Lead tab "Before the restart"); *03 → flow 15*
- **Depends on:** T-0280, T-0289
- **Size:** L
- **Shared files:** creates `otaq-view` `lead_tab.rs`, `panel.rs` (`Tab::Lead`, the epic's tab list), `messages.rs` (the
  epic's threads, "passed on to you"), new `src/api/lead.rs`, `src/api/panel.rs`, `src/api/mod.rs`, new
  `ui/src/screens/lead_tab/`, `ui/src/app/registry/panel.ts`, `ui/src/screens/sections/Messages.tsx`, Gallery

**What and why**
- **The epic's tabs:** an epic's panel shows **Ticket · Lead · History**, plus **Diff** in Ready for Review (0059, accepted
  Q9). It has no Terminal, Run or Shell tab. Messages sit in the Ticket tab. Tickets keep their tabs. The tab numbers
  (⌘⌥N) follow the shown order.
- **Lead tab view model (`lead_tab.rs`):**
  - The header: the Lead Status, "read-only", and "212k today · on the epic".
  - "Changes by the lead": grouped by turn ("after SHOP-40.1 merged · 10:52", "this turn", or "whole epic · 4 changes" once
    ended), each line linked to its ticket ("Edited SHOP-40.3 · description: …", "SHOP-40.3 · waits for 40.4 · both change
    lib/wishlist.ts", "Added SHOP-40.5 · Wishlist empty state", "Suggested closing SHOP-40.6 · closed by you"). The data
    comes from `created_and_changed` with the lead's actor.
  - The body:
    - live → the terminal session;
    - no lead → the "Open lead" empty state;
    - ended by restart → "Before the restart" (REC-02, read-only from the saved conversation), with "Resume lead";
    - ended · epic in review → the last terminal read-only, "Ended · epic in review" and "Reopening a child or adding one
      resumes this conversation".
- **React `screens/lead_tab/`:** it reuses `terminal/Terminal.tsx` (typing rules as a ticket's terminal, §9.3) and the
  recovery body from `screens/terminal_tab/`. The turquoise question bar is shown while the lead asks you (as for
  tickets).
- **Messages on the epic and the child:** the answered question ("Answer from Wishlist lead …"), and the passed-on one
  ("passed on to you", then your answer). These come from T-0284's log.

**Done when**
- [ ] Rust tests: the epic's tab list per column; the Lead tab for each status; the changes list grouping; the epic's Messages
  threads
- [ ] Vitest: the Lead tab's states render from hand-written views
- [ ] matches the 05 Lead tab, ended and Messages rows and 16's "Before the restart", light and dark
  - *Hand check:* open the Wishlist lead, type to it; run `otaq update SHOP-40.3 --stdin` inside its session and see the
    change listed; restart the service and see "Before the restart" and Resume; move SHOP-40 to Ready for Review (all
    children merged) and see "ended · epic in review".
- [ ] checks green; the commit names LEAD-06, LEAD-11, LEAD-12 and decisions 0083, 0079

### T-0291 · The children's cards: lead states, notes, same file, and the flag order
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-06 (asked lead · waiting, via lead), LEAD-07 (Lead's note sent), LEAD-08 (Updated by lead, added
  by lead), LEAD-09 (same file), LEAD-10 (unread note)
- **Decisions:** 0083, 0030, 0068, 0080, 0077
- **Design:** *04 · Ticket card variants → Row · Epic children & the lead (0083)* with its flag order; *Row · Unread notes &
  same file (0083 · T)* with its flag order (ranks 8 and 9); *03 → epic lead flows* 20, 21, 23, 24, 25 and *Row · same file
  (0083 · T)* frame 28; the dark rows
- **Depends on:** T-0282, T-0285
- **Size:** M
- **Shared files:** `otaq-view` `card.rs` (the pill chain, callout, entry and meta rows), `card_page.rs`, `card_fixtures.rs`
  (a `lead_variants()` list), `epic_strip.rs` (the stops' "new · by Wishlist lead" / "new" and the turquoise stop),
  `ui/src/cards/TicketCard.tsx`, `cards/look.ts`, Gallery

**What and why**
- **The flag order (04 · T note):**
  1. turquoise, now also "Asks you · via Wishlist lead" and "Failed 3 times after the lead's note";
  2. Interrupted;
  3. Parent closed;
  4. limit reset;
  5. Validating / background;
  6. stale;
  7. asked another agent · **asked lead · waiting**;
  8. **1 unread note from Wishlist lead** (Entry row);
  9. **same file as SHOP-40.x** (Entry row, or the meta row when 8 holds it);
  10. conflicts;
  11. PR open;
  12. Parked;
  13. Updated by agent / **lead**;
  14. none.

  Update the `card.rs` header comment and the chain to match.
- **States:**
  - "Asked lead" pill with "asked lead · waiting: “…”" in the callout (not turquoise);
  - turquoise "Asks you" with "via Wishlist lead" in the Entry row;
  - "Updated by lead" pill (rank 13, only on never-started children), with "by Wishlist lead · description, now waits for
    SHOP-40.4";
  - "new · added by Wishlist lead" in the Entry row of a never-started child the lead created;
  - "Lead's note sent · validation round 0 of 3: “…”" in the callout of a working card. After a second stop, the card's
    hover shows "LEAD'S NOTE · SENT AFTER ROUND 3" and "LEAD'S VIEW" (frame 25);
  - "1 unread note from Wishlist lead", whose hover shows the note, its time and "Read at the agent's next commit (otaq
    notes). Submit is refused until then.";
  - "same file as SHOP-40.4", whose hover shows the files and "Both changed it so far. Doesn't wake the lead; neither child
    is paused.".
- **Short texts** (Notes · short texts): at 1280 the stop flag reads "new" and the pill "Updated".
- Fixtures: the 04 rows.

**Done when**
- [ ] Rust tests: each state; the rank test extended to the new order (pairs that compete for the slot: Asked lead vs unread
  note, unread note vs same file, Updated by lead vs Parked); the 1280 short texts
- [ ] Vitest: each new card state renders
- [ ] matches the two 04 rows, flows 20 / 21 / 23 / 24 / 25 and frame 28, light and dark
  - *Hand check:* produce each state on SHOP-40's children with commands run inside the lead's session (stand-in), and two
    children editing `lib/wishlist.ts`.
- [ ] checks green; the commit names LEAD-06, LEAD-07, LEAD-08, LEAD-09, LEAD-10 and decisions 0083, 0079

### T-0292 · The inbox: passed-on questions, stuck children, suggestions, and the lead's own questions
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-06, LEAD-07, LEAD-08, LEAD-11 (the lead's own items), §12.2 M17 rows
- **Decisions:** 0083, 0016, 0061, 0063, 0064, 0044, 0060
- **Design:** *10 · Needs-you inbox → Row · Epic lead items (0083)* (the four items, the close confirmation, the notes), the
  dark row; *03 → epic lead flows* 23, 25, 26
- **Depends on:** T-0285, T-0290
- **Size:** M
- **Shared files:** `otaq-view` `inbox.rs` (group titles, the Suggestions group), `inbox_page.rs` (rows), `answer_options.rs`
  ("Recommended by the lead" and the reason), `ui/src/screens/inbox/Inbox.tsx`, `ui/src/screens/review/` or wherever the
  close / delete confirmations live (one prop each), Gallery

**What and why**
- **Group 1 title** becomes "ASKS YOU · NEEDS YOU · IDLE · VALIDATION · AGENTS".
- **Passed-on question:** "SHOP-40.2 Wishlist button | asks · passed on by Wishlist lead".
  - The options are numbered, with the lead's choice preselected and marked "Recommended by the lead · PAY-123 only
    mentions signed-in users", and "Other… type your own answer".
  - The keys: 1–3 pick, ↵ sends, ⌘J opens SHOP-40.2 with its terminal focused.
- **Stuck child after the lead's note:** "SHOP-40.3 Wishlist page | validation failed 3 times · “orders.test.ts expects
  list”".
  - It shows "LEAD'S NOTE · SENT AFTER THE FIRST 3 FAILURES" with the note, "LEAD'S VIEW" with the view (when given), and
    "the lead isn't woken a second time".
  - The actions stay: Move to Review anyway (main), Give it another round, Answer the agent….
  - The same layout serves "Keeps conflicting" and Idle with their own reason and actions. No frame draws those two, so
    reuse this one.
- **Suggestions group** (after group 1, before Interrupted): "SUGGESTIONS · NO OS NOTIFICATION | 1".
  - Each row reads "Wishlist lead suggests closing SHOP-40.6 Wishlist export (CSV)", with the reason, "blocks no agent ·
    SHOP-40.6 never started", **Close…** (or **Delete…**) and **Dismiss**.
  - Close… opens the usual close confirmation (0044) with "Suggested by Wishlist lead: “…”" added. Delete… opens 0060's
    with the same line.
  - Dismiss removes the item.
- **The lead's own question or permission prompt:** a group 1 item like a Project agent's, naming "Wishlist lead ·
  SHOP-40". Opening it (click or ⌘J) opens the epic panel on its **Lead tab**.
- OS notifications: the passed-on question, the second stop and the lead's own items notify, as group 1 items do.
  Suggestions never notify.

**Done when**
- [ ] Rust tests: each item's texts and actions; the group order (1, Suggestions, Interrupted, Review); the recommended mark
  and reason; the lead's item opens the Lead tab; notification plan skips suggestions
- [ ] Vitest: the four item kinds render
- [ ] matches the 10 row and its close confirmation, light and dark
  - *Hand check:* pass on 40.2's guest question from the lead's session (inbox count +1, 1–3, ↵, ⌘J); let 40.3 fail twice
    with a note between; `otaq suggest close SHOP-40.6` then Close… and Dismiss; the lead's own `ask human` opens the Lead
    tab.
- [ ] checks green; the commit names LEAD-06, LEAD-07, LEAD-08, LEAD-11 and decisions 0083, 0079

### T-0293 · Hand-check pass and the Windows check
- **Epic:** M17 · Epic lead and recorded decisions
- **Requirements:** LEAD-01 … LEAD-12, DEC-01 … DEC-06, CLI-07, CLI-08
- **Decisions:** 0083, 0084, 0079, 0062
- **Design:** every frame named in T-0287–T-0292
- **Depends on:** T-0276 – T-0292
- **Size:** M
- **Shared files:** `tickets/review-M17.md`, `scripts/README.md` (only if setup notes change)

**What and why**
- Prepare every hand check of M17 in `tickets/review-M17.md`, with steps and frame names. Group them by screen: cards,
  epic card and strip, epic panel and Lead tab, review and decisions, inbox, terminal (notes), restart. Mark the ones that
  need a real `claude`:
  - planning from the epic card (flow A);
  - a real merge turn (flow C);
  - a child recording a decision;
  - a note read at a commit;
  - a lead answering from a written source.

  Agents don't tick hand checks.
- Sweep for what 0083 / 0084 replaced: no "an epic has no agent" text, no lead under "Terminals & agents", and no
  `otaq edit` in the code, skills, gallery or tests.
- Run `scripts/check.ps1` in the **Windows VM** (VMPal, `Z:\software-factory`). Try the Lead tab terminal, Open lead and a
  Decisions review there with WebView2. Do the same in the Ubuntu VM if it's reachable.
- A VM that can't be reached is **skipped** and recorded as "not checked", not a blocker (the M14 rule).
- Fix what fails in new commits that name the ticket they fix.

**Done when**
- [ ] `tickets/review-M17.md` lists every hand check, with "How it was run" (0079) and "Platforms checked"
- [ ] the sweep finds nothing left of the old rules
- [ ] `check.ps1` green in the Windows VM, or the VM is recorded as skipped
- [ ] checks green on the final tree; the commit names LEAD-01…LEAD-12, DEC-01…DEC-06, CLI-07, CLI-08 and decisions 0083,
  0084, 0079

## Parallel lanes (M17)
**The rules are the Depends on lists and the shared files.** This table is only the earliest grouping. The orchestrator
decides the actual order (0079):
- at most 3 teammates at once;
- never two tickets in the same folder or file at once, except the one-line registration files;
- tickets that change `package.json` or `Cargo.toml` run alone (none should).

| Phase | Tickets (side by side within a phase) | Watch out for |
|---|---|---|
| 0 | T-0276 ∥ T-0278 ∥ T-0282 | all Rust, in different modules (`decisions/`; `project_agents/` + `caller.rs` + `launch.rs`; `epic_lead/overlap.rs`). Two migrations: number at landing. T-0282 creates `epic_lead/mod.rs` only if it lands before T-0279; whichever lands second adds its line |
| 1 | T-0277 (after T-0276) ∥ T-0279 (after T-0278) | `review/` send back and approve vs `epics.rs` / `epic_flight.rs` / `recovery/`: different files, except one call each in `review/mod.rs` (check before starting both) |
| 2 | T-0280 (after T-0276, T-0278) ∥ T-0287 (after T-0277) ∥ T-0289 (after T-0279) | T-0280 owns `policy.rs` / `permissions.rs`; T-0287 is review view models and `DiffTab.tsx`; T-0289 is epic card, strip and `after_restart.rs` |
| 3 | T-0281 (after T-0280) ∥ T-0284 (after T-0279, T-0280) ∥ T-0288 (after T-0287) | T-0281 and T-0284 both add to `otaq-cli` (`lead.rs` / `agent.rs`), `flags.rs` and `deliveries.rs`: registration-style additions only, or run them one after the other if the teammate finds more |
| 4 | T-0285 (after T-0281, T-0284) ∥ T-0290 (after T-0280, T-0289) | T-0285 touches `validation/`, `auto_merge.rs` and `signals/`; T-0290 is the panel and the new Lead tab |
| 5 | T-0283 (after T-0281, T-0282, T-0284) ∥ T-0291 (after T-0282, T-0285) ∥ T-0292 (after T-0285, T-0290) | T-0291 owns `card.rs`; T-0292 owns `inbox*.rs`; T-0283 is `epic_lead/prime.rs` |
| 6 | T-0286 | the plugin skills need every command |
| 7 | T-0293 (alone) | the VMs |

The long chain is T-0278 → T-0279 → T-0284 → T-0285 → T-0291 / T-0292 → T-0293, with T-0280 feeding T-0281 / T-0284. Start
what unblocks the most: T-0278 first in phase 0, T-0279 first in phase 1, T-0280 first in phase 2.
