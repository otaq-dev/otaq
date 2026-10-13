# Hand checks · Linux (Ubuntu 26.04 aarch64 VM)

> The first part is the GPUI app's run (2026-10-11), kept as history. The Tauri UI's section (T-0247, 2026-10-13) is
> at the end: every row skipped, and the real-Linux-hardware run not done.

Tested on 2026-10-11 against `main` at 7a71d8d, by using the app as a user would (clicks, typing, drag and drop,
shortcuts) and comparing it with the frames in `crewboard.pen`.

**Setup in the VM**

- GNOME 50.1 on Wayland. The display is 2880 × 1696 at 2× scale, so the screen is 1440 × 848 points. With the
  dock and top bar, a maximized window is about 1372 × 816. For 1920 × 1080 the display was switched to
  3840 × 2160 at 2× for a while, then set back.
- Debug build (`cargo build -p crewboard-app -p crewboard-cli`, target `~/cb-target`). Data folder:
  `CREWBOARD_HOME=~/cbh`, fresh at the start.
- Repositories in `~/repos`: `shop-web` (a clone of a local bare repo, with a small `server.py` that listens on
  `$PORT`), plus `billing-api`, `crewboard` and `notgit` (a plain folder). Tickets were added with the `seed.py`
  from `review-M2.md`, without its move to Dev.
- **No real `claude`.** A stand-in `~/cb-stub/claude` was first on the service's PATH. It answers `--version` with
  "2.1.0" and otherwise opens an interactive bash, so a "session" is a shell. Agent signals were made from inside
  those sessions by hand: `crewboard ask human …`, and `crewboard hook Notification` fed with a
  permission-prompt payload. `~/.local/bin/claude` (a real Claude Code) was not touched and was not on the
  service's PATH.
- The first app launch used a minimal environment. From "Open URL" on, the app was relaunched with the desktop
  session's variables (`XDG_DATA_DIRS`, `SHELL`, …). See the Open URL row.

## Fails, most serious first

1. **Clicking an OS notification does nothing** (T-0159, T-0117). On GNOME 50 the banner closes, but no panel or
   terminal opens. `notify-send` prints "Actions are not supported by this notifications server. Displaying
   non-interactively.", and that also happens with a plain `notify-send --action=default=Open --wait` outside
   Crewboard. GNOME itself lists `actions` among its capabilities. Every `notify-send --wait` process stays alive
   afterwards, one per notification.
2. **Typing in the Shell tab wipes the agent's open question** (T-0124 / T-0075). SHOP-17 had asked "Should totals
   include tax?" with options. I opened its **Shell** tab ("Shell · not the agent") and ran `pwd`. The card went
   back to "Agent Working · Quiet", the options disappeared from the card and panel, the inbox dropped from 2 to 1,
   and `crewboard show SHOP-17` says `Working`. The question was never answered, but it is gone everywhere.
3. **Some Ctrl+Shift shortcuts don't work while a terminal has focus** (T-0158, Keyboard map). Ctrl+Shift+K, P, N,
   I, J, F, C, V, E and T work. These don't:
   - **Ctrl+Shift+`** (leave terminal): nothing happens, although the hint under the terminal advertises it.
   - **Ctrl+Shift+Alt+1–6** (switch tab): types `%` into the shell for 5 and doesn't switch.
   - **Ctrl+Shift+1–9** (switch project): nothing happens.
   - **Ctrl+Shift+,** (Settings): nothing happens.

   The common factor is a key whose Shift form is a different character (` → ~, 5 → %, 2 → @, , → <). Ctrl+Shift+[
   and ] are probably affected too, but I couldn't confirm it (only one ticket was in the column).
4. **The window is bigger than the screen, and the first maximize click shrinks it instead** (T-0159, T-0033). On a
   1440 × 848 screen the app opens filling the work area. The first click on □ (or the first double-click on the
   header) gives a 1440 × 900 window placed under the dock and off the right and bottom edges. In that state the
   project rail, "Crewboard"/the project name and the × button are cut off. Reproduced on two launches. The second
   toggle fits the screen again. Restored, the window never fits this screen, so the caption buttons are
   off-screen. The □ icon also never changes to a "restore" icon.
5. **Settings loses the edit you are typing when you close the dialog** (T-0045). I typed an Open URL, pressed Tab
   (focus stayed in the field) and clicked ×. `open_url` was still empty in the database. If you click elsewhere
   in the dialog first, it saves. Tab doesn't move to the next field.
