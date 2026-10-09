# M1 · Background service — review

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| T-0006 | Protocol: versioned JSON messages, events and build ID | 83513ab |
| T-0007 | CLI skeleton and global conventions | 267912f |
| T-0008 | `crewboard daemon start / stop / status` | 9406532 |
| T-0009 | Socket locked to the current OS user (macOS) | a9c7058 |
| T-0010 | Event stream subscription | c67e1e8 |
| T-0011 | Client library: connect, start the service, reconnect | 57c6f0e |
| T-0012 | Build-ID / protocol check and user-clicked service restart (core) | d55b058 |
| T-0013 | Tools checklist and permission-mode check (core) | 6723bb8 |
| T-0014 | GPUI app skeleton connected to the service | 6f9cf2f |
| T-0015 | Design foundations in GPUI: tokens, themes, fonts, icons | cabff05 |
| T-0016 | Components: Button, Icon Button, Kbd, Banner, Toast | 7aad362 |
| T-0017 | Banner area, toasts and service banners | 2d28913 |
| T-0018 | Onboarding: welcome and tools checklist | af42f97 |

## 2. How to try it

Use a throw-away data folder so nothing touches your real `~/.crewboard/` (keep its path short: the socket path must
stay under ~100 characters, which `mktemp -d` does):

```sh
scripts/check.sh                                  # fmt, clippy -D warnings, tests
cargo build --workspace                           # builds crewboard, crewboard-app (and gpui, first time ~minutes)
export CREWBOARD_HOME=$(mktemp -d)

target/debug/crewboard --help                     # only `daemon` is listed so far
target/debug/crewboard daemon start               # "Background service started (PID …)"
target/debug/crewboard daemon status              # running, PID, build ID, protocol, uptime
target/debug/crewboard daemon status --json
target/debug/crewboard daemon start               # "already running", exit 0
target/debug/crewboard daemon stop

target/debug/crewboard                            # opens the app (crewboard-app next to it); starts the service
CREWBOARD_GALLERY=1 target/debug/crewboard-app    # also opens the hidden gallery window
```

The app always uses the `crewboard` binary next to it to start or restart the service, so build the whole workspace
(`cargo build --workspace`), not only the app.

Reaching the states for the hand checks:

- **First run / onboarding:** a fresh `CREWBOARD_HOME` has no projects, so the app opens on 1 · Welcome. In M1,
  Continue ends onboarding for this launch (Add project is T-0034); it shows again on the next launch.
- **claude missing / gh and az missing:** the service checks the tools on *its own* `PATH`, which it inherits from
  whatever started it. Stop the service, then start the app with a reduced PATH:
  `target/debug/crewboard daemon stop; PATH=/usr/bin:/bin target/debug/crewboard-app` (git is in /usr/bin; claude,
  gh and az aren't). For gh/az missing but claude found, put a folder with only `claude` and `git` on PATH instead.
- **Permission-mode warning:** point Claude Code's config folder at a temporary one for this run only (your real
  settings are never touched): `mkdir -p /tmp/cc && echo '{"permissions":{"defaultMode":"default"}}' >
  /tmp/cc/settings.json`, stop the service, then `CLAUDE_CONFIG_DIR=/tmp/cc target/debug/crewboard-app`.
  A fresh `CREWBOARD_HOME` is needed again once you have dismissed it (the dismissal is remembered).
- **Background service stopped:** with the app open, run `target/debug/crewboard daemon stop`. The red banner shows;
  nothing restarts on its own; **Start service** starts it again.
- **Reconnecting:** shows while the connection is lost but the service process still lives, e.g. for a moment during
  a restart. To look at it at leisure use the gallery's banner-area demo (toggle "Reconnecting").
- **Service update ready:** close the app, keep the service running, change any file in `crates/crewboard-core`
  (e.g. `touch crates/crewboard-core/src/lib.rs`), `cargo build --workspace`, open the app. Restart now → the
  confirmation → Restart service.
- **Restart required:** `cargo build --workspace` and `crewboard daemon start`; then set `PROTOCOL_VERSION` to `2` in
  `crates/crewboard-core/src/protocol.rs`, `cargo build --workspace` again and open the app (only Restart works; the
  content is greyed). After the restart, set it back to `1`, rebuild, and `crewboard daemon stop`.
- **Banner stacking and toasts:** the gallery's "Banner area · stacking" section raises any combination of situations
  (including the usage limit, which really arrives in M8) and pushes success/error toasts.

## 3. Hand checks

