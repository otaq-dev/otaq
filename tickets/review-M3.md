# M3 · Sessions — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0046 | Port pool: two ports per ticket | 99cfcc8 |
| T-0047 | Keep the base current: fetch, fast-forward, diverged hold | b68881d |
| T-0048 | Worktrees, temporary branches, base_sha, epic integration branch | ef40af6 |
| T-0049 | Env files and setup command | 823ccb8 |
| T-0050 | PTY session manager with stored scrollback | 05f2915 (+ 254562b, a flaky test fixed) |
| T-0051 | Keystroke relay and unsent-input flag | d603ed4 |
| T-0052 | Agent launch command and Claude session ID | bd1d043 |
| T-0053 | Start sequence, Start interrupted, Retry start (+ temporary start trigger until T-0079) | 66c4b49 |
| T-0054 | Automatic deliveries and user-click deliveries | 66ae8cf |
| T-0055 | `crewboard attach <KEY>` | 8b3f0cf |
| T-0056 | Terminal view: grid rendering, input, reconnect redraw | 739913b |
| T-0057 | Terminal: selection, scrollback, find, jump to latest, font size | 425ee01 |
| T-0058 | Terminal focus and key routing | 160434e |
| T-0059 | Side panel: frame, header, tabs, navigation | 5bf50ad |
| T-0060 | Panel action bar per state | 9a2edad (+ 1826311, a missed tick) |
| T-0061 | Terminal header strip, message waiting, unsent-text prompt | 4a34e0a |
| T-0062 | Panel content: Ticket tab, Stack section, History tab | d7d9b27 |
| T-0063 | Start interrupted on card and panel (Retry start) | b0f1b5b |
| — | Agent counts on the rail follow session starts and ends (M2 carry-over) | 168f5c8 |

## 2. How to try it

**Warning: from M3 on, moving a ticket to Dev starts a real `claude` session** (the temporary trigger, T-0053) when the
service was started by `crewboard daemon start` or by the app. Use a throw-away data folder (short path; `mktemp -d`
does), and use the "no claude" variant below when you only want to see the start sequence.

```sh
scripts/check.sh
cargo build --workspace                      # build everything: the app starts the crewboard binary next to it
export CREWBOARD_HOME=$(mktemp -d)
```

**Without claude (start sequence, Start interrupted, panel, history).** Start the service with a PATH that has git but
no `claude`, then use the app:

```sh
PATH=/usr/bin:/bin target/debug/crewboard daemon start
target/debug/crewboard-app
```

Seed projects and tickets with M2's `seed.py` (tickets/review-M2.md §2), or add a repo in the app. Moving a ticket to
Dev then goes Queued → (worktree, branch, ports) → **Start interrupted** at the launch step ("can't start claude …"),
which is the state of *16 → Start interrupted · SHOP-28*. For the setup-failure variant set the project's setup command
to `echo installing; exit 3` in Settings first: Start interrupted at "setup command failed (exit 3): installing", and
the panel's "View the setup log" shows `$CREWBOARD_HOME/logs/setup/SHOP-n.log`. For the diverged hold, use a repo
cloned from a local bare repository with one commit pushed from elsewhere and one committed locally (as in
`crates/crewboard-core/tests/base_sync.rs`): the card is **Held** with "`main` and `origin/main` have diverged. Push or
pull, then retry" and offers Retry.

**With the real claude (terminal, attach, focus, deliveries).** Stop the service, start it normally, and move one
ticket to Dev:

```sh
target/debug/crewboard daemon stop
target/debug/crewboard daemon start          # your PATH: claude is found; the plugin is the repo's plugin/
target/debug/crewboard-app
```

The agent runs `claude --plugin-dir <repo>/plugin --allowedTools "Bash(crewboard:*)" --session-id <uuid>
/crewboard:work-ticket` in `$CREWBOARD_HOME/wt/<repo>/<KEY>` with `CREWBOARD_TICKET`, `PORT` and the crewboard bin
folder first on PATH. The work-ticket skill itself is written in T-0073 (M4), so in M3 claude answers that the command
is unknown; type to it like any session. `target/debug/crewboard attach SHOP-1` (Ctrl+] detaches) shows the same
session from any terminal.

