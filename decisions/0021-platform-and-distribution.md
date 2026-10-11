# 0021. Platforms and distribution

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0001, 0008, 0011, 0012, 0014

## Context
Open-source launch aimed at developers. The author is on macOS.

## Decision
- **No paid code signing on any platform.** No Apple Developer Program (signing and notarization, $99 a year) and no
  Windows code-signing certificate. Crewboard is an open-source side project that may never earn money, so it doesn't pay
  for a yearly license. Signing isn't a login: it is how macOS and Windows learn who made an app, and only affects the
  warnings people see the first time they open it (below). The target users are developers and understand them.
  - **macOS:** an app downloaded from the web is blocked on first open ("Apple could not verify Crewboard…"). Since
    macOS 15 the user allows it once in System Settings → Privacy & Security → **Open Anyway** (or removes the quarantine
    flag with `xattr -dr com.apple.quarantine Crewboard.app`). Apple silicon still needs a free **ad-hoc** signature
    (`codesign -s -`); the Rust linker adds one to plain binaries, and an app bundle gets one when it is built.
  - **Windows:** SmartScreen shows "Windows protected your PC" on first run; the user clicks More info → **Run anyway**.
  - **Linux:** nothing to sign.
  - Free signing for open-source projects (for example the SignPath Foundation on Windows) may be looked at before the
    public launch. Nothing that costs money.
- **No claude CLI version checks.** The factory does not record tested versions and does not warn about newer
  ones. Claude Code updates too often for this to be worth maintaining.
- **macOS, Windows and Linux are all officially supported at launch.** CI builds and tests all three (until the repo has a remote: the local checks, with Windows and Linux in VMs, 0056).
  tmux is replaced by the factory's own background service (0022).
- A first-run checklist shows required tools that are missing (git, the claude CLI, and optionally gh/az),
  with the command to fix each. The factory never installs or updates them.

## Open
- ~~Package channels per platform~~. Decided in 0072: GitHub Releases archives and a one-line install script; a Homebrew
  tap, Scoop and the AUR may wrap them later.
  Without notarization, **Homebrew's main cask repository is likely closed**: Homebrew announced it would stop accepting
  casks that fail macOS's Gatekeeper check from 2026-09-01 (not yet confirmed that it took effect). A Crewboard tap of its
  own, or a plain download, would still work.

## Changelog
- 2026-10-09: created
- 2026-10-09: all three platforms at launch; tmux replaced (0022).
- 2026-10-09: "CI" means the local checks of 0056 until the repo has a remote; Windows and Linux run them in VMs (gap review, user decision).
- 2026-10-11: no paid code signing on any platform (Apple or Windows); what users see instead; Homebrew note (user decision).
- 2026-10-11: package channels decided (0072).