**T-0014 · App skeleton**
- [ ] Launch `target/debug/crewboard` with a fresh `CREWBOARD_HOME`: the window opens and (after the first-run
  screens' Continue) shows "Connected · Background service · PID … · build …"; `crewboard daemon status` reports the
  service the app started.
- [ ] Close the window (red traffic light): the app process ends; `crewboard daemon status` still reports the
  service running. (The agent checked the second half by ending the app process.)

**T-0015 · Foundations** (gallery window, `CREWBOARD_GALLERY=1`, use the System / Light / Dark buttons)
- [ ] `crewboard.pen → 01 · Foundations → Color Tokens`: every swatch and name, light and dark.
- [ ] `01 · Foundations → Status Legend`: four tiers, every pill's colour, icon and label, light and dark.
- [ ] `01 · Foundations → Type · Spacing · Icons`: type scale samples (Geist / Geist Mono), spacing, radius, density
  line, and the icon grid at 14 px / 1.5 stroke. Known difference: the display style's −0.3 letter spacing isn't drawn
  (GPUI has no letter spacing).

**T-0016 · Components** (gallery)
- [ ] `01 · Components → Primitives (Kbd, Button, Icon Button)` and `Buttons → Button matrix`: all four variants ×
  six states and the icon-button row, light and dark. Try the live "Approve" demo: a double click runs the action once.
- [ ] `01 · Components → Keyboard map`: the gallery's Kbd table against the frame (macOS column first on a Mac).
- [ ] `01 · Components → Banners → Banner` and `Overlays & feedback`: the four banners and four toasts.

**T-0017 · Banners, toasts, restart**
- [ ] `11 · Banners, toasts & OS notifications → Banners → Background service stopped` (red, server-off icon). Known
  difference, see §5: M1 says "Agent sessions ended with it. Nothing restarts on its own." and offers **Start
  service**; the frame's session count and **Review & resume** belong to the recovery flow (M7).
- [ ] `→ Background service reconnecting` (neutral, no close button) and `02 · App shell & project switcher →
  Background service reconnecting` (banner under the header; the board arrives in M2). After reconnecting, a quiet
  "Reconnected" toast appears and hides after 6 s.
- [ ] `→ Service update ready` (accent, dismissable, "Restart now (N agents working, N waiting)") and `→ Restart
  required` (amber, no close; the content is greyed and ignores clicks).
- [ ] `→ Row → Service restart · confirm`: the dialog's texts; Esc, Cancel and a click outside cancel; only
  "Restart service" restarts; afterwards a "Background service restarted" toast.
- [ ] `→ Stacking · usage limit + reconnecting` (gallery demo): at most two rows, order stopped › restart required ›
  usage › permission › update ready, and reconnecting folding into the top banner as "· ↻ reconnecting to the
  background service…".
- [ ] Toasts: bottom-right, at most three, success hides after 6 s, the error toast stays and "Copy command" copies.

**T-0018 · Onboarding**
- [ ] `14 · First run & onboarding → 1 · Welcome` and `Dark · welcome` (switch macOS to dark mode).
- [ ] `→ 2 · Tool checklist · everything installed`: versions as shown on your machine.
- [ ] `→ claude missing · blocking`: red row, explanation, install command, disabled Continue with "Install claude
  to continue".
- [ ] `→ gh and az missing · non-blocking (Windows)`: on macOS the commands are `brew install gh` and
  `brew install azure-cli && az extension add --name azure-devops`; Continue works.
- [ ] The copy buttons put the exact command on the clipboard; Re-check runs the check again.
- [ ] Permission warning: inline in the Tools card as in the gh/az frame; after Continue (if not closed) the
  `11 · Banners → Permission-mode warning · first run` banner with "Read the docs" (opens the Claude Code docs).
  Closing either is remembered: a relaunch with the same `CREWBOARD_HOME` doesn't show it again.

## 4. Implementation choices (SDD §26)

- **Protocol:** newline-delimited compact JSON, max 16 MiB per frame; envelope `{protocol_version, id?, caller?,
  request|reply|event|error}`; errors `{"error": {"kind", "message", "data"?}}`. `ping`, `service.status` and
  `service.stop` are version-independent (only optional fields may be added), so an app can always read an old
  service's counts and stop it.
- **Build ID:** `<git short hash>[-dirty].<build time in Unix seconds>`, regenerated whenever a `crewboard-core` file
  changes (the service lives in the core, so app-only edits don't ask for a service restart). **Any** build ID that
  differs from the app's counts as "service older" (d0022 says "if they differ"), including the rare case of an app
  older than the service.
- **Service:** one per data folder via an exclusive `flock` on `service.lock` (holds the PID). The OS drops the lock
  on a crash, so a leftover lock file or socket is replaced on the next start. `daemon start` launches the hidden
  `crewboard daemon run` in its own process group (Ctrl+C in that terminal doesn't reach it); SIGHUP is ignored;
  SIGTERM/SIGINT stop it cleanly. Start and stop wait up to 10 s.
- **Socket lock-down:** data folder `0700`, socket `0600` at every start (also when they existed looser), peer UID
  checked on every connection.
- **Events:** a subscriber is "too far behind" when its outgoing queue (256 messages) is full or it missed bus
  events (1024 kept); it is then disconnected and refetches after reconnecting.
- **Client:** reconnect backoff 100 ms doubling to 2 s; a launched service has 10 s to answer; reconnecting never
  starts a service. "Stopped" vs "reconnecting" in the app = whether the PID in `service.lock` is still alive.
- **Agent counts** in `service.status`: live agent sessions, "waiting" = ticket state in `asks_you`, `needs_you`,
  `idle`. These state names must match the types T-0019 defines. Always 0 until M3.
- **Tools check:** tools are found on the service's PATH; macOS install commands: git `xcode-select --install`, claude
  `npm install -g @anthropic-ai/claude-code` (as in the frame), gh `brew install gh`, az `brew install azure-cli && az
  extension add --name azure-devops`. Permission mode: `permissions.defaultMode` from managed settings
  (`/Library/Application Support/ClaudeCode/managed-settings.json` + `managed-settings.d/`) then
  `~/.claude/settings.json` (`$CLAUDE_CONFIG_DIR` respected); `default`/`manual`/`acceptEdits`/`plan` → prompting;
  `auto`/`dontAsk`/`bypassPermissions` → not; `disableAutoMode: "disable"` → prompting; no mode set → unknown (no
  warning), because the built-in default depends on the claude version and plan. Source: code.claude.com docs
  "Choose a permission mode" and "Deploy managed settings", read 2026-10-09.
- **gpui:** pinned to Zed tag v1.23.2, commit `c0199504b9cc` (2026-10-07), with `gpui_platform` (font-kit). Zed's
  workspace patches `async-task`, so ours mirrors that patch; `unicode-properties` is held at 0.1.3 in `Cargo.lock`
  (gpui_web needs `=0.1.3`). Bump all three together.
- **Fonts and icons:** Geist and Geist Mono 1.7.2 (OFL 1.1), 78 Lucide icons from lucide-static 1.54.0 (ISC) with
  `stroke-width` set to 1.5; licences in `crates/crewboard-app/assets/`.
- **UI:** transparent macOS title bar with the app's own 44 px bar; focus ring = 2 px spread shadow (no layout shift);
  no spinners (the loader icon is static); the permission-mode dismissal is stored as
  `onboarding.permission_warning_dismissed` in the global `settings` table (new requests `onboarding.state` and
  `onboarding.dismiss_permission_warning`; typed settings come in T-0028).
- **New crates:** serde, serde_json, clap, rustix (getuid, PID check), gpui, gpui_platform; tokio features net,
  process, signal, sync, time.

## 5. Open questions and notes

No questions were added to `tickets/questions.md`. Things worth your attention:

1. **"Background service stopped" in M1 differs from its frame.** The frame (and brief 16) shows "4 sessions were
   interrupted (reboot or crash). Nothing restarts on its own." with **Review & resume**, which leads to the recovery
   flow of M7 (T-0111). M1 has neither session counts nor that flow, so the banner says "Agent sessions ended with
   it. Nothing restarts on its own." and offers **Start service** (a click; no confirmation, since nothing runs). M7
   should replace both. Likewise the "Service restarted · Resume all" banner after a restart is M7; M1 shows a toast.
2. **The design's `github` icon no longer exists in Lucide** (brand icons were removed). Not used before M10; it will
   need a replacement (another Lucide icon or a separately licensed mark).
3. **PATH when the app is opened from Finder/Dock.** The service inherits the PATH of whatever starts it. Started from
   a terminal (as in v0.1's personal build) that is your shell's PATH; a Finder-launched app gets a minimal PATH, so
   `claude`/`gh` installed in Homebrew or `~/.local/bin` would show as missing. Not a problem yet; worth deciding when
   installation is decided (open question in d0062).
