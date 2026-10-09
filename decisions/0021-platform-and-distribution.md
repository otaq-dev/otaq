# 0021. Platforms and distribution

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0001, 0008, 0011, 0012, 0014

## Context
Open-source launch aimed at developers. The author is on macOS.

## Decision
- **No Apple signing for now.** Users see the macOS "unverified developer" warning. The target users are
  developers and understand it. Revisit only if it becomes a real adoption problem.
- **No claude CLI version checks.** The factory does not record tested versions and does not warn about newer
  ones. Claude Code updates too often for this to be worth maintaining.
- **macOS, Windows and Linux are all officially supported at launch.** CI builds and tests all three (until the repo has a remote: the local checks, with Windows and Linux in VMs, 0056).
  tmux is replaced by the factory's own background service (0022).
- A first-run checklist shows required tools that are missing (git, the claude CLI, and optionally gh/az),
  with the command to fix each. The factory never installs or updates them.

## Open
- Package channels per platform (for example Homebrew, winget or Scoop, and a Linux package or AppImage). Not decided yet.

## Changelog
- 2026-10-09: created
- 2026-10-09: all three platforms at launch; tmux replaced (0022).
- 2026-10-09: "CI" means the local checks of 0056 until the repo has a remote; Windows and Linux run them in VMs (gap review, user decision).
