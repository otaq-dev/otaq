# M9 · Run, Shell, Project terminal — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0133 | Run process manager (core) | a933fa9 |
| T-0134 | Run tab, card chip, Open URL | ee7c68a |
| T-0135 | Run-command dialog with Ask Claude | dab9e67 |
| T-0136 | Running processes list in the header | 450045b |
| T-0137 | Shell tab | 7a58b2c |
| T-0138 | Project terminal | e51c5da |
| T-0167 | Project terminal: summary of created tickets, with delete | 2b9af80 |

One commit per ticket. `tickets/progress.md` rows of a ticket are committed with the next one (as in M8).

## 2. How to try it

```sh
scripts/check.sh
cargo build --workspace
export CREWBOARD_HOME=/tmp/cbm9-$$                 # short: the socket lives in it
mkdir -p $CREWBOARD_HOME/repo && (cd $CREWBOARD_HOME/repo && git init -q -b main && echo a > a.txt && git add . && git commit -qm init)
B=$PWD/target/debug/crewboard
PATH=/usr/bin:/bin $B daemon start                 # no claude on PATH: starts stop at the launch step
echo '{"protocol_version":1,"id":1,"request":{"method":"project.add","params":{"path":"'$CREWBOARD_HOME/repo'"}}}' | nc -U -w 1 $CREWBOARD_HOME/crewboard.sock
for t in "Price table" "Coupon codes"; do $B -P REPO create "$t" --done-when works; done
echo '{"protocol_version":1,"id":2,"request":{"method":"ticket.move","params":{"key":"REPO-1","target":"dev_queued"}}}' | nc -U -w 3 $CREWBOARD_HOME/crewboard.sock
PATH=/usr/bin:/bin target/debug/crewboard-app
```

REPO-1 goes to Start interrupted (no claude), but it has its worktree and its two ports, which is all Run and Shell
need. Turn Settings → Notifications off first if OS notifications are unwanted while trying.

**Run (T-0133, T-0134, T-0135).** Open REPO-1 → Run tab: "Not running", Run. Clicking Run asks "What command runs
repo?" (no command yet). Type a command that listens on the port, for example `echo serving on $PORT; exec nc -lk
$PORT`, and `http://localhost:$PORT` as Open URL; Save & start. The tab shows Starting… then Running once the port
answers; the card gets `▶ :4001` (click it for Open / Stop), the panel header a green "● :4001" pill, the header
"▶ 1 process" (its menu lists the Run). Stop → "Stopped by you at HH:MM". Other states:
- failed to start: Settings → project → Run command `echo 'Missing script: dev' >&2; exit 1`, then Run;
- port in use: `nc -lk 4001 &` in a terminal of your own (the Run port shown in the tab), then Run; kill it after;
- reaching Done / Close: Close REPO-1 from the panel's "…" menu while it runs → toast "Run stopped · REPO-1 is
  closed";
- stopped by restart: `$B daemon stop; PATH=/usr/bin:/bin $B daemon start` while it runs.

**Ask Claude (T-0135)** runs the real `claude` (tokens): start the service with your normal PATH instead
(`$B daemon start`), open the dialog on a project without a run command and click Ask Claude. It runs
`claude -p --output-format stream-json --verbose --tools Read,Glob,Grep "<prompt>"` in the project's folder; Cancel
kills it. Without claude it is refused ("Claude Code isn't available to the background service").

**Shell (T-0137).** REPO-1 → Shell tab: your login shell in `$CREWBOARD_HOME/wt/repo/REPO-1` with `$PORT` = the Run
port; `echo $PORT $CREWBOARD_TICKET`, `crewboard show`. Closing the window keeps it (reopen the app: same scrollback).
Close the ticket → "Shell closed"; after a service restart → "Stopped by restart" with Reopen.

**Project terminal (T-0138, T-0167).** ⇧⌘T (or the header's terminal icon): a drawer with your shell in the repo
folder and the hint. `crewboard create "Checkout API"`, `crewboard create "Price table" --deps REPO-3`: the tickets
appear in Backlog with a short violet outline, and the summary column ("2 tickets, 1 dependency · all in Backlog")
lists them with trash buttons; trash asks the T-0162 confirmation. `crewboard dep add REPO-3 REPO-4` prints the loop
error. `exit` → "Ended" with Reopen. Running `claude` in it works as in any terminal (Q29).

**Cleaning up:** `$B daemon stop` (Run, Shell and Project terminal end with it), then `rm -rf $CREWBOARD_HOME`.

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each appearance.

**T-0134 · Run tab, card chip, Open URL**
- [ ] `13 · Run & Shell → Not started`, `Starting`, `Running`, `Stopped`, `Failed to start`, `Port already in use`,
  `Stopped by restart` (the recipes above): the state card (icon, title, `:port`, command, message, buttons) and
  OUTPUT below. Known differences: the output is the terminal view (dark/light terminal colours, not the frame's
  light box); after a restart the old output isn't shown (Q28); "Stopped" says "Stopped by you at 11:48" (the frame
  "Stopped · exited 0 at 11:48" is used when the program exited on its own).
