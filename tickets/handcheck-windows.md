# Hand checks · Windows 11 ARM64 VM

Tested by hand in the Windows VM (VMPal id `2EDECAD4-…`), Windows 11 Pro 24H2 ARM64, on `main` at 7a71d8d
(robocopy mirror in `%USERPROFILE%\cb-src`). Display set to 2048 × 1152 at 100 % scale so 1280, 1440 and 1920 px
windows fit. Most checks ran on the **release** build (`cargo build --release -p crewboard-app -p crewboard-cli`); the
first hour ran on the debug build. Both show the same bugs unless a line says otherwise.

**How it was set up**
- `CREWBOARD_HOME=%USERPROFILE%\hc\h1`; sample repos `shop-web`, `billing-api`, `crewboard` and a plain folder
  `not-a-repo` in `%USERPROFILE%\hc\repos`. The shop-web tickets were seeded with the CLI to M2's numbering (SHOP-1…16
  fillers, SHOP-17…24, epic SHOP-25 with 25.1–25.3, the same dependencies, SHOP-19 edited by `agent:SHOP-19`).
- **No real `claude` was run.** The VM does have a real `claude.exe` in `%USERPROFILE%\.local\bin` and a
  `~\.claude\.credentials.json`, so the app was started with a PATH that leaves that folder out and puts a stub
  `claude.cmd` first. The stub prints `2.1.295 (Claude Code)` for `--version` and otherwise opens a plain PowerShell as
  the "agent", so tickets really start, the Terminal tab is live and `crewboard ask human` works from inside the
  session (it runs as the agent). Hooks, tokens and the status line don't exist with the stub.
- No `gh`, `az` or `pwsh` installed.
- VM changes made and undone: a second local account `cbtest2` (random in-memory password, never written anywhere;
  deleted afterwards); the keyboard list was switched to US for typing and set back to en-GB / UK. Left in place:
  the 2048 × 1152 display mode and `%USERPROFILE%\hc` (data, repos, helper scripts).

**Totals: 35 checks · 19 Pass · 13 Fail · 3 Skipped** (one part of the tool checklist, the permission-mode warning,
couldn't be triggered). The 13 failing checks contain the 15 problems listed next.

## Fails, most serious first

1. **The background service crashes on the second Windows toast** (T-0117 / T-0159). Every service process posts its
   first toast fine and dies with an access violation (`0xc0000005`) right after logging the second `notification`.
   Seen 2 of 2 times (debug and release). All agent sessions end with it. Turning notifications off avoids it.
2. **"Start service" can't recover from a crash** (T-0017). The crash leaves `service.lock` behind; the app's Start
   service waits for that file to disappear and fails every time with the toast "Couldn't restart the background
   service · can't start the background service: the old background service didn't stop in time"
   (`crates/crewboard-core/src/client/mod.rs` `restart`). `crewboard daemon start` from a terminal works. Combined
   with 1, a user who gets two notifications loses the service and can't bring it back from the app.
3. **`crewboard attach` can't be detached on Windows** (T-0156): Ctrl+] does nothing in Windows Terminal or in a
   classic console; the only way out is closing the window.
4. **Ctrl+Shift + digit / punctuation shortcuts don't work while a terminal has focus** (T-0158): Ctrl+Shift+`
   (leave terminal, the hint shown under every terminal), Ctrl+Shift+, (Settings), Ctrl+Shift+1–9 (projects) and
   Ctrl+Shift+[ / ] (previous / next ticket) do nothing. Ctrl+Shift + letters work.
5. **The app opens a console window on Windows**, debug and release (no `windows_subsystem = "windows"` in
   `crewboard-app`). The console stays open behind the app showing GPU driver lines, and closing it kills the app.
6. **Text doesn't wrap and runs past its box** in several places: Welcome's three feature lines, the "Not a git
   repository" message, Settings → Tools (the az command is cut off), the Run-command dialog's "Claude looks at the
   repo…" line, and error toasts (cut at the toast's edge).
7. **After an attach client goes away the app's terminal stays at the client's size** (T-0156): attach from a
   maximised Windows Terminal (224 columns), close the tab: the panel's terminal keeps 224 columns, lines are cut off
   at the panel edge and the prompt line is out of view.
8. **A frozen service is never reported** (T-0017 / T-0056): with the service suspended for 100 s the app shows no
   "Reconnecting" banner; the panel terminal just says "Connecting…". The client has no heartbeat, so this probably
   happens on macOS too (the M1 / M3 recipes expect the banner).
9. **The debug build's service dies of a stack overflow (`0xc00000fd`) on Resume.** The release build resumes fine.
   Only affects dev builds.
10. **External ID isn't shown on the ticket card** (T-0041): saved (the Ticket tab shows PAY-77) but the card has no
    chip.
11. **Double-click on the header doesn't restore a maximised window** (T-0159); dragging the header down does. The
    maximise button keeps its □ glyph when maximised (Windows shows the restore glyph).
12. **Plain Ctrl+, opens Settings while a terminal has focus** (T-0158): the keyboard map gives the terminal Ctrl+, and
    the app Ctrl+Shift+, there.
13. **Chain highlight doesn't point at hidden chain members** (T-0037): with SHOP-20 selected, its children SHOP-21 and
    SHOP-26 are inside Backlog's "18 more" and nothing says so (the frame has the "N above · … is in this chain" chip
    and the "Chain of SHOP-20" footer).
