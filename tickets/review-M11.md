# M11 · Windows & Linux — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0155 | Windows: named pipe with an owner-only ACL | c5d5745 |
| T-0156 | Windows: ConPTY sessions, paths and caller identity | ded3898 |
| T-0157 | Linux: socket, PTY and paths | a19c1ea |
| T-0158 | Win/Linux keyboard: Ctrl+Shift shortcuts while the terminal has focus | ba9b5f1 |
| T-0159 | Win/Linux window chrome, notifications and install commands | 93f7d72 |
| T-0160 | Checks on Windows and Linux (local VMs) | b7498ca |

One commit per ticket; a ticket's `tickets/progress.md` row gets its hash with the next ticket's commit. Every commit
was checked with `scripts/check.sh` on macOS on exactly the staged state (a separate worktree with its own target
folder). The Windows and Linux checks ran on the finished M11 tree (the T-0160 state), not on each commit.

**Check results (final M11 tree):**

| Platform | Machine | fmt | clippy -D warnings | tests |
|---|---|---|---|---|
| macOS | the Mac (Apple silicon), `scripts/check.sh` | ✓ | ✓ | 846 passed, 0 failed |
| Linux | Ubuntu 26.04.1 VM (aarch64), `scripts/check.sh` | ✓ | ✓ | 846 passed, 0 failed |
| Windows | Windows 11 Pro 24H2 VM (ARM64), `scripts\check.ps1` | ✓ | ✓ | 846 passed, 0 failed |

## 2. How to try it

The VMs and how to set them up (Rust 1.99.0, the Visual Studio Build Tools on Windows, the GPUI libraries on
Linux) are in `scripts/README.md`; every step done in the VMs is there. In short:

```sh
# Linux VM (shared folder)
cd /media/VMPal/software-factory
export CARGO_TARGET_DIR=$HOME/cb-target PATH=$HOME/.cargo/bin:$PATH
scripts/check.sh
```

```powershell
# Windows VM: a copy on the VM's disk (the VMPal share can't be canonicalized, which sqlx::migrate! needs)
robocopy Z:\software-factory $env:USERPROFILE\cb-src /MIR /XD target .claude /XF .DS_Store
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\cb-target"
cd $env:USERPROFILE\cb-src
powershell -ExecutionPolicy Bypass -File scripts\check.ps1
```

To try the app on Windows or Linux: `cargo build -p crewboard-app -p crewboard-cli`, set `CREWBOARD_HOME` to a
temporary folder (on Linux a short one, the socket lives in it) and start `crewboard-app` from the desktop session.
On Linux the app needs a Wayland or X11 session (`WAYLAND_DISPLAY` / `DISPLAY` set). Notifications: Settings →
Notifications.

What the agent saw in the VMs (screenshots through VMPal):
- **Linux (GNOME, Wayland):** the app opens with its own title bar: "Crewboard", the onboarding steps, then a
  divider and – □ × at the right; maximize and restore work. The Tools step shows `sudo apt install gh` and the
  `curl … InstallAzureCLIDeb | sudo bash && az extension add --name azure-devops` line. The app window is larger than
  the VM's 1280 × 754 screen (the app's minimum is 1280 × 800), so maximized it overflows a little there.
- **Windows 11:** the app opens with the same title bar (system-handled caption buttons; maximize works), git
  "2.55.0.windows.5" and "Claude Code 2.1.295" found, `winget install GitHub.cli`, and the az line wrapping onto a
  second line exactly as the frame. A test toast ("shop-web · SHOP-22 needs permission") appears attributed to
  "Windows PowerShell"; clicking it doesn't reach the process (see Q34).

## 3. Hand checks

**T-0155 Named pipe**
- [ ] On Windows, a second local account can't use the service: sign in as another user, set `CREWBOARD_HOME` to
  the first user's data folder, run `crewboard daemon status` → "isn't running" (the pipe refuses the account), and
  the first user's service log has no request from it.

**T-0156 ConPTY, paths, identity**
- [ ] `crewboard attach SHOP-1` in Windows Terminal: the scrollback, typing, resize, and **Ctrl+] detaches** (the
  test's pseudo console doesn't pass Ctrl+] on as a key, so the automated attach test stops before the detach on
  Windows).
- [ ] A real `claude` session on Windows: drag a ticket to Dev; the Terminal tab shows Claude Code; typing, resizing;
  Stop ends it (and anything it started); `crewboard show` from inside it acts as the agent (`--actor` can't change
  that), from another terminal as the user.
- [ ] Run on Windows with a command like `npm run dev -- --port %PORT%` (or `$PORT`): the first line shows the
  command with the port, Running once the port answers, Stop ends the dev server.