- [ ] `13 → Reaching Done stops it`: the toast "Run stopped · SHOP-19 is done" (Local merge) / "… is closed" (Close).
  Parking a ticket stops its Run without a toast ("Stopped at HH:MM · SHOP-19 was parked", Q30).
- [ ] `13 → Card & panel-header controls`: the card chip `▶ :4020` (a click opens Open :4020 / Stop — the frame
  shows it on hover) and the panel header's green pill with Open and Stop; both only while the Run is up.
- [ ] `13 → Run · backend (BILL-4) · no Open URL` and `Dark · Run (backend)`: no Open anywhere when the project has
  no Open URL; the tab marker ":4004".
- [ ] `13 → Dark · Run tab`. The log stays smooth with long output (`yes | head -100000`-style commands) and follows
  the end unless scrolled up.

**T-0135 · Run-command dialog**
- [ ] `13 → 1 · Empty`, `2 · Claude thinking`, `3 · Suggestion`, `4 · Editing`, `5 · Saved` with the real claude
  (Ask Claude spends tokens): the progress lines, the suggestion filling both fields with the why, Cancel stopping
  claude, Save & start starting the Run. Light/dark.

**T-0136 · Running processes**
- [ ] `13 → All running processes · header menu`: "▶ N processes" only while something runs, the menu's rows (project
  chip, key, title, uptime, port, Open only with an Open URL, Stop), Stop all, a row opening the Run tab. Known
  difference: uptime is shown (the ticket asks for it; the frame has none).

**T-0137 · Shell tab**
- [ ] `13 → Shell tab · BILL-4` and `Dark · Shell`: the strip "Shell · not the agent", the worktree, "Run port ·
  $PORT=4004"; neutral colours; "Typing goes to the shell, not the agent" while focused; ⇧⌘` leaves.
- [ ] The login shell behaves as in your own terminal (prompt, aliases, PATH); a focused Shell never turns the agent's
  card into "your turn".

**T-0138 · Project terminal**
- [ ] `17 · Project terminal → Empty · shell prompt` (380 px drawer, focused, hint until the first key), `Plain shell
  use`, `Cycle rejected by the tool`, `Adding a ticket to an epic (0039)` (`crewboard create --parent …`),
  `Running · creating tickets` (violet outline on new Backlog cards, a few seconds), `Ended` (Reopen),
  `Dark · Project terminal, claude running`. Known differences (Q29): the header states "claude running" /
  "claude · waiting for you", the "claude exited · 38k tokens · created 4 tickets" line, the "created by agent"
  mark and the "crewboard attach plan" chip aren't built; the header has maximize and × (hide), no separate
  minimize.
- [ ] ⇧⌘T opens it, and focuses the terminal when it is already open; closing the window and reopening the app finds
  the same shell.

**T-0167 · Summary of created tickets**
- [ ] `17 → The review moment`: the summary beside the terminal ("4 tickets, 3 dependencies · all in Backlog", rows
  with ⛓ badges and criteria counts, the footer); a row opens the ticket. Known difference: one column serves both
  the frame's "Created this session" list and the summary.
- [ ] `17 → Undo a created ticket`: trash → the T-0162 confirmation naming the removed link; after Delete the row
  goes and the counts follow; a ticket dragged to Dev meanwhile has no trash.

## 4. Implementation choices (SDD §26)

- **Run** (`terminals::run::Runs`): a PTY session of kind `run` running `/bin/sh -c '<run command>'` in the worktree
  with the ticket's env (CREWBOARD_TICKET, CREWBOARD_HOME, the crewboard folder first on PATH) and PORT = the Run
  port. Output = the session's 2 MiB scrollback, streamed like any session; a first line `$ pnpm dev --port 4020`
  and a last line `[crewboard] exited with code 1` / `stopped by you` / `stopped · SHOP-19 is done`
  (`Sessions::append_line`). Running = a TCP connection to the port succeeds (polled every 250 ms on 127.0.0.1 and
  ::1); an exit while starting with a non-zero code = failed to start; port in use = the bind check before
  anything starts (nothing is killed). States live in the service's memory; `Board::run_ports` feeds the card's
  `▶ :4020` flag. Stop = SIGHUP to the process group, SIGKILL after 2 s.