14. **Dev's "AGENT WORKING N" counts Interrupted / Asks-you cards** (T-0035): "AGENT WORKING 2" while the header says
    "0 running" and both Dev cards are Interrupted.
15. **Settings → Data shows the logs folder as `~\hc\h1\logs/`** (mixed separators; the database path is fine).

Smaller notes that aren't counted as fails are under each check.

## M11 · Windows checks

### T-0159 · Title bar (*02 → Title bar · Windows & Linux*) — **Fail** (minor, see Fail 11)
- Pass: project at the left, no traffic-light space, divider then – □ × at the right; close is red on hover with a
  measured 40 × 32 px hit area; dragging the empty header moves the window; double-click maximises; hovering maximise
  shows Snap layouts; dragging the header down from maximised restores; Win+← snaps.
- Fail: maximised → double-click the header again → nothing happens (expected: restore, as every Windows app does).
  The maximise button shows the same □ glyph in both states.
- Note: the top resize edge isn't there (dragging the window's top edge moves it); bottom and side edges resize, and
  the 1280 × 800 minimum holds. Setting the size programmatically (`MoveWindow` / `SetWindowPos`) always gives a
  1164 px-high client area whatever height is asked for; dragging works, so this only affects window-management tools.

### T-0158 · Keyboard with the terminal focused (*01 → Keyboard map*) — **Fail** (Fails 4 and 12)
Tested in the Project terminal and the Shell tab with a PowerShell `[Console]::ReadKey` loop, US layout.
- Reach the program: Ctrl+J (LF), Ctrl+C (3), Ctrl+T (20), Esc (27), Ctrl+K/P/N/E/D/F/V, Ctrl+[ (Esc), Ctrl+],
  Ctrl+I, Tab, arrows — **Pass**.
- Act as the app's: Ctrl+Shift+K (search), Ctrl+Shift+P (switcher), Ctrl+Shift+N (quick-create), Ctrl+Shift+I
  (inbox), Ctrl+Shift+J (leaves the terminal), Ctrl+Shift+F (find, "2 of 2"), Ctrl+Shift+C / V (copy / paste),
  Ctrl+Shift+T (Project terminal), Ctrl+Shift+E (expand) — **Pass**.
- **Fail:** Ctrl+Shift+` , Ctrl+Shift+, , Ctrl+Shift+1 and Ctrl+Shift+] do nothing (nothing reaches the shell either).
  Steps: click into the Project terminal, press Ctrl+Shift+` → the footer still says "Typing goes to the shell".
  Likely the shifted key is reported as `~` / `<` / `!` / `}` rather than the unshifted key the bindings use.
- **Fail:** Ctrl+, inside the terminal opens Settings (expected to go to the terminal; the app's key there is
  Ctrl+Shift+,).

### T-0158 · Outside the terminal (Windows column) — **Pass**
Ctrl+K, Ctrl+P (and Ctrl+N inside it to add a project), Ctrl+1, Ctrl+N, Ctrl+, , Ctrl+] (next ticket in the panel),
Ctrl+Alt+5 / 6 (Ticket / History tab), Ctrl+↵ (create in the full form), Shift+↵ (full form from quick-create).

