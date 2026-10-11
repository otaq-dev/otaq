# Review: M12's core, CLI and plugin work (UI held for Tauri)

Decision 0074 (Q36) moves the UI to Tauri before v0.1. These sessions built only the **core, CLI and plugin** work of
the remaining M12 tickets (T-0193–T-0205) on GPUI-era `main`. The app got only the changes it needs to compile. M12
isn't finished: its UI parts and the VM checks (T-0206) wait for the Tauri rebuild. T-0175–T-0192 are reviewed with
the rest of M12 later.

## 1. What was built

| Ticket | Commit | Kind |
|---|---|---|
| T-0198 · Several Project terminals per project | 7c1d085 | full (core ticket) |
| T-0193 · Panel: validation actions, "…" menu, … | none | no core work: every panel action is `ticket.validation_action` (T-0187 / T-0189); VAL-08 (a description edit cancels the validation and tells the agent) is covered by `tests/validation.rs::a_description_edit_cancels_a_title_edit_does_not` |
| T-0194 (core) · the focused-terminal rule per session for validators | c63cd43 | partial |
| T-0195 (core) · validation rounds for History; Create ticket from a note | d34f17d | partial |
| T-0196 (core) · needs-you lines, OS notifications and toast events | 8553d0a | partial |
| T-0197 · Settings screens | none | no core work: the settings and their validation exist and are tested since T-0181 (`tests/settings.rs::m12_defaults`, `m12_overrides_and_resolution`: Validate skill, Validation on/off, rounds 1–5, Auto-merge Off / Epic children only / All tickets, Project agent instructions) |
| T-0199 · Project agents (core): sessions, naming, lifecycle | 893a81e | full |
| T-0200 · Project agent permissions, `prime --project`, Created and changed (fixes T-0184) | b6b8a01 | full |
| T-0201 (core) · Project agent hooks, inbox items, notifications and tokens | 3bd9b5f | partial |
| T-0202 · Plugin: the project-agent skill | d66ce30 | full |
| T-0205 (core) · after a restart: validating tickets and Project agents | 1b145ba | partial |

Held entirely (no work): T-0203, T-0204, T-0206.

## 2. How to try it

```sh
cargo build -p crewboard-cli          # the `crewboard` command
crewboard daemon start                # uses ~/.crewboard; set CREWBOARD_HOME=/tmp/cb to try it in a scratch folder
crewboard project list
```

There is no UI for the new parts yet. Talk to the service through the CLI, or send raw requests
(`printf '%s\n' '{"protocol_version":1,"id":1,"request":{"method":"project_agent.open","params":{"project":"SHOP"}}}' | nc -U ~/.crewboard/crewboard.sock`):

- **Project agents:** `project_agent.open {project}` starts `claude` with `/crewboard:project-agent` in the project's
  folder; `crewboard attach` takes tickets only, so follow it through `session.attach {session}` (the session is in
  the reply) until the Tauri UI shows it. `project_agent.list {project}` shows the tabs and the
  recently ended; `project_agent.close {id, confirm}`, `project_agent.resume {id}`, `project_agent.rename {id, name}`.
- **Inside a Project agent:** `crewboard prime --project`, `crewboard agent name "Checkout slowness"`,
  `crewboard create … --stdin`, `crewboard ask SHOP-18 --stdin`, `crewboard ask human --stdin`.
- **Validation:** `ticket.validations {key}`, `ticket.create_from_note {key, note}`, `service.startup_report`.

Sample data: `TestService::shop_sample()` in `crewboard-core/src/service/testing.rs` builds the brief's SHOP board
(the tests use it).

## 3. Hand checks

Only the two that need a real `claude` can be done before the Tauri UI:

- **T-0191** (validate skill): submit a small ticket with validation on; check once that the validator reads
  `crewboard prime --validate`, runs the checks, and ends with `crewboard validate pass --stdin` (and once with a
  deliberate bug: `validate fail --stdin` with a file:line finding that reaches the agent).
