# M4 · Agent signals — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0064 | Caller identity by process ancestry, `--actor` | e233026 |
| T-0065 | Permission scope enforced by the service | acd8db1 |
| T-0066 | Hook entry point and hook-to-status mapping | 2bf61e2 |
| T-0067 | Orange family, clearing rules, last signal | b31294a |
| T-0068 | CLI read commands: show, list, ready, blocked, project | 5997ef3 |
| T-0069 | CLI create and update (incl. `-f`, `--parent`, `--type epic`) | deaa3e5 |
| T-0070 | CLI dependency commands | 5295608 |
| T-0071 | `crewboard prime` | 50a111b |
| T-0072 | `ask human` and `review submit` (refuses uncommitted files) | 734ed8e |
| T-0073 | Plugin: manifest, hooks, work-ticket and commit skills | aaf96aa |
| T-0074 | Ticket-change note when the user edits a working ticket | 7d8df06 |
| T-0075 | Cards and panel: orange family, background work, last signal | 37363f9 |
| T-0076 | Live agent activity on the board: created-by-agent, toasts, change-note hint | 6836e23 (also unticks a visual item of T-0075) |
| T-0163 | `ask human` answer options: CLI flags and storage | 22564f4 |
| T-0164 | Skills: offer 2–4 options with a recommendation | e3cffcf |

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)      # a short path: the socket lives in it
```

**Without claude (the CLI, signals, board).** Start the service with a PATH that has git but no `claude`, add a
repository and play the agent from your own terminal:

```sh
PATH=/usr/bin:/bin target/debug/crewboard daemon start
target/debug/crewboard-app &
# add a project in the app (or: printf '%s\n' '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"/path/to/repo"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock)
B=target/debug/crewboard
$B project list
$B -P SHOP create "Price table" --done-when "sorted by price" --done-when "works on mobile widths"
$B -P SHOP create -f tickets/v0.1-tickets.md --silent | head      # the tickets file imports (167 tickets)
$B -P SHOP list --tree ; $B -P SHOP blocked ; $B dep tree SHOP-3 ; $B prime SHOP-3
CREWBOARD_TICKET=SHOP-1 $B create "Follow-up"   # "created by agent": origin label + toast in the app
```

Commands run from your own terminal are **the user** (full access) even with `CREWBOARD_TICKET` set; only processes
inside an agent session are the agent. To see the agent's permission scope and the hooks without claude, the tests do
it with a shell as the agent session (`crates/crewboard-core/src/service/testing.rs`, `AgentShell`).

**With the real claude.** Stop the service, start it with your normal PATH, move a ticket to Dev:

```sh
target/debug/crewboard daemon stop
CREWBOARD_LOG=debug target/debug/crewboard daemon start   # debug: raw hook payloads go to the log
target/debug/crewboard-app
```

The agent now starts with `/crewboard:work-ticket` (the skill exists), runs `crewboard prime`, works, commits with
`/crewboard:commit` and submits with `crewboard review submit --stdin`. Hooks drive the card: Needs you on a permission
prompt, Idle when it stops, Working in background while background tasks run, Working again on your next prompt. The
raw payloads are in `$CREWBOARD_HOME/logs/crewboard.<date>.log` (`hook` lines at debug level).

**Cleaning up:** `target/debug/crewboard daemon stop`, then remove `$CREWBOARD_HOME` (worktrees live inside it; `git
worktree prune` in the repo afterwards).

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0073 · Plugin with the real claude**
- [ ] In a ticket's worktree: `CREWBOARD_TICKET=SHOP-12 claude --plugin-dir <repo>/plugin`, then `/crewboard:work-ticket`:
  the skill loads, the agent runs `crewboard prime` first, follows the printed Agent instructions, commits with
  `/crewboard:commit` and submits with `crewboard review submit --stdin`. (Run by hand from your terminal you are the
  user: nothing on the board changes status from its hooks; see implementation choices.)
- [ ] A plain `claude` session (no `CREWBOARD_TICKET`, with and without `--plugin-dir plugin`) shows no hook errors and
  nothing changes on the board.
- [ ] With the service started by the app and a ticket moved to Dev: a permission prompt turns the card **Needs you**
  with Claude's message; answering it with Enter clears it; the turn's end turns it **Idle** with its last line (or
  keeps **Asks you** after `ask human`); a background task (`run_in_background`) shows **Working in background**. Check
  the debug log's `hook` payloads against the mapping, especially `background_tasks[].status` values, which task types
  are helper agents, and whether `agent_id` appears on a subagent's Notification.

**T-0164 · Answer options with the real claude**
- [ ] Ask the agent to make a choice with clear alternatives: it runs `ask human --option … --recommended N --stdin`;
  ask an open question: it runs `ask human --stdin` without options. (The inbox/card option UI comes in M7.)

**T-0075 · Cards and panel**
- [ ] `04 · Ticket card variants · Light` and `· Dark` → **Working · needs you** (SHOP-22: mono prompt, orange),
  **Dev · Asks you** (SHOP-23: the question), **Dev · Idle** ("Ended its turn: “…”"), **Dev · working in background**
  (blue pill, cpu, "background: test run, 1 helper agent · 6 min"), **Dev · background · scheduled wakeup** (alarm clock,
  "wakes up at 4:10pm"), **Dev · needs you · helper agent** (bot icon, "helper agent asks: …"), **Working · stale signal**
  ("Quiet N min" pill). Reach each with the real claude, or set the state by hand: `sqlite3 $CREWBOARD_HOME/crewboard.db
  "UPDATE tickets SET state='idle', board_column='dev', signal_text='Should I also update the tests?' WHERE number=3"`
  (states: `needs_you`, `asks_you`, `idle`, `working_in_background`; for background set
  `signal_text='background: test run, 1 helper agent', signal_since=<ms>`; for a wakeup leave `signal_text` empty and set
  `wakeup_at`; for stale set `state='working', last_signal_at=<ms 6 min ago>`), then switch projects to refetch.
- [ ] The card's age ("40s", amber after 3 min) and the background minutes tick every second without a refetch; the
  card turns "Quiet 5 min" after five silent minutes; the panel header shows "12s ago", amber when stale.
- [ ] A card that turns orange moves to the top of Dev below pinned cards, and back when cleared (Enter in its terminal,
  or the agent running any `crewboard` command).
- [ ] `05/06 · Panel — SHOP-22 Needs you · Terminal · Light` (and Dark): Answer in terminal opens the Terminal tab
  focused.
- Known difference: the frame's stale card reads "Quiet 4 min"; the threshold is more than 5 min (ATT-05), so the pill
  starts at "Quiet 5 min".

**T-0076 · Live agent activity**
- [ ] `11 · Banners, toasts & OS notifications → Success toasts`: "Ticket created by agent · SHOP-29 Add cart analytics
  → Backlog" with the sparkles icon (`CREWBOARD_TICKET=SHOP-1 crewboard create …` from a terminal, or an agent);
  `create -f` from an agent gives one "N tickets created by agent" toast. The new card shows "created by agent".
- [ ] `08 · Ticket editor → Editing while the agent works`: edit a Working ticket with a live session in the panel's
  Ticket tab: the footer says "SHOP-19’s agent will be told what changed."; after Save, the banner "Saved. SHOP-19’s
  agent will be told what changed: Added to 'Done when': '…'." with Open terminal; the card shows "1 message waiting"
  until the note is typed (once the agent is idle and the input line is empty).
- [ ] `06 · Terminal tab → Ticket change delivered (SHOP-19)`: the note arrives in the terminal as one message.
- [ ] An agent's edit (`crewboard update --add-done-when …` from the agent) appears in the open panel's Ticket tab with
  "Updated by agent" without reopening it.
- Known differences: the change parts quote with straight quotes ('…', the note's own wording) where the frame uses
  curly ones; toasts don't open their ticket on click (none does yet).

Earlier milestones' hand checks stay as listed in review-M1…M3.

## 4. Implementation choices (SDD §26)

- **Caller identity:** the service uses the socket's peer PID (LOCAL_PEEREPID) and walks its ancestry with
  `proc_pidinfo` (macOS) or `/proc` (Linux); a request's claimed `pid` must equal it. Sessions that run claude for a
  ticket (agent; answer and delivery from M10) make their descendants that agent; Shell, Project terminal and Run don't.
  A process that leaves the session's process tree on purpose (setsid + double fork) counts as the user.
- **Actor:** an agent is always `agent:<KEY>`; a user's command uses `--actor`, else `agent:$CREWBOARD_TICKET` when the
  variable is set (d0032; so the plugin works by hand and edits show as the agent's), else the OS user.
- **Policy:** one `policy::authorize`; every request is mapped to Read / ChangeTicket / CreateTicket / Dependency /
  UserOnly by an exhaustive match. Agents may read across projects; typing into sessions, pins, moves, settings,
  project changes, deletes and stopping the service are user-only; `dep remove` follows the `dep add` rule.
- **Hooks:** `crewboard hook` exits 0 silently in every case (unset variable, unknown event, unreachable service, 3 s
  limit); the plugin's commands start with `command -v crewboard … || exit 0`. Hooks from outside an agent session
  (the plugin used by hand) change nothing. Hook status changes aren't written to History. Notification types are
  filtered by the service (no matcher). Background tasks count while their status is running/pending; a task with
  `agent_type` (or a type containing "agent") is a helper agent; a subagent's prompt (payload with `agent_id` /
  `agent_type`) is stored as "helper agent asks: …". The wakeup time is computed from the cron `schedule` in local time
  (Claude Code reports no time).
- **Signals:** a pure state machine (`signals::state`), stored on the ticket (migration 0003: `signal_text`,
  `signal_since`, `wakeup_at`, `last_signal_at`); turn facts and "idle between turns" are kept per session in the
  service's memory. Idle between turns = the last hook ended the turn; typing deliveries marks the session busy until
  its next Stop. `STALE_AFTER_MS` = 5 min (strictly more than); Working in background and orange are never stale.
- **CLI:** JSON shapes are documented at the top of `read.rs`, `write.rs`, `deps.rs`, `agent.rs`; `-d` and `--stdin`
  are exclusive; `ask <KEY>` answers "not available yet" until M10; readiness is "every parent Merged" until T-0077.
- **Batch create:** `ticket.create_batch`, all or nothing by deleting the batch's tickets again on any error (numbers
  stay used); the `-f` format is the tickets file's own Markdown (an ID is a word with a digit before " · ").
- **review submit:** uncommitted = `git status --porcelain --untracked-files=all` (tracked changes and untracked files);
  the summary goes into a `review_submitted` event (prime reads the latest one); refused for tickets not at work.
- **Change note:** one note per save, queued in order; dependency changes included; pins and External ID send nothing.
- **Answer options:** labels at most 40 characters; migration 0004 `human_questions`; the open question is on
  `Ticket.question` while Asks you.
- **UI:** board and panel redraw every second while a Dev ticket has a last signal; agent-created toasts batch on 800 ms
  of quiet; the panel's read mode rebuilds from the workspace's copy of the ticket.
- **Moved code:** the Stack list rows and chain (`board::stack`) moved from the app into core, shared with `dep tree`.
- **Test harness:** `crewboard_core::service::testing` (`TestService`, `AgentShell`, `shop_sample`) runs a service in a
  temporary folder with an `sh` as a ticket's agent session; used by core and CLI tests. No test starts claude.
- **New crate:** libc 0.2.190 in crewboard-core (proc_pidinfo).

## 5. Open questions

Both deferred and built so they are easy to change (`tickets/questions.md`):

- **Q17 · May an agent's new ticket depend on an existing ticket?** d0031 read literally refuses
  `create "Follow-up" --deps SHOP-19` from SHOP-19's agent (the link isn't from its own ticket and SHOP-19 wasn't
  created by it). Built as option 1 (literal); recommendation option 2 (allow links whose dependent the agent created).
- **Q18 · Do Crewboard's own env-file copies count as uncommitted files?** In a repo that doesn't ignore `.env.local`,
  the copy blocks `review submit`. Built as option 1 (count every file, as d0050 says); recommendation option 2 (leave
  the configured env files out). The Commit skill tells the agent to ask the user instead of committing an env file.

Notes for later milestones:
1. **T-0077** replaces `board::readiness::readiness` (used by `ready` / `blocked`).
2. **T-0078 / M7** read the open question from `Ticket.question`; the inbox answer uses
   `board::questions::format_answer` and should mark the row answered (`answer`, `answered_at`). An inbox answer
   should clear orange through `Signals` (`state::Input` gets an "answered" input, or reuse `Enter`).
3. **T-0110** (focused terminal): `Sessions::is_focused` exists; the orange transition lives in
   `signals::state::transition`.
4. **T-0087** gives `address-review` the options guidance of `work-ticket`; **T-0139** sets the Commit skill that
   `prime` names (already read from the effective settings).
5. **T-0128** acts on `StopFailure` (`Signal::Failed`, logged now).
6. **M10:** `ask <KEY>`, `answer`, `questions`, `pr …`, `branch rename` need their policy rows in
   `service/permissions.rs` (the exhaustive match forces it); answer and delivery sessions already count as agents.
7. The ports test `freed_ports_are_reused` failed once under load (a port in its free range was taken by another test)
   and passed on rerun; it is M3's and unchanged.