### T-0158 · AltGr on a non-US layout — **Pass**
UK layout, quick-create title field: Ctrl+Alt+4 typed "€" (the field read "a€b"); Ctrl+Alt+5, which has no AltGr
character on UK, switched the panel to the Ticket tab. No character was swallowed.

### T-0159 · Toasts (*11 → OS notifications*, Windows) — **Fail** (Fail 1)
- Pass: `crewboard ask human` from SHOP-17's agent session while the board had focus posted the toast "Windows
  PowerShell · shop-web · SHOP-17 asks a question / Should I split the name everywhere?"; with SHOP-17's terminal
  focused, the same question posted no toast and the card stayed calm ("your turn · in terminal"); clicking the toast
  only dismissed it (known, Q34).
- **Fail — service crash.** Steps (release build, notifications on): start a ticket, run
  `Start-Sleep 8; 'Q?' | crewboard ask human --stdin` in its terminal and click the board within 8 s. The service has
  already posted one toast in this process (here "session interrupted" at start-up). Expected: a second toast.
  Happened: the service log ends with `notification title=shop-web · SHOP-17 asks a question`, the app shows
  "Background service stopped", and the Application event log has `crewboard.exe` faulting with `0xc0000005`
  (release offset 0x955e64; debug offset 0xdc8aa0, same pattern on the debug build's second toast). WER dumps are in
  `C:\ProgramData\Microsoft\Windows\WER\ReportArchive\AppCrash_crewboard.exe_*`. The Windows toast code
  (`attention/notify.rs`, `ToastNotifier`) starts a thread per toast that calls `RoInitialize(MTA)` and never
  uninitialises — a place to start looking.
- Note: every service start re-posts "session interrupted" for the same ticket (4 times over the session).

### T-0117 · Notifications toggle — **Pass**
Settings → Notifications → off: no toast afterwards, and no crash.

### T-0159 · Tools step (*14 → gh and az missing · non-blocking (Windows)*) — **Pass**
git 2.55.0.windows.5, Claude Code 2.1.295 (stub), `winget install GitHub.cli` and
`winget install Microsoft.AzureCLI  &&  az extension add --name azure-devops` wrapping onto two lines like the frame.
Both copy buttons put exactly those strings on the clipboard (checked with `Get-Clipboard`).
- Notes: bold monospace labels (`git`, `claude`, `az + azure-devops`) render in a Courier-like face rather than Geist
  Mono (only Geist Mono Regular/Medium are bundled); no feedback after a copy click; the az command uses `&&`, which
  the default Windows PowerShell 5.1 doesn't accept (cmd and pwsh do).

### T-0155 · A second local account is refused — **Pass**
Created `cbtest2`, ran `crewboard daemon status` and `crewboard -P SHOP list` as that account with
`CREWBOARD_HOME` pointing at the first user's folder while the service was running: "Background service: stopped" and
"error: the background service isn't running". The service log has no entry from it. Account deleted afterwards.

### T-0156 · `crewboard attach` in Windows Terminal — **Fail** (Fails 3 and 7)
- Pass: `crewboard attach SHOP-17` replays the scrollback, typing goes to the session (seen in the app's panel too),
  maximising Windows Terminal resizes the PTY (`[console]::WindowWidth` 78 → 224).
- **Fail:** Ctrl+] doesn't detach (pressed several times; `echo still` afterwards still ran in the agent's session).
  Same in a classic console (`conhost powershell`). Expected: back to the user's prompt.
- **Fail:** after closing the Windows Terminal tab that was attached at 224 columns, the app's panel terminal stays at
  224 columns: lines are cut at the panel's edge and the typed command isn't visible; `[console]::WindowWidth` in the
  panel still prints 224. A later attach from an 80-column console fixed it.

### T-0156 · Run on Windows — **Pass**
Run command `powershell … -File …\serve.ps1 $PORT` with Open URL `http://localhost:$PORT`: the first output line shows
the command with 4001 filled in, the card shows `▶ :4001`, the header "1 process", the panel header the green :4001
pill; "Open localhost:4001" opened Edge at that address; Stop → "Stopped by you at 11:25", the server process is gone
and the port is closed.
- Note (part of Fail 6): in the "What command runs shop-web?" dialog the line "Claude looks at the repo and suggests a
  command." runs to the dialog's edge. Also, once the command field has text the dialog shrinks and moves (the Ask
  Claude row goes away), so a click aimed at Open URL lands elsewhere.