- **T-0202** (project-agent skill): open a Project agent (`project_agent.open`), give it a first request ("why is
  checkout slow? make tickets"). Check that it runs `crewboard prime --project`, names itself with
  `crewboard agent name "<3–4 words>"` right after the request, never edits a file, and creates a Backlog ticket
  (`created_by` is `project-agent:<id>`).

Every frame check of T-0193–T-0197, T-0201, T-0203–T-0205 is redone on the Tauri UI (section 6).

## 4. Implementation choices

- **Validator permission bug (T-0184), fixed in T-0200:** `permissions::check` skipped the policy for every caller
  but `Caller::Agent`; now only `Caller::User` skips it. Socket test: "SHOP-29's validator can't change tickets".
- **Project agents (T-0199):** a second `agent name` needs `--rename` (the skill passes it only when the user asked);
  the agent may replace a fallback name, never the user's; the fallback is the request's first 4 words (≤ 40
  characters, no trailing punctuation); Resume is refused during the usage limit like Open; `claude` exiting ends the
  agent but keeps its tab until closed; the service stopping leaves it to the startup reconcile ("Ended by restart");
  an agent that never got a request is discarded at close and at startup; the status-line settings file is one per
  project.
- **Permissions (T-0200):** "still in Backlog" means in the Backlog column and never started. A Project agent may link
  a Backlog ticket to any ticket (it waits for a started one); moving a ticket into an epic stays the user's. Its
  `ask <KEY>` needs no link (any ticket, §16 rules); an answer that arrives while its session is gone stays in the
  messages table. Actor `project-agent:<id>`; the client shows the agent's current name. `ONLY_A_PROJECT_AGENT` is
  "Only a Project agent can do this".
- **`crewboard prime --project`:** the global `-P/--project <KEY>` now takes an optional value; a bare `--project` on
  `prime` means the Project agent context. `-P KEY` works as before.
- **Migrations:** 0028 (`project_agents`, `sessions.project_agent_id`), 0029 (`messages.from_agent_id`,
  `human_questions.project_agent_id`; exactly one asker per row).
- **Attention (T-0201):** the permission-prompt wait lives in memory until the turn moves on, the agent runs a
  `crewboard` command, or Enter is pressed in its terminal; Project agent items are group 1 and in the header and
  switcher counts but not in the scheduler's back-pressure (they don't count toward the agent limit, d0068); their
  notification body is "<question or prompt> — <agent name>" and a click opens the inbox.
- **Restart (T-0205):** `service.startup_report` carries the interrupted tickets, `was_validating`,
  `project_agents_ended` (ids) and `new_agents_discarded`; `service.status` is unchanged.
- **T-0167 summary:** the Project terminal's "created here" summary is unchanged; Project agents have their own
  Created and changed (`project_agent.created`).
- Earlier commits of this run (T-0194–T-0196) list their own choices in their commit messages.

## 5. Open questions