6. **Keyboard shortcuts stop working after a dialog or menu closes** (T-0030). This happened after dismissing the
   "no Done when" dialog by clicking outside it, and after picking "Pin to top" from a card's menu. Ctrl+N and
   Ctrl+, did nothing until I clicked inside the window.
7. **The chain highlight stays after the panel closes** (T-0037, T-0038). After a card's panel is closed, its
   chain stays highlighted and every other card stays dimmed. Esc and clicking the empty board don't clear it;
   only selecting another card does. The cards a search finds also stay dimmed: "flaky" found SHOP-22 but showed
   it greyed out, and its orange "Needs you" card was dimmed too.
8. **The "AGENT WORKING" label in Dev is in the wrong place with the wrong count** (T-0035, *03 · Board*). The
   orange cards (Asks you, Needs you, Interrupted) sit **above** the "AGENT WORKING 2" label, and nothing is under
   it. The frames put them under it. After the service restart the label still said "AGENT WORKING 2" with no
   agent working.
9. **The Welcome card's text runs out of the card** (T-0018, *1 · Welcome*). The three lines ("Attach from any
   terminal with crewboard attach SHOP-18.", "…Ctrl+J opens the next.", "…written to by settings.") don't wrap and
   cross the card's right edge. The frame wraps them.
10. **The terminal stays light after switching to Dark** (T-0056, *Dark · live terminal*). The board turned dark,
    but the open terminal kept its white background until I clicked into it. Then it turned dark.
11. **Settings → Tools doesn't match its frame** (T-0045 / T-0159, *Global · Tools*). Each install command is
    inline on the tool's row with no copy button, and the az line (`curl -sL https://aka.ms/InstallAzureCLIDeb | …`)
    is cut off at the dialog's right edge. The frame and onboarding show it as a block under the row, with a copy
    button.
12. **A card's tooltip covers its own right-click menu** (T-0039). The title tooltip stays on top of the menu and
    hides its first item, "Pin to top", until the mouse moves into the menu. The same happens with header tooltips
    over the open inbox popover and over Settings.
13. **The Run tab says there's no run command after one was set** (T-0134). Right after setting it in Settings,
    the tab read "No run command yet. Run asks for the command…". Pressing Run started `python3 server.py`, so the
    text was stale.
14. **The "Ready for Review · Approved · Done" chip covers Dev cards** (T-0035). With the panel open, this chip for
    the hidden columns sits on top of SHOP-22 / SHOP-18 in the Dev column.
15. **The connection state is wrong around the service** (T-0017, T-0122):
    - A fresh app launch shows a "Reconnected" toast, though the app had never been connected.
    - With the service stopped, the header still showed "1 process", "2 running" and "Needs you 1".
    - A click on New ticket while it was stopped took effect after Start service: a quick-create row appeared and
      even followed me to billing-api.
16. **A stop of the service is described as a reboot** (T-0121). SHOP-17's card says "Session lost after reboot"
    after `crewboard daemon stop`, when nothing rebooted.
17. **The search placeholder is cut at 1280 px** (T-0033, *1280 × 800 · minimum*). It reads "Search · is:need".
    The frame shows just "Search" at that width.
18. **Smaller visual differences** from the frames:
    - The Done rail stacks the letters D/o/n/e vertically instead of rotating "Done" (T-0035).
    - The onboarding stepper's finished steps are grey circles with a green check, not solid green (T-0018).
    - In the inbox, the selected row's ↵ key chip is stretched to the row's full height (T-0118).
    - The × keeps its grey glyph on the red hover (T-0159).
    - The dock shows a generic gear icon with the tooltip "Unknown": the window has no app id or desktop entry.
      The notifications show a generic icon too (T-0159).

Worth a look, not counted as Fails:

- Ctrl+Shift+J in SHOP-22's focused terminal only left the terminal. SHOP-22's own question then became an inbox
  item and fired an OS notification at once, while its panel was still on screen. A second Ctrl+J was needed to
  reach SHOP-17.
- Terminal echo lags **2–15 s** in this VM: 61 characters took about 2 s to show, a 407-character line about 15 s.
  New-ticket fields lag too. This is a debug build on a VM GPU, so not counted as a Fail, but it is very visible.
- The quick switcher's footer says "Ctrl+N add project", while Ctrl+N is "New ticket" everywhere else.
- Terminal selection doesn't fill the Linux PRIMARY selection, so middle-click paste doesn't work. Copy
  (Ctrl+Shift+C) also includes a trailing space.