- [ ] Shell tab and Project terminal on Windows: PowerShell (`pwsh` when installed) in the worktree / project folder.

**T-0157 Linux**
- [ ] The app on a Linux desktop: Open URL opens the browser; Settings → Data → **Show in Files** opens the folder.

**T-0158 Keyboard** (*01 · Components → Keyboard map*)
- [ ] Windows and Linux, terminal focused: Ctrl+J, Ctrl+C, Ctrl+T, Esc reach `claude`; Ctrl+Shift+K (search),
  Ctrl+Shift+P (switch project), Ctrl+Shift+N, Ctrl+Shift+I, Ctrl+Shift+J (leaves the terminal), Ctrl+Shift+`
  (leave), Ctrl+Shift+F (find in scrollback), Ctrl+Shift+C / V (copy / paste) act as the app's.
- [ ] Outside the terminal: the Windows/Linux column (Ctrl+K, Ctrl+P, Ctrl+1–9, Ctrl+Alt+1–6, Ctrl+↵, Ctrl+⌫ …).
- [ ] On non-US Windows layouts Ctrl+Alt+digit is AltGr+digit (a character in text fields): check that switching tabs
  with Ctrl+Alt+1–6 doesn't swallow typed characters there (d0058's choice; worth a look, not changed).

**T-0159 Chrome, notifications, install commands**
- [ ] *02 · App shell & project switcher → Title bar · Windows & Linux*: project at the left (no traffic-light room),
  divider and – □ × at the right with 40 × 32 hit areas, close red on hover; drag the empty header to move the
  window, double click to maximize; Windows: hover maximize shows snap layouts; Linux: drag a window edge to resize.
- [ ] *11 · Banners, toasts & OS notifications → OS notifications*: on Linux a permission prompt (or `ask human`)
  posts "shop-web · SHOP-22 needs permission"; clicking it brings Crewboard forward on SHOP-22 with the terminal
  focused (with the app closed, it starts the app on SHOP-22). Note: with the VM's screen locked, notify-send
  reported "Actions are not supported by this notifications server"; check the click on an unlocked GNOME session.
- [ ] Same on Windows: the toast appears (header "Windows PowerShell", no buttons, Q34); never for the focused ticket;
  a click does nothing.
- [ ] *14 · First run & onboarding → gh and az missing · non-blocking (Windows)*: on Windows `winget install
  GitHub.cli` and `winget install Microsoft.AzureCLI  &&  az extension add --name azure-devops` (wrapping), copy
  buttons copy them; on Linux the apt / curl commands.

**T-0160 Checks**
- [ ] Repeat `scripts/README.md` on a fresh VM if you want to confirm the steps are complete.

## 4. Implementation choices (SDD §26)

- **Transport** (`crewboard_core::ipc`): one module for both endpoints. Windows pipe name
  `\\.\pipe\crewboard-<16 hex digits>`: FNV-1a of the lower-cased socket path (stable across builds; one pipe per
  data folder). Security descriptor `O:<SID>D:P(A;;GA;;;<SID>)`: no Administrators, no SYSTEM; first instance
  exclusive; remote clients rejected; every connection's account checked again from the client process's token.
  The socket file path is never created on Windows.
- **Service lock on Windows:** locked on one byte at offset 2^32 (Windows locks are mandatory; a lock over the file
  would hide the PID that `daemon status` and "already running (PID …)" read). Same-file check by volume serial and
  file index.
- **Detached start on Windows:** `daemon start` starts the service in a new process group with no window and leaves
  the caller's job when the job allows it; this process's standard handles are made non-inheritable first (Windows
  hands every inheritable handle to a child, so the service kept a caller's output pipe open and anything reading
  it, a script or a test, waited forever).
- **Sessions on Windows:** ConPTY through portable-pty; the service's environment is passed explicitly (portable-pty
  starts from the registry's environment otherwise, losing PATH changes); the service answers ConPTY's startup
  cursor query (ESC[6n) itself, once, and keeps it out of the scrollback; Stop and the end of the service end the
  session's job object (`process::Group`; SIGHUP / SIGKILL of the process group on macOS and Linux).
- **Caller identity on Windows:** ToolHelp parent IDs (a "parent" that started after its child is a reused PID and
  no parent), then the agent session's job: Git Bash (how Claude Code runs commands on Windows) replaces a process
  by starting a new one, so the Windows parent chain breaks inside a session; the job still holds every process.
- **Shells (Q33):** setup and Run: `cmd /d /s /c "<command>"`, handed to an outer `cmd` through
  `%CREWBOARD_COMMAND%` so quotes and operators reach it untouched; Run also fills in `$PORT`. Shell and Project
  terminal: PowerShell (`pwsh` if on PATH, else Windows PowerShell, `-NoLogo`). Status line: Git Bash
  (`CLAUDE_CODE_GIT_BASH_PATH`, else `bin\bash.exe` of the git on PATH), as Claude Code runs it; the status-line
  command in Crewboard's settings file is `'C:/…/crewboard.exe' statusline`.
- **Paths:** `paths::canonical` (canonicalize without the `\\?\` prefix git refuses); PATH joined with the platform
  separator; PATHEXT lookup (`gh.exe`, `az.cmd`); Claude Code's managed settings folder per OS
  (`C:\Program Files\ClaudeCode`, `/etc/claude-code`).
- **Local clock:** `local_time` (localtime_r / mktime; localtime_s / _mktime64 on Windows), used by the core and the
  app (the app no longer depends on libc).
- **attach on Windows:** raw VT console mode, keys read as console events (Ctrl+] as a key), window size polled four
  times a second (a console has no resize signal).
- **Keyboard:** bindings generated from the d0058 table; on Windows and Linux the "in terminal" column is bound in
  the terminal's key context; the terminal's own copy / paste / font keys move to Ctrl+Shift there.
- **Window chrome:** `window_controls`: Windows control areas (the system handles the caption buttons and dragging);
  Linux client-side decorations with gpui actions and edge resize handles; nothing changes on macOS.
- **Notifications (Q34):** Linux: `notify-send --app-name=Crewboard --action=default=Open --wait`, watched for up to
  an hour; a click publishes `OpenFromNotification` to the connected app (a subscribed client), or starts the app
  with `CREWBOARD_OPEN=<KEY>` (`inbox` for a grouped one). Windows: a toast through the Windows Runtime under
  Windows PowerShell's identity, title and text only.
- **Install commands:** winget (Windows; the frame's commands), apt (Linux; Debian/Ubuntu as the hint; az through
  Microsoft's install script), Homebrew (macOS).
- **Linux app:** GPUI's `wayland` and `x11` features are enabled on Linux (without them the app panics at start).
- **Tests on Windows** use Git for Windows' Unix tools (`sh`, `cat`, `sleep`; `check.ps1` puts its `usr\bin` at the end
  of PATH), as macOS and Linux use theirs: stand-ins for `gh` / `az` are `name.cmd` running `name.sh` with Git's sh;
  raw protocol requests from inside a session use a PowerShell pipe client instead of `nc -U`; identity tests use
  PowerShell's `$PID` (Git's sh numbers processes its own way); Run and setup tests use `cmd` syntax; the test
  repositories turn `core.autocrlf` off; `tests/setup.rs` became `tests/worktree_prep.rs` (Windows asks for
  administrator rights to run any program whose name contains "setup").
- **Run's port check** asks again for up to 100 ms before "port in use" (a port briefly held by another check's test
  socket in a process forked at that moment; seen as a flaky Run test on Linux).
- **New crates:** `windows-sys` 0.61 (Win32 calls) and `windows` 0.62 (the toast), Windows only, both already in the
  lock through tokio and gpui.

## 5. Open questions

Added to `tickets/questions.md`:
- **Q33** — Which shell runs commands and terminals on Windows: built as option 1 (cmd for commands, PowerShell for
  terminals, Git Bash for the status line).
- **Q34** — Notifications on Windows and Linux: built as option 1 with a change found in the VM: Windows hands a
  toast's click only to an app registered for its identity, so the Windows toast has no buttons and a click does
  nothing (as on macOS, Q22); the click on Windows comes with a registered identity (option 2 / packaging).
- **Q35** — VM access (blocker, resolved by the user: no snapshots, Windows signed in).

Notes for later work:
1. **Flaky test seen on macOS:** `the_plugin_hook_commands_run_under_sh` failed once (of many runs) with `sh: child
   setpgid (47620 to 47620): Operation not permitted` on stderr; it passed on every rerun and isn't caused by M11.
2. **Windows tests are slow** (MSYS `sh` startup inside ConPTY): the agent-command tests take 30–60 s each binary;
   the whole Windows run is about 40 minutes in the ARM64 VM.
3. On Windows the service's own port check binds all interfaces; Windows may show a firewall prompt for
   `crewboard.exe` the first time (allowing or cancelling both work; the check only binds). Worth a look when
   packaging.
4. The app's minimum size (1280 × 800) is larger than small laptop screens and the VMs' displays.
5. The VMs' setup state: both have Rust 1.99.0 and the build tools; Windows has `%USERPROFILE%\cb-src` (the mirror),
   `cb-target`, `cb-logs` and `cb-scripts` (helper scripts used by the agent), firewall listen prompts off; Ubuntu
   has `~/cb-target` and `gdb` (installed while looking at the app). No snapshots were taken (Q35).