None added (Q36 is the user's resolution these sessions worked under).

## 6. Held for the Tauri UI

One entry per held or partial ticket: the UI work left (frames and open **Done when** boxes) and what the new UI
calls. Requests are socket methods; events come on `events.subscribe`.

### T-0193 · Panel: validation actions, "…" menu, confirmations, token split, editing while it validates
- **UI left:** 05 · Ticket side panel → Action bar · every state (Validating…, Needs you · validator, Validation failed 3
  times, Validation ended without a result, Auto-merge stopped, Auto-merge stop · messages); Row · validation (SHOP-29 ·
  Validating…, SHOP-32 · Validation failed 3 times, Close / Park · while validating) + Dark; Close ticket · panel "…"
  menu; Submit for review · dialog; 08 · Ticket editor → Editing while it validates (+ Dark).
- **Open boxes:** all five (panel states, "…" menu and confirmations, token split and editing warning, view-model
  tests and hand checks, checks green).
- **Calls:** `ticket.validation_action {key, action, note?}` (Validate and submit, Stop validation, Skip to Review, Move
  to Review anyway, Give it another round, Answer the agent, Retry validation); `ticket.close`, `ticket.park` (with
  `confirm`), `ticket.submit_by_user`, `ticket.update` (editing cancels a validation, VAL-08), `usage.summary` /
  `usage.sessions` (TicketUsage `tokens`, `validation`, `rounds` for the split). Events: `ticket_changed`,
  `card_status_changed`, `validation_cancelled`.

### T-0194 · Terminal tab: the Agent / Validator switch
- **UI left:** 06 · Terminal tab → Row · validator (Validator running, waiting on a permission prompt, interrupted) and
  Row · validation results in the terminal (read-only verdict, failure and cancel messages in the agent's terminal),
  + Dark.
- **Open boxes:** the switch and both sides vs the frames; typing into a running validator / read-only finished one;
  ⌘J on Needs you · validator; view-model tests and hand checks; checks green.
- **Calls:** `session.list {key}` (sessions of kind `validator`), `session.attach {key, kind: "validator"}` or
  `{session}`, `session.input`, `session.resize`, `session.focus {session, focused}` (the per-session focus rule is
  in place); the Copy attach command is `crewboard attach <KEY> --validator`. Events: `session_state`,
  `session_output`, `session_focus`.

### T-0195 · Validation reports in History and Review; Create ticket from a note
- **UI left:** 05 → Row · validation → History · validation rounds (+ Dark); 07 → Row · validation report &
  Auto-merge (Diff · Validated · round 2, Review · Failed 3 times · moved by you, Auto-merge · success, conflict handed
  back vs manual Local merge) (+ Dark); 11 → Success toasts (Created SHOP-41 from a validation note).
- **Open boxes:** all five (History rounds and auto-merge line, Diff tab Review entry and report, Create ticket with
  toast, view-model tests and hand checks, checks green).
- **Calls:** `ticket.validations {key}` (round, trigger, outcome, start, end, duration, report, notes, tokens),
  `ticket.history {key}` (the `merged_automatically` event), `diff.files` / `diff.file`, `ticket.create_from_note
  {key, note}` (user only; the toast text is the UI's).

### T-0196 · Inbox, OS notifications and toasts for validation and Auto-merge
- **UI left:** 10 · Needs-you inbox → Row · M12 item types → New item types · group 1 (M12), Dark · new item types;
  In context · 1440 window; Inbox popover · mixed, 2 projects; the group label "ASKS YOU · NEEDS YOU · IDLE ·
  VALIDATION · PROJECT AGENTS"; 11 → Success toasts (the two toasts).
- **Open boxes:** inbox frames; inline actions; the two toasts; the tests box (needs-you and notify-plan tests exist,
  view-model tests and hand checks don't); checks green.
- **Calls:** `inbox.list` (items with `reason`, `status`, `group`; `counts`), `inbox.counts`, `inbox.answer`,
  `ticket.validation_action` for the inline actions. Events: `inbox_changed`, `merged_automatically {project, key,
  title, message}`, `validation_cancelled {project, key, title, message}`, `open_from_notification`.

### T-0197 · Settings screens: Validation & Auto-merge, Project agent instructions; onboarding line
- **UI left:** 15 · Settings → Global (nav "Validation & Auto-merge", Workflow skills → Validate, Validation, rounds,
  Auto-merge, Project agent instructions); Project · fresh; Dark global / project; Row · validation & Auto-merge per
  project (+ Dark); 14 · First run → 4 · Done · empty board (+ Dark).
- **Open boxes:** all five.
- **Calls:** `settings.get` / `settings.set`, `project_settings.get` / `project_settings.set` (problems list the
  invalid values; rounds 1–5; Auto-merge `off` / `epic_children` / `all`), `onboarding.state`.

### T-0201 · Project agent hooks, inbox, notifications and tokens
- **UI left:** 10 → New item types · group 1 (M12) (the Project agent "Import GitHub issues" item) and its inline
  answer; 11 → OS notifications → macOS · M12 (the two Project agent notifications) checked by hand.
- **Open boxes:** the tests box (it includes the hand check against the frames).
- **Calls:** `inbox.list` → `project_agents` (project, project_name, agent, name, status, reason, question, since) and
  the counts; `project_agent.answer {id, choice, clear}` (`needs_confirmation` with unsent text);
  `session.focus` for the agent's terminal. Events: `inbox_changed`, `project_agent_changed`.

### T-0203 · Terminals & agents: header button, drawer, tabs, + menu, ⇧⌘T / ⇧⌘A (held entirely)
- **UI left:** 02 · App shell → Terminals & agents button · a Project agent waits (+ Dark); 17 · Project terminals (Empty
  · shell prompt, Plain shell use, Ended, Cycle rejected by the tool, Adding a ticket to an epic, After a restart ·
  terminals are gone); 18 → Row · lifecycle → + menu; 01 · Components → Keyboard map (`KeyAction::NewProjectAgent`,
  ⇧⌘A / Ctrl+Shift+A; ⇧⌘T "Terminals & agents (last-used terminal)").
- **Open boxes:** all six.
- **Calls:** `project_terminal.open {project, new}` / `.list` / `.close {session}` / `.created` / `.dismiss`;
  `project_agent.open {project}` / `.list {project}` (`open` tabs and `recent`); `session.attach {session}`,
  `session.input`, `session.resize`, `session.focus`. The orange dot: `inbox.list` → `project_agents` non-empty for the
  project. Events: `session_state`, `project_agent_changed`, `inbox_changed`.

### T-0204 · Project agent views: New agent, rename, close, Created and changed, recently ended (held entirely)
- **UI left:** 18 · Project agents & terminals → Row · investigating, Row · importing, Row · dark, Row · states (New
  agent · no request yet, Waiting for you · permission prompt / a question, Refused action, Rename · your name wins),
  Row · lifecycle (Close mid-turn · confirm, Ended normally, Ended by restart · Resume, Recently ended · list, Delete a
  created ticket · confirm) (+ Dark); 04 · Ticket card variants → Backlog · created by Project agent.
- **Open boxes:** all six.
- **Calls:** `project_agent.list` (ProjectAgent: `name`, `name_source`, `state`, `got_request`, `mid_turn`,
  `limit_stopped`, `closed`, `question`, `needs_you`, `focused`, `tokens`, `session`), `project_agent.rename {id, name}`,
  `project_agent.close {id, confirm}` (`needs_confirmation` "<name> is in the middle of a turn. Stop it and close
  it?"; an unused New agent closes without it), `project_agent.resume {id}`, `project_agent.created {id}`
  (`created` with `deletable`, `edited`, `dependencies`, `summary`), `ticket.delete_preview` / `ticket.delete` (the
  0060 confirmation). Cards: `created_by` / `updated_by` = `project-agent:<id>` → the agent's name from the list;
  `ticket.agent_edits` counts its edits. Events: `project_agent_changed`, `ticket_changed`.

### T-0205 · After a restart: validating tickets and Project agents in the summary
- **UI left:** 16 · Recovery & cleanup → Opening after a reboot; Row · validation & Project agents after a restart →
  Validating ticket after a restart (SHOP-29) (+ Dark); `view_models/after_restart.rs` counts ("8 sessions were
  interrupted · 2 Project agents ended"), the agent rows with Resume, the "unused New agent is gone" line, and "was
  Validating · Resume = fresh validation".
- **Open boxes:** the frames; the view-model tests and hand checks.
- **Calls:** `service.startup_report` (`interrupted`, `was_validating`, `project_agents_ended`,
  `new_agents_discarded`, `unclean`), `project_agent.list` (state `ended_by_restart`), `project_agent.resume {id}`,
  `ticket.resume {key}` (a was-Validating ticket starts a fresh validation, T-0188), `tickets.resume_all`.

### T-0206 · M12 checks on Windows and Linux (held entirely)
After the Tauri UI: `scripts/check.ps1` / `scripts/check.sh` in the VMs; validator and Project agent callers on
Windows (the job-object path) and Linux; Ctrl+Shift+A by hand.