- "Show in Files" for the logs folder shows `logs` selected inside `~/cbh` rather than opening it. For the
  database it selects `crewboard.db`, which is fine.
- The Shell tab and Project terminal start `$SHELL -l`. Launched without `SHELL`, the app used `/bin/sh -l`, and
  the login profile put `~/.local/bin` (the real `claude`) on that shell's PATH. Expected behaviour, but a stand-in
  for the agent doesn't keep the real `claude` out of these shells.

## M11 Linux checks (`review-M11.md`)

| Ticket · check | Result | Notes |
|---|---|---|
| T-0159 · title bar matches *Title bar · Windows & Linux*: project at left, divider and – □ × at right, 40 × 32 hit areas, close red on hover | **Pass** | Measured about 40 × 32. Close turns red on hover (glyph stays grey, Fail 18). |
| T-0159 · drag the empty header to move the window | **Pass** | |
| T-0159 · double click to maximize | **Pass** | Toggles. First toggle after launch goes the wrong way (Fail 4). |
| T-0159 · Linux: drag a window edge to resize | **Pass** | The left edge resizes; the handle is about 2 px inside the edge. |
| T-0159 · caption buttons work (– □ ×) | **Fail** | – and × work. □ goes wrong on a small screen, and restored the buttons are off-screen (Fail 4). |
| T-0158 · terminal focused: Ctrl+J, Ctrl+C, Ctrl+T, Esc reach `claude` | **Pass** | Checked with `cat -v` in a session: `^T ^[`, Ctrl+J ends the line, Ctrl+C interrupts. |
| T-0158 · terminal focused: Ctrl+Shift+K / P / N / I / J / ` / F / C / V act as the app's | **Fail** | K, P, N, I, F, C, V, E, T work. J works but only leaves the terminal (see "Worth a look"). Ctrl+Shift+`, Ctrl+Shift+Alt+digit, Ctrl+Shift+digit and Ctrl+Shift+, don't (Fail 3). |
| T-0158 · outside the terminal, the Windows/Linux column (Ctrl+K, Ctrl+P, Ctrl+1–9, Ctrl+Alt+1–6, Ctrl+↵ …) | **Pass** | Ctrl+K, Ctrl+N, Ctrl+, , Ctrl+1/2, Ctrl+J, Ctrl+↵ (create, save) work. Not tried: Ctrl+[ ], Ctrl+⌫, Ctrl+D. Ctrl+Alt+5 inconclusive (that tab was already open). All shortcuts die after a dialog or menu closes (Fail 6). |
| T-0158 · non-US layouts: Ctrl+Alt+digit is AltGr | Skipped | Windows-only. |
| T-0159 · notifications only for blocking items | **Pass** | A permission prompt (SHOP-22) and a question (SHOP-17) each posted "shop-web · SHOP-n needs permission / asks a question". After a service restart: "SHOP-22 session interrupted". Nothing else notified. |
| T-0159 · never for the focused ticket | **Pass** | `crewboard ask human` from SHOP-22 while its terminal had focus: no notification (log has none). One came as soon as I left the terminal. |
| T-0159 · clicking opens the ticket with its terminal focused | **Fail** | Fail 1. |
| T-0159 · Tools step: Linux commands (`sudo apt install gh`, the curl … `az extension add` line) | **Pass** | Matches *gh and az missing* with Linux commands. The az line wraps. |
| T-0159 · copy buttons copy them | **Pass** | `wl-paste` showed the exact text for gh and az. No visual feedback on click. |
| T-0157 · Open URL opens the browser | **Pass** | With the desktop session's environment, Firefox opened `http://localhost:4001`. With a minimal environment (no `XDG_DATA_DIRS`) the URL opened in Text Editor; that's the test setup, not the app. |
| T-0157 · Settings → Data → Show in Files | **Pass** | Files opened `~/cbh` with `crewboard.db` selected (logs: see "Worth a look"). |
| T-0156 · `crewboard attach` in a terminal: scrollback, typing, resize, Ctrl+] detaches | **Pass** | In Ptyxis: replayed scrollback, typing reached the session, `stty size` = 24 80 (window size), Ctrl+] printed "[detached from SHOP-22; its session keeps running]". |
| T-0156 · Run with `$PORT`: command shown, Running once the port answers, Stop ends it | **Pass** | `python3 server.py` on :4001: "Running", HTTP 200. Stop shows "Stopped by you at 12:01". Run again bound :4001 again, so the old server had ended. "Open localhost:4001" only after a re-run (Fail 13). |
| T-0156 · Shell tab and Project terminal in the worktree / project folder | **Pass** | Shell: cwd `~/cbh/wt/shop-web/SHOP-17`, `$PORT`=4001, branch `crewboard/SHOP-17`. Project terminal: `~/repos/shop-web`, `crewboard` works. Fail 2 is a separate problem. |
| T-0156 · real `claude` session: drag to Dev, typing, resize, Stop, identity | Skipped | Needs real claude. |
| T-0155 · a second local account can't use the service | Skipped | Windows-only. |
| Quitting the app leaves the service running | **Pass** | After × the app process ended. `crewboard daemon status`: running, uptime 22 m. The Run process stayed up too. |

