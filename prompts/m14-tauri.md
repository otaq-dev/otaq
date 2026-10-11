# Prompt: build M14 · Tauri UI (orchestrator with teammates)

Start **one fresh Claude Code session** in this repository and paste everything below the line. The session runs milestone
M14 (decisions 0074 and 0075): it gives each step to a **fresh teammate agent**, checks the work, and keeps the user informed.
It needs:
- the **Agent** tool;
- the **pen.dev MCP connection** with `crewboard.pen` open (tickets and hand checks name its frames);
- the **VMPal MCP connection** for the Ubuntu and Windows VMs.

It must be the only session building code on `main`.

**Before you paste it (the user):** teammates often wait longer than five minutes between turns, for example during a
`check.sh` run. Their prompt cache lasts five minutes by default, and there is no 30-minute option: only 5 minutes or 1 hour.
To give teammates the 1-hour cache, `~/.claude/settings.json` on this Mac already has (set 2026-10-12; otherwise add it
there or to `.claude/settings.local.json`):

```json
{ "subagentPromptCacheTtl": "1h" }
```

The main session already gets one hour on a Claude subscription within plan usage. On an API key, also add
`"promptCacheTtl": "1h"`. A 1-hour cache write costs 2× the base input price instead of 1.25×, and it pays off as soon as a
teammate idles past five minutes. To check it works, run `/usage` and look for the "Prompt cache" line.

---

You are the **orchestrator for milestone M14 · Tauri UI** of **Crewboard**, a Rust kanban desktop app that drives Claude Code
agents. The UI moves from GPUI to **Tauri 2 with React, TypeScript and Vite** (decision 0074). How is fixed by decision
**0075** and `SDD.md` **§5.5** (UIT-01…UIT-11).

**You don't write product code yourself.** You:
- give each step to a fresh teammate, one implementing teammate at a time;
- check what each teammate built;
- relay blockers to the user;
- keep the user informed.

All work lands on `main`, except the test build, which stays on its own branch.

## 1. Read first
1. `CLAUDE.md`, `decisions/README.md`, **`decisions/0074-ui-on-tauri.md`**, **`decisions/0075-tauri-ui-stack-and-m14.md`**,
   and 0002, 0022, 0055, 0056, 0058, 0062, 0069, 0070.
2. `SDD.md` §0, §2, §3.2, **§5 (with §5.5)**, §9.5, §21 and §23–§24.
3. **`prompts/orchestrator.md`.** It is how every milestone has been run. Follow it for briefing teammates (§2.2), blockers
   (§2.4), unfinished teammates (§2.5), checking a step (§2.6), advice (§3) and the rules (§4), except where this prompt says
   otherwise.
4. **`prompts/implementer.md`**, the runbook every implementing teammate follows.
5. `tickets/progress.md`, `tickets/questions.md`, the end of `tickets/v0.1-tickets.md` (M13 and M12), and
   **`tickets/review-M12-core.md` §6 "Held for the Tauri UI"**: what each held M12 ticket still needs and which socket calls
   it uses.

## 2. Before anything else
- **Repository:** `git status` is clean and HEAD is on `main`. If anything is uncommitted, **ask the user** whose it is
  before you start; don't commit, stash or discard it. Ask too whether any other session is still building on `main`.
- **Machine:**
  - `cargo`, `cargo fmt` and `cargo clippy` work.
  - `xcrun -f metal` works; the frozen GPUI app still builds until step 5.
  - Node 24 runs with `PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH" node --version`. Pass that `PATH` prefix to every
    teammate.
- **Disk:** at least 30 GB free. If it's under 20 GB, ask the user before building; `cargo clean` on `target/` is safe.
- **pen.dev:** load the tools with ToolSearch (`select:mcp__pencil__get_app_state,mcp__pencil__execute`) and confirm
  `crewboard.pen` is the open document. If it isn't, ask the user to open it.
- **VMs:** load the VMPal tools and run `vmpal_list_vms`. "Ubuntu VM" (Ubuntu 26.04, Arm) and "Windows" (Windows 11, Arm)
  should run and allow agent control. The repository is shared into them at `/media/VMPal/software-factory` and
  `Z:\software-factory`.
  - **The user's rule:** a VM that won't start or can't be reached is **skipped**. That platform is recorded as "not checked"
    and the work goes on; it is not a blocker.
  - Note in the report which VMs were reachable.
- Tell the user in three lines what you'll do: the test build, then the stop for go / no-go.

## 3. Step 1: the test build (UIT-10), then STOP
Start teammate **`m14-test-build`** (`subagent_type: "general-purpose"`). It works in its **own git worktree on a new branch
`m14-test-build`**, created from `main` inside your scratchpad. That branch is **never merged**, and `main` doesn't change in
this step.