### T-0137 / T-0138 · Shell tab and Project terminal — **Pass**
Windows PowerShell 5.1 (`pwsh` isn't installed) in the worktree (`$env:PORT` = 4001 in the Shell tab) and in the repo
folder for the Project terminal; Ctrl+Shift+T opens/focuses it; the hint disappears at the first key.
- Note: after switching project the drawer keeps showing the previous project's terminal until Ctrl+Shift+T is
  pressed again.

### T-0156 · Real `claude` session on Windows — **Skipped** (needs real claude)
### T-0157 · Linux desktop — **Skipped** (Linux VM, other tester)
### T-0160 · Repeat `scripts/README.md` on a fresh VM — **Skipped** (not a fresh VM; the existing setup built fine)

## General flows

### T-0014 · App skeleton — **Fail** (Fail 5)
- Pass: launching the app with no service starts one (`daemon status` shows it); closing the window ends the app and
  the service keeps running (checked with the app-started service and a CLI-started one).
- **Fail:** a console window opens with the app (from Run, from `start ""`, debug and release) and stays behind it.

### T-0018 · Welcome (*14 → 1 · Welcome*) — **Fail** (Fail 6)
Steps: fresh `CREWBOARD_HOME`, launch. Expected: the three feature lines wrap inside the card, as in the frame.
Happened: each description is one line running past the card's right edge ("…crewboard attach SHOP-18.",
"…Ctrl+J opens the next.", "…written to by settings."). The rest matches, with "Ctrl+J" instead of ⌘J.

### T-0018 · Tool checklist states — **Pass**
Everything installed; `claude` missing (renamed the stub, Re-check) → red row, `npm install -g
@anthropic-ai/claude-code`, Continue disabled with "Install claude to continue" (frame *claude missing · blocking*);
Re-check after restoring it → green, Continue works. The permission-mode warning didn't appear (the VM's Claude Code
settings evidently don't prompt), so that part is **not checked**.

### T-0034 · Add project — **Fail** (Fail 6)
- Pass: valid repo (*3 · Add project · valid*: name, base branch `main`, key SHOP; no Remote line, known), Already
  added ("shop-web is already in Crewboard · Switch to it with Ctrl+1…"), second project from Ctrl+P → Ctrl+N (*Adding
  a second project later*, key BILL), "Project added · BILL" toast, *4 · Done · empty board*.
- **Fail:** *Not a git repo*: the message "~\hc\repos\not-a-repo has no .git folder. Pick the repo's root folder, or run
  git init there first." stays on one line and runs out of the red box and the card (the frame wraps it).

### T-0035 · Board, light and dark — **Fail** (minor, Fail 14)
- Pass: five columns, Dev sections, Done rail, card meta rows, "Quiet 5 min" after five silent minutes, dark theme
  from Settings → Appearance.
- **Fail:** Dev's "AGENT WORKING 2" while the header says "0 running" and both Dev cards (SHOP-17, SHOP-22) are
  Interrupted and drawn above that heading. Expected the section count to match the cards under it.
- Note: the Done rail spells D/o/n/e as stacked upright letters (the frame rotates the word).

### Responsive 1280 / 1440 / 1920 — **Pass**
1280: all five columns with the 34 px Done rail, narrow search. 1440: panel open folds Approved/Done into the
"Approved · Done ›" chip with the fade. 1920: the rail becomes the sidebar ("shop-web · SHOP", "billing-api · BILL",
Add project), all columns open, wide search. Header search width follows the frames at each size.

### T-0040 · New ticket — **Pass**
Quick-create (Ctrl+N; Shift+↵ → full form), full form with title, What and why, three Done-when items, Depends on
SHOP-20 and SHOP-17 (live status chips "Backlog", "Interrupted"), External ID PAY-77, Ctrl+↵ → SHOP-26 created; the
open Stack list updated live. The form body scrolls when it's taller than the dialog.

### T-0041 · Depends-on picker, cycle, External ID — **Fail** (Fail 10)
- Pass: picker lists same-project tickets only ("Tickets from billing-api are not listed…"); SHOP-18 → add SHOP-21 →
  "That would create a loop · SHOP-18 can't depend on SHOP-21: … SHOP-21 → SHOP-18 → SHOP-20 → SHOP-21", link not
  added.