## v0.1 open hand checks (`v0.1-tickets.md`)

| Line · ticket | Check | Result | Notes |
|---|---|---|---|
| 83 · T-0005 | unformatted file / clippy warning fails the script | Skipped | Needs a repo file changed; repo edits are off-limits here. |
| 360 · T-0014 | launching starts the daemon; connected/reconnecting | **Pass** | App launch started `crewboard daemon run`. "Reconnecting" never showed (a stop shows the red "stopped" banner). |
| 361 · T-0014 | closing the last window quits; service still running | **Pass** | |
| 363 · T-0014 | by hand: launch, close, service keeps running | **Pass** | |
| 386 · T-0015 | foundations in light and dark | **Pass** | Spot-checked: both themes apply across board, settings, dialogs, cards (terminal: Fail 10). |
| 408 · T-0016 | components in light and dark (disabled ignores clicks) | **Pass** | Disabled Continue and Add project ignore clicks. |
| 428 · T-0017 | Reconnecting on disconnect; global input disabled meanwhile | **Fail** | Not reached via `daemon stop`. In the stopped state a New ticket click took effect later and the header counts stayed stale (Fail 15). |
| 429 · T-0017 | Service update ready / Restart required | Skipped | Needs a core rebuild or a protocol change in the repo. |
| 431 · T-0017 | banners against the frames | **Pass** | "Background service stopped" with Start service; "Service restarted · 2 sessions were interrupted…" with Resume all / Review & resume. Spot-checked only. |
| 450 · T-0018 | first run opens Welcome → Tools; every checklist state | **Pass** | Fresh home → Welcome → Tools. States seen: all found, gh/az missing, claude missing (Continue disabled, "Install claude to continue"). Permission-mode warning not tried. |
| 451 · T-0018 | copy buttons; Re-check calls tools.check again | **Pass** | Re-check picked up a stand-in `gh` added meanwhile, and a removed `claude`. |
| 455 · T-0018 | against the named frames | **Fail** | Welcome text overflow (Fail 9); stepper style (Fail 18). |
| 764 · T-0030 | inputs and menus, keyboard, light and dark | **Fail** | Focus lost after a menu or dialog closes (Fail 6); tooltip over menu (Fail 12). |
| 789 · T-0031 | card flag priority against *04 · Ticket card variants* | **Pass** | Spot-checked live: Asks you pill with the question; `▶ :4001` and links in the meta row; Interrupted with Resume; Quiet n min. |
| 812 · T-0032 | Epic card | Skipped | Not looked at in this session. |
| 831 · T-0033 | window anatomy at 1440 / 1280 / 1920, light and dark | **Fail** | 1920 × 1080: rail expanded with names and counts, five open columns, search ≈ 476 px, all as the frame. 1280: fits with the Done rail, but the search placeholder is cut (Fail 17). Exact 1440 × 900 isn't possible on this screen (1372 × 816). Window sizing: Fail 4. |
| 836 · T-0033 | switcher ordering, badges, shortcuts | **Pass** | Ctrl+Shift+P and click; rows show "1 running"/idle and Ctrl+1–3; Ctrl+1/2 switch. |
| 859 · T-0034 | add project validation with `shop-web` and a non-git folder | **Pass** | Non-git: red field and "Not a git repository · ~/repos/notgit has no .git folder…", Add disabled. Valid: name, base branch, key SHOP. No "(detected from origin/HEAD)" or Remote row (local-path origin). |
| 877 · T-0035 | `shop-web` renders like *03 · Board* light and dark | **Fail** | Fails 8, 14, 18. |
| 882 · T-0035 | column building, fold counts, Done rail | **Fail** | As above. Done folds to a rail at 1280–1372 and opens at 1920. |
| 906 · T-0036 | drop targets; SHOP-24-style no-Done-when warning | **Pass** | Allowed (blue), forbidden (red "Approve a ticket from Ready for Review"), the no-Done-when dialog and Start anyway, Queued "waits for SHOP-17 to merge". |
| 927 · T-0037 | chain and dim set, light and dark | **Fail** | Chains themselves are right (SHOP-20 lights 17, 18, 19, 20; hover SHOP-19 lights 19, 20). The selection sticks (Fail 7). |
| 948 · T-0038 | filter against the frame | **Fail** | The filter itself works ("1 result", "26 hidden by filter", 1/27 counts); results stay dimmed (Fail 7). |
| 970 · T-0039 | Pin / Unpin from the menu, board re-sorts | **Fail** | Pin to top works (SHOP-20 jumps to the top with a pin icon). Item hidden by the tooltip (Fail 12); focus lost afterwards (Fail 6). |
| 992 · T-0040 | quick-create and full form | **Pass** | Ctrl+N row → Shift+↵ full form. Three Done-when items (the last one typed without ↵ was kept). Ctrl+↵ created SHOP-26. |
| 1016 · T-0041 | Depends-on picker, base branch, External ID | **Pass** | The picker found SHOP-17 ("this ticket will wait for it"; other projects are excluded). `crewboard show SHOP-26`: Depends on SHOP-17, External ID PAY-200, body and three criteria. |
| 1036 · T-0042 | agent-edit markers | Skipped | Not looked at. |
| 1060 · T-0043 | epic actions | Skipped | Not looked at. |
| 1082 · T-0044 | Stack list | **Pass** | Indented trees for SHOP-17 and SHOP-20 with status chips; SHOP-20 shows under both parents. |
| 1107 · T-0045 | settings load/save and validation | **Fail** | Fails 5 and 11. Nav icons and Agent limits match. |
| 1157 · T-0162 | Delete from the card menu | **Pass** | "Delete SHOP-1 filler 1? It never started…" → deleted, toast "Deleted SHOP-1". Not tried with dependents. |
| 1337 · T-0052 | real `claude` loads the plugin | Skipped | Needs real claude. The stand-in got `--plugin-dir … --allowedTools Bash(crewboard:*) --settings … --session-id … /crewboard:work-ticket`, cwd = worktree, `CREWBOARD_TICKET`, `PORT`. |
| 1434 · T-0056 | attach, replay, live output, no input dropped | **Pass** | No input lost; heavy echo lag in this VM (see "Worth a look"). |
| 1435 · T-0056 | colours, cursor, attributes | **Pass** | 16, 256 and truecolor, bold, italic, underline, strike, inverse, dim, and CJK (界面) all rendered. |
| 1436 · T-0056 | resize reflows; 480 px and full window | **Pass** | Expand (Ctrl+Shift+E) to full width reflowed and back. 480 px panel not tried. |
| 1439 · T-0056 | frames in light and dark; real claude once | **Fail** | Fail 10. Real claude skipped. |
| 1462 · T-0057 | find in scrollback | **Pass** | Ctrl+Shift+F "cat" → "1 of 1", highlighted; Esc returns to the terminal. |
| 1483 · T-0058 | focus ring and hint | **Pass** | Blue ring; "Typing goes to the agent · Ctrl+Shift+` leave terminal". |
| 1485 · T-0058 | key routing | **Fail** | Fail 3. |
| 1509 · T-0059 | panel navigation and tabs, light and dark | **Pass** | Spot-checked: tabs, expand/collapse, close, tab dots. |
| 1532 · T-0060 | action bar per state | **Pass** | Spot-checked: Start (to Dev) + Edit; Answer in terminal; Resume. |
| 1554 · T-0061 | terminal header strip, message waiting | **Pass** | Spot-checked: Live / Waiting for you / your turn, the attach chip, font buttons. "Message waiting" not reached. |
| 1575 · T-0062 | Ticket tab, Stack, History | **Pass** | Spot-checked: read mode after save, "Created by you", History count. |
| 1596 · T-0063 | Start interrupted | Skipped | Not reproduced. |
| 1842–1844 · T-0073 | plugin with real claude | Skipped | Needs real claude. |
| 1887 · T-0075 | card variants from live state, light and dark | **Pass** | Needs you (permission prompt), Asks you, Quiet n min, Interrupted, "your turn · in terminal", all from live service state. |
| 1891 · T-0075 | signal states against the frames | **Fail** | Fail 2. |
| 1912 · T-0076 | created-by-agent origin | Skipped | Not looked at. |
| 1960 · T-0164 | real claude asks an options question | Skipped | Needs real claude. Options via `crewboard ask human --option` render on card and panel. |
| 2099 · T-0082 | agent limits settings; toggle shows 5 and 3 | **Pass** | Toggling on enables 5 / 3 and the header shows "0 / 5 agents". |
| 2102 · T-0082 | toggle and popover | **Pass** | Popover not opened. |
| 2123 · T-0083 | Queued "waits for" text | **Pass** | "waits for SHOP-17 to merge". |
| 2329–2418 · T-0091–T-0095 | Diff, comments, Send back, Approve / Local merge | Skipped | No ticket reached Ready for Review in this session. |
| 2489, 2673, 2692, 2711, 2732 | conflict flags, Close/Reopen, Park, Parent closed, Epic review | Skipped | Not reached in this session. |
| 2917 · T-0117 | click opens the app, panel and terminal | **Fail** | Fail 1 (Linux). |
| 2919 · T-0117 | macOS check | Skipped | macOS. |
| 2941 · T-0118 | inbox groups and text | **Pass** | Spot-checked: "Asks you · Needs you · Idle" and "Interrupted & parent closed" groups with Answer & resume and Resume all (2). Not the full sample set. |
| 2944 · T-0118 | inbox against the frames | **Fail** | Stretched ↵ chip; tooltip over the popover (Fails 12, 18). |
| 2964 · T-0119 | Ctrl+J stepping | **Pass** | "Done · 1 left · next Ctrl+J · Exit"; opens the item with its terminal focused. |
| 2984 · T-0120 | inline answer | Skipped | Not tried. |
| 3007 · T-0121 | Interrupted states | **Fail** | Interrupted card with Resume works; "Session lost after reboot" after a plain stop (Fail 16). |
| 3026 · T-0122 | after-reboot summary, "reconnected" | **Fail** | "Reconnected" on a fresh launch (Fail 15). The restart banner is fine. |
| 3046 · T-0123 | Cleanup view | Skipped | Not opened. |
| 3065 · T-0124 | "your turn" while the terminal has focus | **Fail** | The agent terminal is right ("your turn · in terminal", no inbox item, no notification). The Shell tab kills the question (Fail 2). |
| 3088 · T-0165 | answer options in the inbox | Skipped | Not tried. |
| 3112 · T-0166 | answer options on the card and panel | **Pass** | Card: 1 Yes / 2 No / 3 Other…; panel: radio rows with keys, "1–4 pick · ↵ sends", Send disabled until a pick. |
| 3166, 3260, 3283, 3307 · T-0126–T-0132 | tokens, plan meter, usage limit | Skipped | Needs real claude usage. |
| 3357 · T-0134 | Run tab, card chip, Open URL | **Fail** | Works, but Fail 13. Chip `▶ :4001` on the card and `:4001 ↗` in the panel header. |
| 3400 · T-0136 | running processes in the header | **Pass** | "1 process" chip while Run is up; gone after stop. |
| 3470 · T-0167 | Project terminal summary of created tickets | Skipped | Not tried. |
| 3498 · T-0139 | workflow skills settings | **Pass** | Global defaults and per-project "use global" fields shown; not tried with an override. |
| 3523 · T-0140 | push / open-pr skills | Skipped | Needs real claude and gh. |
| 3546 · T-0141 | PR provider detection | **Pass** | Local-path origin → "Not detected: use your own Open PR skill". github.com / Azure not tried (needs gh/az). |
| 3725, 3746 · T-0149, T-0150 | Open PR / PR state | Skipped | Needs gh/az. |
| 3818, 3847 · T-0153, T-0154 | finished tickets answering, messages | Skipped | Not reached. |
| 3873 · T-0155 | second user refused | Skipped | Windows. |
| 3932 · T-0158 | keymap on Linux | **Fail** | Fail 3. |
| 3950 · T-0159 | title bar and window controls | **Fail** | Fail 4 (the rest passes; see the M11 table). |
| 3951 · T-0159 | notifications | **Fail** | Blocking-only and focused-ticket rules pass; click fails (Fail 1). |
| 3953 · T-0159 | install commands on Linux | **Pass** | |