**A colour chart** (for T-0056's colours and attributes): in a live session's terminal, run it through claude's bash
mode (type `!` followed by the command) or ask the agent to run it:

```sh
for i in $(seq 0 15); do printf "\e[48;5;${i}m %3d \e[0m" $i; done; echo
for i in $(seq 16 231); do printf "\e[48;5;${i}m  \e[0m"; done; echo
for r in 0 64 128 192 255; do printf "\e[48;2;${r};100;200m  \e[0m"; done; echo
printf "\e[1mbold\e[0m \e[3mitalic\e[0m \e[4munderline\e[0m \e[9mstrike\e[0m \e[7minverse\e[0m \e[2mdim\e[0m 界面\n"
```

**A waiting message** (T-0061; nothing queues deliveries before M4): with a live session for SHOP-1,

```sh
sqlite3 $CREWBOARD_HOME/crewboard.db "INSERT INTO pending_deliveries (id, session_id, kind, text, created_at)
  SELECT 'hand-1', id, 'ticket_change', 'The ticket was updated by the user.', 0 FROM sessions
  WHERE ended_at IS NULL LIMIT 1"
```

then switch projects (or restart the app) so the board refetches: the card and the strip show "1 message waiting ·
Deliver now". Type something in the terminal without Enter and click Deliver now for the unsent-text prompt.

**Cleaning up:** `target/debug/crewboard daemon stop` (sessions end; the next start records them as ended), then
remove `$CREWBOARD_HOME`. Worktrees live inside it; the repos only got branches `crewboard/<KEY>` and git's worktree
metadata (`git worktree prune` after deleting the folder).

## 3. Hand checks

Every frame is in `crewboard.pen`. "Light/dark": compare once in each (macOS appearance or Settings → Appearance).

**T-0051 · Line clear**
- [ ] With the real claude: type a line (also a two-line input with Shift+Enter), then trigger "Clear it and send" (the
  waiting-message recipe above). Crewboard sends Ctrl+E then Ctrl+U before typing; check the typed text is gone and the
  message arrives alone. If multi-line input survives, say so (the sequence is one constant, `session::CLEAR_LINE`).
- [ ] Deliveries: the message is typed and submitted (80 ms between text and Enter); a multi-line message arrives as one
  message (bracketed paste).

**T-0052 · Agent launch**
- [ ] Once with the real claude: the session starts in the ticket's worktree, the Crewboard plugin is loaded (`/plugin`
  in claude lists it), `/crewboard:work-ticket` was sent (unknown until T-0073), `echo $CREWBOARD_TICKET $PORT` in a
  claude `!` command prints the key and the agent port, and the History tab's Claude session ID resumes the
  conversation with `claude --resume <id>` from the worktree.

**T-0056 · Terminal view**
- [ ] `06 · Terminal tab → Live · streaming, app has focus` and `Dark · live terminal`: the grid, colours, cursor, the
  dimmed muted hint under it; output streams smoothly (60 fps) and typing never drops keys.
- [ ] `06 → Reconnecting to the background service`: with a session open, `crewboard daemon stop` is too strong (it
  ends sessions); instead pause the service (`kill -STOP <pid>` from `crewboard daemon status`) until the banner says
  reconnecting, then `kill -CONT <pid>`: the scrollback dims under "Reconnecting… agents keep running", input is off,
  "Input paused while reconnecting · output will catch up"; afterwards the terminal is redrawn without doubled output.
- [ ] `06 → Session ended normally`: end the claude session (/exit): the end marker "Session ended · HH:MM · N min" with
  the end reason, "Read-only · the session ended", no cursor, no input. Known difference: the marker sits at the
  bottom of the grid, not right after the last line; tokens come with M8.
- [ ] `06 → Narrow panel · 480 px`: drag the panel to its minimum: the grid reflows (the PTY follows), tabs are
  icon-only with tooltips, the strip drops path and port. Expanded (⇧⌘E) works too.
- [ ] The colour chart above: 16 colours (light and dark palettes), the 256-colour cube and greys, truecolor, bold /
  italic / underline / strikethrough / inverse / dim, wide characters.

**T-0057 · Selection, scrollback, find**
- [ ] `06 → Scrolled up · find in scrollback`: ⌘F opens the bar; matches bold amber, the current one orange; "3 of 12";
  ↵ older, ⇧↵ newer; Esc closes find (and only then reaches claude). While scrolled up, new output doesn't move the
  view; the "N new lines" pill jumps back; "Scrolled up 1,320 lines" and "… line X of Y" show.
- [ ] Drag, double-click and triple-click select; ⌘C copies; ⌘V pastes; trackpad scrolling is smooth; ⌘+ / ⌘− and the
  strip's A− / A+ change the size (default from Settings → Terminal font size).