- **Fail:** SHOP-26's card shows "SHOP-17 SHOP-20 · 3 criteria" but no PAY-77 chip (review-M2: "saved and shown as a
  chip on the card"). Ticket card components don't read `external_id`.

### Editing a ticket — **Pass**
Ticket tab → Edit switches to the form in place; Cancel returns to read mode. (A saved edit wasn't tried.)

### T-0036 · Drag and drop — **Pass** (note)
SHOP-17 → Dev starts it; Backlog card over Ready for Review → red outline "Only the agent or Submit for review moves a
ticket to Ready for Review"; Esc during a drag puts the card back; epic SHOP-25 over Dev → "Epics don't go to Dev ·
Drag its tickets to Dev instead."; SHOP-22 (no Done-when) → *Starting with an empty "Done when"* dialog, Start anyway →
it starts. Note: the dragged card is drawn over the forbidden-reason box and hides part of it.

### T-0037 · Chain highlight — **Fail** (Fail 13)
Selecting/hovering SHOP-20 outlines SHOP-17/18/19, dims the rest and draws the arrows (pass). Missing the frame's
"N above / below · X is in this chain" chip and the "Chain of …" footer when members are folded away. Note: the
selection's dimming stays after the panel closes and also greys out filter results.

### T-0038 · Search and filter — **Pass**
Ctrl+K focuses the field, the suggestion list shows the tokens; "receipt" / "wish" / "flaky" filter with "1/28",
"27 hidden by filter"; Esc clears and returns to the board.

### T-0039 · Pin / Unpin — **Pass**
Right-click → Pin to top: moves to the top with the pin icon; Unpin puts it back.

### Dialogs are modal — **Pass**
With the empty-Done-when dialog open, a click on the board's "+ Add ticket" or the header's New ticket only closed the
dialog (cancel) and didn't reach the control; Ctrl+1 while Settings was open did nothing.

### Menus and popovers — **Pass** (notes)
Card context menu (Pin, Move to epic…, Close ticket…, Delete… "Never started · nothing on disk"), project switcher,
inbox popover (440 px), quick-create. Notes on the inbox: the ↵ key hint on a row is stretched to the full row
height; the Next button is indigo where the frame uses the orange accent; the empty state says "You'll get a
notification…" even with notifications off.

### T-0045 · Settings (nav icons, project values) — **Fail** (Fails 6 and 15)
- Pass: every nav item has its icon; project page: Open URL `localhost` → "localhost isn't a URL; use something like
  http://localhost:$PORT" and `.env.local` → "doesn't exist in the repository", both red, shop-web marked in the nav;
  the Run command and Open URL saved from the Run dialog show here; "Show in Explorer" opens Explorer at the folder.
- **Fail:** Tools section: "winget install Microsoft.AzureCLI && az ex…" is cut off at the dialog edge and the az
  row's "optional" column shifts left. Logs folder reads `~\hc\h1\logs/`.

### T-0056 / T-0057 · Terminal panel and find — **Pass** (note)
480 px panel: icon-only tabs, path dropped, grid reflows; find bar with matches and "2 of 2"; selection + Ctrl+Shift+C
/ V. Note: double-clicking "PS" in `PS C:\…` selected "PS C".

### T-0118 / T-0120 · Inbox — **Pass**
Needs you 1 · all projects, row with the question, "session ended by restart · answering resumes it", Answer &
resume → typed answer → the ticket resumed and the Messages section shows the question and "Answered by you".

### T-0017 · Connection banners — **Fail** (Fails 2 and 8)
- Pass: "Background service stopped" banner with Start service after the crash; "Reconnected" toast when a service
  came back; "Service restarted · N sessions were interrupted" / "1 session was interrupted · The background service
  started again at 11:28 after a reboot or crash" banners with Resume all.
- **Fail:** Start service after a crash (see Fails list 2).
- **Fail:** suspended service → no Reconnecting banner within 100 s.
- Note: a clean `crewboard daemon stop` is reported on the cards as "Session lost after reboot".

### Quitting leaves the service running — **Pass**
See T-0014.

### Debug build only — **Fail** (Fail 9)
Steps: debug build, ticket Interrupted, click Resume. The service logs `resuming ticket=SHOP-17` and dies with
`0xc00000fd` (stack overflow). The release build resumes the same ticket fine.