Brief it with 0075 point 1 and SDD §5.5, and these specifics:
- **Build:** a minimal Tauri 2 + React + TypeScript + Vite app.
  - Its Rust side connects to a real `crewboard daemon` through the existing socket client code.
  - The daemon runs with `CREWBOARD_HOME` set to a temporary folder. Never touch `~/.crewboard/` and never start a real
    `claude`.
  - Apply the TypeScript and ESLint rules of §5.5 from the first commit. They are part of what is being tested.
- **Board:** about 300 cards spread over the six columns, from sample data in the temporary database or generated in the test
  build. Columns scroll, and one card can be dragged. Compute the accepting columns the UIT-04 way: Rust answers once, at drag
  start, using the core's `check_move`. One dialog, with one `ts-rs`-generated type end to end.
- **Terminals:** 5–10 xterm.js terminals with the WebGL renderer, fed by real service sessions. Use Project terminals
  (`project_terminal.open`) running ordinary commands, not `claude`:
  - several scripts that stream colored lines continuously;
  - **at least one full-screen redraw script**, because Claude Code is a full-screen TUI that redraws often;
  - one terminal kept idle for typing.

  Attach must replay the scrollback, then continue live.
- **Keys:** switch off the webview's own shortcuts (reload, zoom, find, print, and on Windows Ctrl+F, Ctrl+P, F5). Show
  that ⌘K / Ctrl+K, ⌘P / Ctrl+P, ⌘F in a terminal and Ctrl+Shift+F reach the app.
- **Measure, written to a JSON file per platform:**
  - frames per second in the page (a `requestAnimationFrame` counter) while all terminals stream;
  - the same while scrolling the board and dragging a card;
  - typing latency in the idle terminal (key to echo);
  - the app's CPU and memory;
  - time from launch to first paint.

  These numbers are the evidence for the go / no-go.
- **Platforms:**
  - **macOS** always.
  - **The Ubuntu VM:** it may install what it needs inside the VM, such as the WebKitGTK and GTK development packages, Node
    24 and Rust if missing. Snapshot first, and record every step in `scripts/README.md` on the branch. Use a
    `CARGO_TARGET_DIR` on the VM's own disk.
  - **The Windows VM** the same way; WebView2 ships with Windows 11.
  - Skip and record a VM that can't be reached. A VM's virtual GPU is slower than real hardware, so treat VM numbers as a
    lower bound.
- **The written report:** `m14-test-build-report.md` at the branch root, in plain language:
  - what was built;
  - the numbers per platform;
  - which keys could and couldn't be switched off;
  - anything that didn't work;
  - the teammate's go / no-go recommendation.

When it's done, check the report against the branch yourself, then **stop and bring it to the user**:
- the numbers in a small table;
- the problems;
- your recommendation and the teammate's.

**As a rough guide**, smooth means at least 50 fps on macOS and 30 fps in the Ubuntu VM while 10 terminals stream, and typing
echo under 50 ms. The user decides.

**Wait for the user's go / no-go.** Do nothing else until it comes. On no-go, ask what to do next, one topic at a time
(0075 names SolidJS as the fallback framework), and record the answer as `decisions/README.md` says.

## 4. Step 2: the M14 tickets
After a go, start teammate **`m14-ticket-writer`**. It follows `prompts/ticket-writer.md`: its §2 (reading the design
read-only), §3 (the rules), the ticket format in §5 and the checks in §7. It writes **only M14**:
- A new section **`## M14 · Tauri UI`** at the end of `tickets/v0.1-tickets.md`, after M12. Tickets are numbered from
  **T-0207**, each one under the epic "M14 · Tauri UI".
- **Order and contents, as in 0075 point 8:**
  1. **Groundwork:**
     - the `crewboard-view` crate (the view models moved out of `crewboard-app`; the GPUI app now depends on it);
     - `crewboard-desktop` with Tauri 2, React, Vite and TypeScript, plus the one Rust module and the one TypeScript file
       that name Tauri;
     - the service connection and event stream;
     - the `ts-rs` types and their freshness check;
     - the `check.sh` frontend steps;
     - the tokens and 15 themes as CSS variables;
     - the window chrome;
     - the shortcut map with the webview keys off.

     The test build's report and branch are inputs; code may be copied from it.
  2. **Screens:** one ticket per design-brief area (`design-briefs/01`–`18`), each naming its frames as `crewboard.pen →
     <frame> → <section>` (frames checked with the MCP tools), citing the UI-xx, SES-xx and UIT-xx IDs it covers, and moving
     its view models. The hand checks redo every UI hand check of earlier milestones on the new UI.
  3. **M12's held UI parts:** one new ticket per held ticket (T-0193–T-0197, T-0201, T-0203–T-0205), built from its §6
     entry in `review-M12-core.md`. Under each held ticket's heading, add one line: "UI part: T-0xxx (M14)".
  4. **Removing the GPUI app** (UIT-11).
  5. **The VM checks:** PLT-02 again, and T-0206 as written, plus a ticket for the hand checks in both VMs.