- **Finished hooks are async** now (`Review::on_ticket_finished`): the Done cleanup waits for Run and Shell to stop
  before it frees the ports and removes the worktree.
- **Park stops Run** (Q30, follow-up commit): Park frees the ports (d0052), so `Review::on_ticket_parked` hooks run
  first and stop the Run (`StopCause::Parked`, last line "[crewboard] stopped · SHOP-19 was parked", no toast).
  The Shell holds no port and stays open.
- **Ask Claude** (`terminals::suggest`): `claude -p --output-format stream-json --verbose --tools Read,Glob,Grep
  <prompt>`; prompt in `crates/crewboard-core/prompts/suggest-run-command.md`; answer = a JSON object
  {command, open_url, why}; progress and outcome as `run_suggestion` events; one per project; Cancel kills it; 3
  minutes at most. `ServiceConfig::ask_claude` is `None` by default and in tests (refused), `claude` in the daemon.
- **Shell / Project terminal** (`terminals::shell::Shells`): the user's login shell (`$SHELL -l`, else /bin/sh),
  sessions of kind `shell` / `project`; `shell.open {key, reopen}`, `project_terminal.open {project, reopen}`:
  running → last in memory → stored and stopped by restart (until Reopen) → new. `TerminalEnv` (ServiceConfig
  `terminals`) holds the shell and PATH; tests use `/bin/sh` and the built CLI's folder.
- **Caller identity**: Run, Shell and Project terminal callers are the user (full access). Creating tickets from a
  Project terminal is recorded by ancestry (`caller::project_terminal_of`) in `tickets.created_in_session`
  (migration 0017, also `sessions.summary_dismissed_at`).
- **App**: `TerminalView` takes a `TerminalTarget` (Agent, Shell, Run read-only, Project); `session.attach` accepts
  `{key, kind}` and `{project, kind}`; only the agent's terminal reports focus. Panel: `panel/run_tab.rs`,
  `panel/shell_tab.rs`; dialog `run_command.rs`; drawer `project_terminal.rs` (⇧⌘T bound, header button); header
  processes chip in `shell.rs`; view models `run`, `run_command`, `processes`, `shell`, `project_terminal`,
  `created_summary`. New tickets that arrive live get a 4 s violet outline (`Workspace::fresh`). New tab marker
  `TabIndicator::Problem` (red dot).
- **New protocol**: `run.start / run.stop / run.status {key}`, `run.list`, `run.suggest / run.suggest_cancel
  {project}`, `shell.open`, `project_terminal.open / .created / .dismiss`; replies `run`, `runs`, `session`,
  `created_here`; events `run_changed`, `run_stopped`, `run_suggestion`; refusals `no_run_command`, `no_worktree`.
  Writes are user-only; `run.status`, `run.list` and `project_terminal.created` are reads.
- **No new crates.**

## 5. Open questions

Added to `tickets/questions.md`, all deferred:
- **Q28** — does a Run's output survive a restart? d0045: no terminal output on disk; the *Stopped by restart* frame
  shows old lines. Built as option 1 (memory only).
- **Q29** — does the Project terminal notice a `claude` started in it ("claude running", "claude exited · tokens ·
  created N", "created by agent")? Built as option 1 (a plain terminal; the summary still lists what was created).
- **Q30** — does Park stop a ticket's Run? No decision says; Park frees the ports (d0052). Built as option 1 (Park
  stops Run; the Shell stays).

Notes for later milestones:
1. **M10**: answer and delivery sessions are agent-kind sessions; `Sessions::agent_roots` and the finished hooks are
   unaffected. A PR flow that finishes a ticket (Mark merged) should call `Review::finish` so Run and Shell stop.
2. **M11 Windows/Linux**: Run uses `/bin/sh -c` and a Unix process group; the Shell uses `$SHELL -l`; the port check
   tries 127.0.0.1 and ::1. Windows needs `cmd /c` / PowerShell, ConPTY and job objects for Stop; the user's shell
   from `%COMSPEC%` or settings.
3. **Park and Run** (Q30, fixed after the review): Park now stops the ticket's Run before the ports return to the
   pool, so the freed Run port is really free; the Shell stays open.
4. Smoke-run (agent): a service with no claude on PATH, a ticket at Start interrupted, Run with `nc -lk $PORT`
   (starting → running), the Shell and the Project terminal opened over the socket, Cleanup listing nothing, the app
   running for a few seconds without errors, a service restart leaving all three "stopped by restart", no process
   left behind. Visuals were not checked by the agent.
