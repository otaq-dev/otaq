# Prompt: finish M12's core, CLI and plugin work (the UI waits for Tauri)

Start **one fresh Claude Code session** in this repository and paste everything below the line. The session builds the
non-UI part of the remaining M12 tickets on `main`, one ticket at a time, then stops. It must be the only session building
code on `main`. The pen.dev connection is optional: it's only used to read one frame's notification texts (§4, T-0196).

---

You are the implementing agent for the **rest of milestone M12** of **Crewboard**, a Rust kanban desktop app that drives
Claude Code agents. Earlier sessions built T-0175–T-0192. The user decided (decision 0074, question Q36) that the UI moves
from GPUI to Tauri before v0.1. So **you build only the core, CLI and plugin work** of the remaining M12 tickets. The UI
parts are held for the Tauri rebuild, which starts after you. **You write no new GPUI screens.**

## 1. Read first
1. `CLAUDE.md`, `decisions/README.md`, and **`decisions/0074-ui-on-tauri.md`** ("The rest of M12" says what's in and out),
   plus 0063–0068 (M12's decisions).
2. **`prompts/implementer.md`** is your runbook: follow everything below its `---` line, except where this prompt says
   otherwise.
3. `tickets/questions.md` **Q36** (the resolution you're working under), the M12 tickets T-0175–T-0206 at the end of
   `tickets/v0.1-tickets.md` (after M13), `tickets/progress.md`, and `git log` from `e01fa5b` on (what M12 built so far, with
   implementation choices).

## 2. Where the work stands
- **Committed**, one commit per ticket in file order: T-0175 … T-0192 (HEAD `d43af35`, "T-0192: Cards: validation,
  Auto-merge and Review-entry states").
- **T-0192 has no progress row yet.** Each ticket's row goes into the next commit, because the hash is only known after
  committing. Add this row with your first commit:
  `| T-0192 | Cards: validation, Auto-merge and Review-entry states | M12 | d43af35 | the 12 variants against 04 · Ticket card variants · Light / Dark (gallery card grid and the app); the legend against 01 · Foundations → Status Legend → Pills · M12 | Flags gains validating_round, findings, conflict_files (no migration); status filter gets is:validating (deferred from T-0179); Base diverged / Checkout dirty read like Keeps conflicting; card buttons send ticket.validation_action; the board request handler's future is boxed in connection.rs (debug-build stack overflow in the CLI's write test) |`
- **T-0198 is built but not on `main`.** It's one scratch commit, `2b98036`, on top of T-0191 (`8487629`), in the worktree
  `/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/9f09fbb8-5923-47c3-a22b-1010ce312d8f/scratchpad/wt-core`
  (branch `m12-core-scratch`). fmt, clippy and its targeted tests pass; the full `scripts/check.sh` has **not** run on it.
  - **Shells** (`terminals/shell.rs`) keeps the open Project terminals in memory (session id → project and number). A
    restart forgets them.
  - **`open_project(project, new)`** returns `ProjectTerminal { number, name, session }`.
    - With `new`, it always starts a new shell.
    - Without it, it returns the newest running terminal, else the newest ended one that's still open, else a new one.
  - **Names** are "Terminal N", where N is the lowest number no open terminal of that project has.
  - **New functions:** `project_terminals(project)`, `latest_project_terminal(project)` and `close_project_terminal(session)`.
    Closing stops the shell and frees its number.
  - **Protocol:**
    - `project_terminal.open`: its `reopen` param is renamed `new` (the alias `reopen` still works), and the reply is
      `Reply::ProjectTerminal`.
    - New `project_terminal.list` (`Reply::ProjectTerminals`) and `project_terminal.close` (`SessionIdParams`).
    - Both new requests are wired in `permissions.rs` (list is Read; close is UserOnly "close terminals"), `connection.rs`,
      `board_requests.rs` and `every_request.rs`.
  - **Attach:** `session.attach {project}` goes through `shells.latest_project_terminal`, and
    `Sessions::latest_project_terminal` is removed.
  - **The app's change** is only the compile fix in `project_terminal.rs` (it reads the new reply).
  - **Tests:**
    - New `crates/crewboard-core/tests/project_terminals.rs` covers open/list/close/naming, the protocol round trip,
      surviving a closed window, and being gone after a service restart.
    - `created.rs` and `crates/crewboard-cli/tests/terminals.rs` are updated.
  - **Unchanged:** the T-0167 created-tickets summary (T-0200 moves it).

## 3. What you build, in this order
**Core, CLI, plugin and protocol only.** In `crates/crewboard-app` you may make only the changes that keep it compiling and
its existing tests passing: a new enum arm, a renamed reply, struct literals in fixtures. Add no screens, widgets,
keyboard shortcuts or new view-model features. Where the view-model logic will live after the migration isn't decided yet
(0074 follow-ups).

1. **T-0198** (full ticket). Bring it over first; it no longer waits for T-0193–T-0197.
   1. Make the patch: `git -C <wt-core> diff 8487629 2b98036 --binary > <your scratchpad>/t0198.patch`.
   2. Apply it in the main tree with `git apply --3way`.
   3. Add the T-0192 row, tick the boxes, stage, run the staged check, and commit.
   4. Afterwards, remove the worktree (`git worktree remove <wt-core>`) and the branch (`git branch -D
      m12-core-scratch`). The scratch commit has no attribution lines; it is only diffed, never merged.
2. **The core parts of T-0193–T-0197**, in file order. A ticket whose core part already exists gets a test if one is
   missing, or nothing at all (see §5).
   - **T-0193:**
     - Every panel action the frames name must already be a request (T-0187, T-0189).
     - Check that editing a validating ticket's description cancels the validation and tells the agent what changed
       (VAL-08), and add a test if none covers it.
     - Probably no new code.
   - **T-0194:**
     - The focused-terminal rule per session for **validator** sessions (ATT-03): while the validator's terminal is
       focused (in the app or attached), the validator raises no orange, inbox item or notification. Add the core test the
       ticket asks for.
     - `crewboard attach SHOP-29 --validator` exists (`crates/crewboard-cli/src/attach.rs`); make sure a test covers it.
   - **T-0195:**
     - Whatever the protocol lacks so a client can show History's validation rounds (outcome, duration, tokens, report) and
       the "Merged automatically" line.
     - The core side of **Create ticket** from a "Not covered by any ticket" note: a user-only request, or the existing
       `ticket.create` if it's enough, that makes a prefilled Backlog ticket in the source ticket's epic. The note parsing
       is `note_of` in `board/validation.rs`.
     - A policy test that the validator can never create tickets.
   - **T-0196:**
     - The needs-you derivation and the **notify plan** for M12 items. Today `attention/notify.rs::one` falls through to
       "session interrupted" for `ValidatorNeedsYou`, `ValidationFailed`, `ValidationNoResult` and `AutoMergeStopped`, which
       is wrong.
     - Notify for the validator's permission prompt, "failed validation 3 times" and "ended without a result". Never notify
       for failed rounds that go back to the agent, for successful auto-merges, or for the focused ticket.
     - Take the texts from the frame `11 · Banners, toasts & OS notifications → OS notifications → macOS · M12`, read-only
       through pen.dev. If pen.dev isn't available, use `design-briefs/11-banners-and-notifications.md` (M12 additions) and
       defer the exact wording (runbook §3.2).
     - **The ticket and the brief disagree about an Auto-merge stop:** the brief notifies for it, but T-0196's list leaves
       it out. The frame decides. If the frame doesn't settle it, defer it as a question.
     - Make sure the service sends events for the two toasts ("SHOP-40.1 merged automatically into the Wishlist branch",
       "Validation cancelled: SHOP-29's code changed"), so a client can show them.
   - **T-0197:** check that the settings exist and validate as the screens need (Validate skill, Validation on/off, rounds
     1–5, Auto-merge Off / Epic children only / All tickets, Project agent instructions; mostly T-0181). Probably no new
     code.
3. **T-0199** Project agents (core), full ticket. The previous session's design notes (§6) are yours to change.
4. **T-0200** Project agent permissions, `prime --project`, Created and changed, full ticket. It also fixes the **validator
   permission bug** from T-0184:
   - `service/permissions.rs::check` returns Ok for every caller where `!caller.is_agent()`. `is_agent()` is only
     `Caller::Agent`, so a validator's requests skip the policy and it could change tickets, although 0063 makes it a
     read-only judge. `policy::authorize` handles validators correctly; it just never runs for them.
   - The fix: skip only `Caller::User`.
   - Add a socket-level test that a validator is refused a change ("SHOP-29's validator can't change tickets").
   - Name it in the commit as a fix of T-0184.
5. **T-0201**, everything except the drawn inbox rows:
   - `crewboard hook` for the Project agent role without `CREWBOARD_TICKET`;
   - `map_hook` (permission prompt → Needs you, `ask human` → Asks you, never Idle);
   - the needs-you items labelled with project and agent name;
   - answering inline delivers into the agent's session;
   - the header and switcher counts in the service's agent counts;
   - the notify plan ("shop-web · Project agent asks a question" / "needs permission");
   - the focus rule;
   - tokens per agent and in today's total.
6. **T-0202** Plugin: the project-agent skill, full ticket.
7. **T-0205**, the core part: what the after-restart summary needs from the service, including Project agents.
   - The count ("2 Project agents ended") and their rows with Resume.
   - That an unused "New agent" is gone.
   - A validating ticket's row ("was Validating · Resume = fresh validation").
   - Resume from the summary starts a fresh validation or resumes the Project agent.

**Held entirely, no work now:**
- T-0203 (the Terminals & agents drawer and ⇧⌘T / ⇧⌘A);
- T-0204 (the Project agent views; its actions come from T-0199 / T-0200);
- T-0206 (the VM checks).

Don't run the VMs.

## 4. pen.dev (only for T-0196's texts)
Load the tools with ToolSearch: `select:mcp__pencil__get_app_state,mcp__pencil__execute,mcp__pencil__read_skill`. The file
is `/Users/nevzatopcu/.pencil/documents/29a4e0bb-601e-4c4e-b9f8-65ab4fd4451a/crewboard.pen`.
- **Read only:** never modify it, and never `Read` or `grep` a `.pen` file.

## 5. Recording split tickets
- **Commits:** a ticket you build only partly gets one commit titled `T-0196 (core): …`.
  - The message names only the requirement IDs you covered.
  - It says "UI part held for the Tauri rebuild (0074, Q36)" and lists what's left.
- **Progress rows:** add one row in `tickets/progress.md` with Milestone `M12 (core)`. Notes start with "UI held (0074):"
  and name the held parts.
- **No core work:** a ticket with nothing to build gets no commit and no row. List it in the review file.
- **Ticking boxes:** tick only the **Done when** boxes you verified. A box that mixes core and UI stays unticked; say in
  the progress row which half is done. Hand checks stay unticked.
- **Review file:** write **`tickets/review-M12-core.md`** (not `review-M12.md`; M12 isn't finished). It has the runbook's
  five sections (§5), plus a sixth: **"Held for the Tauri UI"**, one entry per held or partial ticket. Each entry gives:
  - the UI work left, with its frames and the **Done when** boxes still open;
  - the requests and events the new UI will call for it.

  The migration plan is written from this section, so make it complete.

## 6. Design notes for T-0199 (from the previous session; change them freely)
- **Migration `0028_project_agents.sql`:**
  - A `project_agents` table with id, project_id, name, name_source (agent / fallback / user), claude_session_id, state
    (running / ended / ended_by_restart), created_at and ended_at.
  - Also `first_request` (NULL means it never got a request, which is how an unused "New agent" is spotted) and
    `limit_stopped`.
  - `sessions.project_agent_id` (ON DELETE SET NULL). `sessions.kind` already allows `'project_agent'` (migration 0026).
- **Session and launch:**
  - `SessionKind::ProjectAgent` (`runs_claude`).
  - `launch_project_agent` in `session/launch.rs`: like `launch_validator`, but cwd is the repo,
    `CREWBOARD_ROLE=project_agent` (`naming::ROLE_PROJECT_AGENT`), and no ticket or PORT env.
  - `statusline::write_settings` takes a `TicketKey`, so it needs a variant for project agents.
- **Caller:** `Caller::ProjectAgent { project, agent_id, session }`, resolved through project-agent session roots like
  `project_terminal_holding`. `agent_roots` needs a ticket, so Project agents aren't in it.
- **Hooks:** the CLI already sends the role without a ticket. In `connection.rs`, route a ProjectAgent caller's hooks to a
  `ProjectAgents::hook`.
  - Update the turn signals for idle and mid-turn.
  - UserPromptSubmit: ignore the skill invocation itself (`naming::skill("project-agent")`); otherwise store the first
    request.
  - Stop after the first request: if the agent hasn't named itself, use the request's first words (the fallback).
  - StopFailure with the limit: set `limit_stopped` and make the limit active. `PlanLimit::hit` needs a ticket, so it
    needs a variant.
  - `PlanLimit::resume_all` also queues "continue" for limit-stopped Project agents.
- **`crewboard agent name "<name>"`** is a new CLI command and request.
  - It sets the name once. A second call is refused unless an explicit rename flag is given (an implementation choice;
    name it in the commit).
  - It's always refused after the user has renamed the agent: the user's name wins.
- **Lifecycle:**
  - Open is refused while the usage limit is active.
  - Close: an agent that never got a request is deleted without confirmation; otherwise it ends.
  - Startup reconcile: running agents become ended_by_restart, and those that never got a request are deleted.
  - The recent-ended list holds the 10 newest. Resume uses `claude --resume`.
  - Project agents never count toward `max_concurrency`.
- **Tests:** expose `project_agents` on `TestService`. Test end to end in `crates/crewboard-cli/tests` with `sh` standing in
  for `claude`, so `$CB agent name` and `$CB hook …` run inside the session.

## 7. Code facts
- **Validation:**
  - `validation/mod.rs` is the Validation service.
  - `board/validation.rs` has the pure transition, the DB functions, `note_of` and `findings_message`.
  - `domain/validation.rs` has the exact texts, `ReviewEntry`, `ValidationOutcome`, `AutoMergeStop` and `ValidationAction`.
- **Auto-merge** is in `review/auto_merge.rs`.
- **Usage split:** `usage/mod.rs` `TicketUsage { tokens, validation, rounds, agent() }`.
- **A new request needs:**
  - `#[serde(rename = "...")]`;
  - an arm in `method()`;
  - a line in `service/permissions.rs`;
  - routing in `connection.rs` and in `board_requests.rs`'s "return None" list (for terminal and session requests);
  - an entry in `crates/crewboard-core/tests/every_request.rs` (and `base_params` for new fields).
- **Caller and role:** `Caller::Validator` exists. `CREWBOARD_ROLE` comes from `naming::role_env_var()` (`ROLE_VALIDATOR`,
  `ROLE_PROJECT_AGENT`), and `HookParams.role` is passed.
- **Migrations:** 0022–0027 are used; the next is **0028**.
- **Struct literals:** adding a field to `Ticket`, `Flags`, `SpawnSpec` or `SessionInfo` breaks many literals in core tests
  and the app (`card_fixtures.rs`, view-model tests). Avoid it, or fix them all.
- **Debug-build stack overflows:** if a test aborts with "has overflowed its stack", box the large future (`Box::pin`). It
  happened in T-0187 and T-0192. The board handler is already boxed in `connection.rs`.

## 8. How to work
- **Disk first.** Run `df -h .` before building anything. On 2026-10-11 only about **6.5 GB** was free.
  - The main `target/` is about 147 GB, and `target-check` (below) about 31 GB.
  - Under 20 GB free, **stop and ask the user** before building. Suggest `cargo clean` on the main `target/`: it's only a
    build cache and rebuilds in minutes.
  - Never create a new `CARGO_TARGET_DIR`.
- **Before staging:** `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and the targeted tests.
- **The check runs on the staged state, in a separate worktree,** and takes about 5–6 minutes. Run it in the background and
  wait for it.
  - Helper:
    `/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/9f09fbb8-5923-47c3-a22b-1010ce312d8f/scratchpad/staged-check.sh <label>`.
    It applies `git diff --cached` to the reusable detached worktree
    `/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/8eee7bb8-2f16-4b1b-8185-47b44eb187b2/scratchpad/wt-check`
    and runs `scripts/check.sh` there.
  - Run it with
    `CHECK_TARGET=/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/8eee7bb8-2f16-4b1b-8185-47b44eb187b2/scratchpad/target-check`;
    that folder is already built for this worktree.
  - It prints the exit code and totals, and writes `check-<label>.log` into the 9f09fbb8 scratchpad.
  - Copy the helper into your own scratchpad if you want to change it.
- **Ticking boxes:**
  `python3 /private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/8eee7bb8-2f16-4b1b-8185-47b44eb187b2/scratchpad/tick.py T-0xxx "<text of the box>"`.
- **Staging:** stage your own paths only, for example `git add crates plugin tickets/progress.md tickets/v0.1-tickets.md`.
- **Committing:** commit with a plain `git commit` from the index, not `git commit -- <paths>`. The message follows the
  format in `git log`: requirement IDs, decisions, what was built, implementation choices, hand checks pending. End it with
  the attribution lines your session gives you for commits.
- **Files that are NOT yours:** another session is changing `DESIGN.md`, `CLAUDE.md`, `decisions/*` (including 0070–0074),
  `design-briefs/*`, `scripts/README.md`, `scripts/licenses*`, `THIRD-PARTY-LICENSES.md`, `tickets/handcheck-*.md` and
  `prompts/m12-core.md`. Never stage or commit them.
  - `tickets/questions.md` already has the user's uncommitted **Q36**. If you add a question (next number Q37), committing
    the file with Q36 in it is fine.

## 9. Rules that always apply
- **Naming and tests:**
  - Every product, command, env and plugin name comes from `crewboard_core::naming`.
  - Tests set `CREWBOARD_HOME` to a temporary folder and never start a real `claude`.
- **Database:** schema changes go in NEW migrations. sqlx 0.9 needs `sqlx::AssertSqlSafe` for dynamic SQL.
- **GPUI:** it stays pinned (see `review-M1.md`), and you don't upgrade or remove it. The Tauri migration replaces it later.
- **Hand checks** of earlier milestones stay unticked.
- **Git:**
  - Other sessions may commit to `main`. Commit only your own files.
  - Don't change git config, never rewrite history, never push, never stash.
- **`scripts/check.sh`** must be green before every commit.
- **Design documents:** runbook §7 applies. Don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/`, `CLAUDE.md`
  or `crewboard.pen`. Respect SDD §2. Don't install anything.
- **Questions:**
  - Non-blocking ones are deferred as runbook §3.2 says.
  - Stop only for a blocker (runbook §7), and ask the user one topic at a time, with the options, a concrete example and
    your recommendation.
  - If a core change would need a UI decision, defer it.
- **Low context:** stop after a committed ticket with `tickets/progress.md` up to date, and tell the user where you stopped.

## 10. Before you stop, check it yourself
- `git status` shows none of your files uncommitted.
- `git log` has one commit per built ticket, in the order of §3, plus the review commit. Every commit names its requirement
  IDs and decisions.
- `scripts/check.sh` is green on the final tree.
- `tickets/progress.md` has a row for T-0192 and for every ticket you built, fully or partly.
- `tickets/review-M12-core.md` exists with the five runbook sections plus "Held for the Tauri UI".
- No hand checks are ticked.
- Nothing under `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md` changed in your commits.
- No `crewboard` process from the tests is still running (`pgrep -fl crewboard`), and `~/.crewboard/` wasn't touched.
- The scratch worktree `wt-core` and the branch `m12-core-scratch` are removed. **Keep** `wt-check` and `target-check`:
  the UI migration will reuse them.

## 11. Final report
Reply with:
- the ticket → commit table, marking which tickets were built only partly;
- the hand checks the user must do (T-0191 and T-0202 need a real `claude` run);
- the implementation choices worth the user's attention;
- any questions you deferred;
- the "Held for the Tauri UI" list, briefly;
- the final `scripts/check.sh` result.

Don't start the UI migration.