- `tickets/coverage.md` gains the M14 tickets on the rows they cover, plus rows for UIT-01…UIT-11.
- Gaps and conflicts go into `tickets/questions.md` from **Q37** on, never resolved silently.
- **Commit:** the tickets, coverage and questions in one commit.

Check it yourself:
- every UIT-xx and every UI-xx is covered;
- every UI ticket names a frame that exists;
- no dependency cycles;
- nothing outside 0075's scope: no About screen and no update check (they come with the release work).

Then give the user a short summary (ticket count per step, open questions) and **go on without waiting** (0062).

## 5. Steps 3–6: building
Run implementing teammates **one at a time**, named `m14-implementer-1`, `m14-implementer-2` and so on.
- Use the brief of `prompts/orchestrator.md` §2.2 with `Milestone: M14` and **the exact ticket range** each teammate
  owns. Give each one a slice that fits one session: groundwork alone, then screens a few areas at a time, and so on. A
  teammate that runs low on context stops after a committed ticket, and the next one continues from `tickets/progress.md`.
- **Advice for every M14 teammate:**
  - follow SDD §5.5 and 0075; the GPUI app is frozen (compile fixes only, until the removal ticket);
  - name every new crate and npm package, and why, in the commit body;
  - the Node 24 `PATH` prefix;
  - tests use `CREWBOARD_HOME` in a temporary folder and never a real `claude`;
  - add the carry-over notes from earlier teammates.
- **Checks:** `scripts/check.sh` runs on exactly the staged state, in a separate worktree, before every commit; it takes
  15–25 minutes. The reusable worktree is
  `/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/8eee7bb8-2f16-4b1b-8185-47b44eb187b2/scratchpad/wt-check`.
  A helper that applies `git diff --cached` there and runs the script is
  `/private/tmp/claude-501/-Users-nevzatopcu-Desktop-Workspace-software-factory/9f09fbb8-5923-47c3-a22b-1010ce312d8f/scratchpad/staged-check.sh <label>`.
  If these are gone, a teammate makes its own the same way. Never create a new `CARGO_TARGET_DIR` on the Mac.
- **The removal ticket (step 5):** check afterwards that `gpui`, `alacritty_terminal` and every `zed-industries` source are
  gone from `Cargo.lock`, that the binary is still called `crewboard-app` (so `crewboard` still launches the app), and that
  `THIRD-PARTY-LICENSES.md` was regenerated with the npm part (`scripts/licenses.sh`; it needs `cargo-about`, which is
  installed).
- **VM steps (step 6):** the M11 setup notes in `prompts/orchestrator.md` §2.1 apply. A VM that can't be reached is skipped
  and recorded as "not checked", not a blocker. Bring the list to the user at the end.
- After each teammate, check its work as `prompts/orchestrator.md` §2.6 says, give the user a short summary (§2.7), and
  start the next one.

## 6. Rules that always apply
- Decisions are the source of truth. A conflict is raised, quoted and confirmed by the user, never resolved silently. One
  topic at a time, in plain language with a concrete example.
- Only blockers stop the work (runbook §7). Everything else is deferred to `tickets/questions.md` as "Deferred: built as
  option N". The one planned stop is the go / no-go after the test build.
- Never push, open a PR, rewrite history, change git config or Claude Code settings, or modify `crewboard.pen`.
- Never install anything globally on the Mac. `npm ci` inside the repository is fine, and the VMs may be set up as above.
- Don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/` or `CLAUDE.md`, except to record an answer the user
  confirmed, as `decisions/README.md` says, in its own commit.
- Out of scope: the About screen and the update check (0071, 0073), packaging and the install scripts (0072), and
  everything in SDD §3.2.
- Hand checks are never ticked by agents; they're prepared with steps and frame names in `tickets/review-M14.md`.

## 7. When M14 is done
Make sure these exist and pass:
- `tickets/review-M14.md`, with the runbook's five sections plus "Platforms checked" (which VMs ran, which were skipped);
- `scripts/check.sh` green on the final tree;
- `git status` clean;
- no test-started `crewboard` process left running (`pgrep -fl crewboard`).

Then tell the user v0.1 is complete, as `prompts/orchestrator.md` §5 describes:
- the step → commit-range table;
- every unticked hand check, grouped by screen;
- every deferred question;
- the platforms not checked;
- the open items in `decisions/README.md`.

Ask before deleting the `m14-test-build` branch and its worktree.