**T-0058 · Focus and keys**
- [ ] `06 → Answering a question (SHOP-23) · focused`: clicking the grid shows the accent border and "Typing goes to the
  agent · ⇧⌘` leave terminal". Esc, Esc Esc, Ctrl+C, Ctrl+J, arrows, Tab reach claude; Esc doesn't close the panel
  while focused. ⌘K, ⌘P, ⌘N, ⌘, ⌘[ / ⌘], ⌘⌥1–6, ⇧⌘E, ⇧⌘D still work. ⇧⌘`, ⌘J and a click outside leave focus (check
  ⇧⌘` isn't taken by macOS's window cycling).
- [ ] `01 · Components → Keyboard map` against the behaviour above.

**T-0059 · Side panel**
- [ ] `05 · Ticket side panel → SHOP-18 · Ready for Review` and `05/06 · Panel — SHOP-22 Needs you · Terminal · Light /
  Dark`: a card click opens the panel (↵ on the selected card too); Esc and × close it; the header (key, title, pill,
  branch chip with copy, "on main", last signal and tokens when they exist); tabs with indicators.
- [ ] `05 → Long title & long branch`: three lines then cut (full title on hover); a long branch is cut in the middle.
- [ ] `05 → Expanded · ⇧⌘E`; resizing from the left edge (480 px minimum, the width is kept while the app runs);
  ⌘[ / ⌘] walk the column; ⌘⌥1–6 switch tabs.

**T-0060 · Action bar**
- [ ] `05 → Action bar · every state` for the states M3 can reach (Backlog: Start + Edit; Working: Open terminal;
  Start interrupted: Retry start; Queued with a parent: Edit + View parent). The others are hidden until their
  milestones. The "…" menu (`05 → Delete · never-started ticket`, `Close ticket · panel "…" menu` for the items that
  exist): Delete… enabled with "Never started · nothing on disk" or disabled with "Started tickets can only be closed."

**T-0061 · Strip, message waiting, prompt**
- [ ] `06 → Message waiting · unsent text (SHOP-19)` and the strip in the other 06 frames ("Live", "Waiting for you",
  path, "agent :4019", attach chip copying `crewboard attach SHOP-19`).
- [ ] `06 → Row · update F → Unsent-text prompt (SHOP-23)`: title, the typed line, what would be sent, Keep it and cancel
  (nothing changes), Clear it and send.
- [ ] "Instructions: shop-web" opens Settings at the project's Agent instructions.

**T-0062 · Ticket tab, Stack, History**
- [ ] `05 → SHOP-20 · Queued, waits for merge`: the Stack section open with SHOP-17…21; a line opens that ticket;
  collapsed summary; "+ Add dependency" opens the Ticket tab's form.
- [ ] `16 · Recovery & cleanup → History · Claude session IDs`: EVENT / WHEN / DURATION / TOKENS, the Claude session ID
  copies. Known difference: newest first (the ticket's order; the frame lists oldest first).
- [ ] Ticket tab: read mode, Edit switches to the form in place, Esc returns to read mode; a double click on a card opens
  this tab.

**T-0063 · Start interrupted**
- [ ] `16 → Start interrupted · SHOP-28 card` and `· SHOP-28 panel`; `04 · Ticket card variants · Light → Queued · Start
  interrupted`: the failed step on the card ("Setup command failed · agent never started"), the panel's callout with
  the message, "Retry start does", "View the setup log"; Retry start can't be pressed twice; nothing retries by
  itself.

## 4. Implementation choices (SDD §26)

- **Ports:** 4000–4999 for every project, lowest free first, two per ticket (agent, Run); a port counts as used when it
  can't be bound on both 127.0.0.1 and 0.0.0.0.
- **Base sync:** the branch's upstream, else `origin` (or the only remote); "dirty" = changes to tracked files; a dirty
  checkout that is behind doesn't hold the start (recorded as a note); `OnlyOnRemote` starts from the remote branch.
- **Worktrees:** `<data>/wt/<repo folder name>/<KEY>`, branches `crewboard/<KEY>` made with `--no-track`; a foreign
  non-empty folder at the path is reported, never removed.
- **Setup:** `sh -c` in the worktree with the service's environment; log `logs/setup/<KEY>.log` (replaced each start);
  the last 10 lines in the failure message; no timeout.
- **Sessions:** scrollback 2 MiB per session in the service's memory, trimmed to a line start; output as base64 in
  `session_output` events sent only to attached connections; 1024 live chunks before a slow client is disconnected;
  TERM=xterm-256color, COLORTERM=truecolor; stop = SIGHUP to the process group, SIGKILL after 2 s; rows left open by a
  crashed service are closed at the next start ("service stopped") until M7's reconcile.
- **Unsent-input flag:** errs on "there is text": cursor moves make backspace untrusted, history keys set the flag,
  unknown deletions keep it; `CLEAR_LINE` = Ctrl+E Ctrl+U.
- **Claude session ID:** chosen by Crewboard (UUIDv7) and passed with `--session-id` (listed by `claude --help`, 2.1.296).
- **Plugin folder:** `<bin>/plugin`, `<bin>/../share/crewboard/plugin`, or the repository's `plugin/` (dev builds).
- **Tests never launch claude:** `ServiceConfig::launch` is `None` by default (the launch step then fails);
  only `crewboard daemon run` uses `ServiceConfig::for_daemon`.
- **Temporary start trigger:** `Starter::start_on_enter_queued`, called once in the `ticket.move` handler; T-0079
  replaces both.
- **Deliveries:** `NeverIdle` until M4's hooks, so automatic deliveries wait for Deliver now; multi-line text as
  bracketed paste; 80 ms before Enter.
- **attach:** detach key Ctrl+]; never starts the service.
- **Terminal view:** `alacritty_terminal` from zed-industries/alacritty at 4c129667ce56 (the revision the pinned Zed tag
  uses; one `vte` 0.15.0); 16-colour palette derived from the theme's status colours; line height 1.45; 10 000 history
  lines in the view; keys and resizes go through one ordered queue; terminal answers (cursor position…) are sent as
  `reply` input that doesn't touch the unsent flag, and never for replayed output.
- **Key routing:** a GPUI keystroke interceptor in front of all bindings, driven by one table (`keymap::route`).
- **Panel:** 660 px default, 480 px minimum, the board keeps 420 px; icon-only tabs below 560 px; existing tickets are
  edited only in the Ticket tab (the modal editor is for new tickets); History newest first.
- **Schema:** migration `0002_start_problems.sql` (`tickets.start_problem_step`, `start_problem_message`).
- **New protocol:** `session.list / attach / detach / resize / input / deliver_now / focus`, `ticket.retry_start`,
  `ticket.history`; events `session_output`, `session_state`, `session_enter`, `session_focus`, `messages_waiting`;
  refusals `already_running`, `session_ended`.
- **New crates:** portable-pty 0.9.0 (core; and a dev-dependency of the CLI), base64 0.22.1, rustix `termios` (CLI),
  alacritty_terminal (app), libc, serde_json (app). Icons added from lucide-static 1.54.0 (stroke 1.5): arrow-up,
  arrow-down, chevron-up, mouse-pointer-click, a-large-small, a-arrow-down, a-arrow-up, maximize-2, minimize-2,
  keyboard, send.

## 5. Open questions and notes

**Q16 added** to `tickets/questions.md`: the design's ⌘T (focus the terminal), ⌘↓ (jump to latest) and ⇧⌘C (copy branch
name) aren't in d0058's keyboard map, which says it is the full map. M3 built without them (recommendation: keep d0058,
fix the design). It also notes that d0058's ⇧⌘` is macOS's "previous window" key.

Notes:
1. **Visuals were not checked by the agent.** Screen capture isn't available to it; every frame comparison above is open.
   The app was smoke-run against a real service (no claude on PATH): starts reached Start interrupted with the right
   step and message, the setup log was written, History listed the events, and the app ran without errors.
2. **work-ticket arrives in M4** (T-0073): an M3 agent session starts with an unknown command.
3. **A service restart still just ends sessions** (Q11.6). Tickets stay in Working with an ended session until M7's
   reconcile marks them Interrupted; the terminal shows the ended state.
4. **Agent edits are not refused for agents yet** (T-0065), and delete stays refused for agent callers.
5. **Frame wording followed the ticket where they differ:** Start interrupted shows the failed step and its message (the
   frame says there is no step-by-step status); History is newest first.
6. **For M4:** `Sessions::input` publishes `session_enter` (ATT-02 clears orange with it); the `IdleSignal` passed to
   `Deliveries` should come from the hook mapping (T-0066/T-0067); `session_focus` / `Sessions::is_focused` is what
   the focused-terminal rule (T-0110) needs; the ticket-change note (T-0074) queues with
   `Deliveries::queue_delivery(.., DeliveryKind::TicketChange, ..)`.