## Totals

M11 Linux table: 16 Pass, 3 Fail, 3 Skipped. v0.1 table (one row per line or grouped range): 36 Pass, 22 Fail,
22 Skipped. The skips are mostly real claude/gh/az, Windows/macOS-only checks, and review/PR flows not reached in
this session.

## State left in the VM

The app and service are still running on `~/cbh`, with the shop-web Run (`python3 server.py` on :4001) and two
stand-in sessions (now interrupted). The app theme is set to Dark.

Changes made inside the VM:

- installed `wl-clipboard`
- set Firefox as the default browser (`xdg-settings`)
- switched the display to 3840 × 2160 and back to 2880 × 1696 @ 2×
- added `~/cb-stub`, `~/repos`, `~/cb-logs` and `~/cbh`
- cleared the GNOME notification list once

# Tauri UI · 2026-10-13 (T-0247)

**Not checked.** The Ubuntu VM froze during T-0246's check run (the Mac's disk filled up while both VMs built) and quit;
the user chose "Skip Linux" rather than restart it (decision 0075: a VM that can't be reached is skipped and recorded).
So every row of the GPUI run above has no Tauri-UI result; each one is recorded below as skipped. Nothing about the
Tauri UI on Linux has been seen: the app opening, WebKitGTK's repaint without `WEBKIT_DISABLE_DMABUF_RENDERER` (Q39),
WebGL against the DOM renderer (Q37), the clipboard read, the folder picker, the keys, notifications, the window
chrome. The Ubuntu VM's setup for the Tauri build is in `scripts/README.md` (T-0246) and was not undone.

**Real Linux hardware: not done.** T-0247's first step, one run on a real Linux machine with Intel or AMD graphics
(the terminals with WebGL and with DOM, with and without the DMABUF variable, the board's scroll and drag, typing
echo), needs the user's own Linux machine (or a borrowed one); the teammates have no access to one. It stays open for
the user and is what Q37 (the DOM renderer as Linux's default) and Q39 (the DMABUF variable) wait for; the built
defaults stay as they are until the user answers. The test build's bench (branch `m14-test-build`, `bench/run.sh`)
can give the numbers.

Totals: 0 Pass, 0 Fail, 102 Skipped (22 M11 rows, 80 v0.1 rows); the real-hardware run not done.

## M11 Linux checks (Tauri UI)

| Ticket · check | Result |
|---|---|
| T-0159 · title bar matches *Title bar · Windows & Linux*: project at left, divider and – □ × at right, 40 × 32 hit areas, close red on hover | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · drag the empty header to move the window | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · double click to maximize | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · Linux: drag a window edge to resize | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · caption buttons work (– □ ×) | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0158 · terminal focused: Ctrl+J, Ctrl+C, Ctrl+T, Esc reach `claude` | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0158 · terminal focused: Ctrl+Shift+K / P / N / I / J / ` / F / C / V act as the app's | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0158 · outside the terminal, the Windows/Linux column (Ctrl+K, Ctrl+P, Ctrl+1–9, Ctrl+Alt+1–6, Ctrl+↵ …) | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0158 · non-US layouts: Ctrl+Alt+digit is AltGr | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · notifications only for blocking items | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · never for the focused ticket | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · clicking opens the ticket with its terminal focused | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · Tools step: Linux commands (`sudo apt install gh`, the curl … `az extension add` line) | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0159 · copy buttons copy them | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0157 · Open URL opens the browser | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0157 · Settings → Data → Show in Files | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0156 · `crewboard attach` in a terminal: scrollback, typing, resize, Ctrl+] detaches | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0156 · Run with `$PORT`: command shown, Running once the port answers, Stop ends it | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0156 · Shell tab and Project terminal in the worktree / project folder | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0156 · real `claude` session: drag to Dev, typing, resize, Stop, identity | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| T-0155 · a second local account can't use the service | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| Quitting the app leaves the service running | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |

## v0.1 open hand checks (Tauri UI)

| Line · ticket | Check | Result |
|---|---|---|
| 83 · T-0005 | unformatted file / clippy warning fails the script | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 360 · T-0014 | launching starts the daemon; connected/reconnecting | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 361 · T-0014 | closing the last window quits; service still running | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 363 · T-0014 | by hand: launch, close, service keeps running | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 386 · T-0015 | foundations in light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 408 · T-0016 | components in light and dark (disabled ignores clicks) | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 428 · T-0017 | Reconnecting on disconnect; global input disabled meanwhile | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 429 · T-0017 | Service update ready / Restart required | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 431 · T-0017 | banners against the frames | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 450 · T-0018 | first run opens Welcome → Tools; every checklist state | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 451 · T-0018 | copy buttons; Re-check calls tools.check again | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 455 · T-0018 | against the named frames | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 764 · T-0030 | inputs and menus, keyboard, light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 789 · T-0031 | card flag priority against *04 · Ticket card variants* | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 812 · T-0032 | Epic card | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 831 · T-0033 | window anatomy at 1440 / 1280 / 1920, light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 836 · T-0033 | switcher ordering, badges, shortcuts | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 859 · T-0034 | add project validation with `shop-web` and a non-git folder | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 877 · T-0035 | `shop-web` renders like *03 · Board* light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 882 · T-0035 | column building, fold counts, Done rail | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 906 · T-0036 | drop targets; SHOP-24-style no-Done-when warning | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 927 · T-0037 | chain and dim set, light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 948 · T-0038 | filter against the frame | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 970 · T-0039 | Pin / Unpin from the menu, board re-sorts | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 992 · T-0040 | quick-create and full form | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1016 · T-0041 | Depends-on picker, base branch, External ID | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1036 · T-0042 | agent-edit markers | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1060 · T-0043 | epic actions | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1082 · T-0044 | Stack list | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1107 · T-0045 | settings load/save and validation | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1157 · T-0162 | Delete from the card menu | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1337 · T-0052 | real `claude` loads the plugin | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1434 · T-0056 | attach, replay, live output, no input dropped | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1435 · T-0056 | colours, cursor, attributes | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1436 · T-0056 | resize reflows; 480 px and full window | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1439 · T-0056 | frames in light and dark; real claude once | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1462 · T-0057 | find in scrollback | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1483 · T-0058 | focus ring and hint | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1485 · T-0058 | key routing | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1509 · T-0059 | panel navigation and tabs, light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1532 · T-0060 | action bar per state | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1554 · T-0061 | terminal header strip, message waiting | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1575 · T-0062 | Ticket tab, Stack, History | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1596 · T-0063 | Start interrupted | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1842–1844 · T-0073 | plugin with real claude | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1887 · T-0075 | card variants from live state, light and dark | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1891 · T-0075 | signal states against the frames | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1912 · T-0076 | created-by-agent origin | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 1960 · T-0164 | real claude asks an options question | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2099 · T-0082 | agent limits settings; toggle shows 5 and 3 | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2102 · T-0082 | toggle and popover | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2123 · T-0083 | Queued "waits for" text | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2329–2418 · T-0091–T-0095 | Diff, comments, Send back, Approve / Local merge | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2489, 2673, 2692, 2711, 2732 | conflict flags, Close/Reopen, Park, Parent closed, Epic review | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2917 · T-0117 | click opens the app, panel and terminal | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2919 · T-0117 | macOS check | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2941 · T-0118 | inbox groups and text | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2944 · T-0118 | inbox against the frames | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2964 · T-0119 | Ctrl+J stepping | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 2984 · T-0120 | inline answer | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3007 · T-0121 | Interrupted states | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3026 · T-0122 | after-reboot summary, "reconnected" | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3046 · T-0123 | Cleanup view | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3065 · T-0124 | "your turn" while the terminal has focus | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3088 · T-0165 | answer options in the inbox | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3112 · T-0166 | answer options on the card and panel | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3166, 3260, 3283, 3307 · T-0126–T-0132 | tokens, plan meter, usage limit | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3357 · T-0134 | Run tab, card chip, Open URL | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3400 · T-0136 | running processes in the header | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3470 · T-0167 | Project terminal summary of created tickets | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3498 · T-0139 | workflow skills settings | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3523 · T-0140 | push / open-pr skills | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3546 · T-0141 | PR provider detection | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3725, 3746 · T-0149, T-0150 | Open PR / PR state | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3818, 3847 · T-0153, T-0154 | finished tickets answering, messages | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3873 · T-0155 | second user refused | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3932 · T-0158 | keymap on Linux | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3950 · T-0159 | title bar and window controls | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3951 · T-0159 | notifications | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |
| 3953 · T-0159 | install commands on Linux | Skipped: Ubuntu VM not checked (the user's skip, 2026-10-13) |

## Real Linux hardware (Intel or AMD graphics)

| Check | Result |
|---|---|
| Terminals with WebGL, DMABUF renderer on (no variable) | Not done: needs the user's real Linux machine (Q37, Q39) |
| Terminals with WebGL, `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Not done: needs the user's real Linux machine (Q37, Q39) |
| Terminals with DOM, DMABUF renderer on | Not done: needs the user's real Linux machine (Q37, Q39) |
| Terminals with DOM, `WEBKIT_DISABLE_DMABUF_RENDERER=1` | Not done: needs the user's real Linux machine (Q37, Q39) |
| Board scroll and drag; typing echo | Not done: needs the user's real Linux machine (Q37, Q39) |
