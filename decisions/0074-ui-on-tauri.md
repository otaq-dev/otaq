# 0074. The UI moves to Tauri before v0.1; GPUI is dropped

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0001 (superseded), 0002, 0022, 0053, 0055, 0072

## Context
0001 chose GPUI, the UI framework inside the Zed editor. By the time M12 was underway (193 of 206 v0.1 tickets done), its
footing looked weaker than when it was chosen:
- GPUI has no repository of its own. It lives inside Zed's monorepo, and Crewboard pins it to a Zed commit.
- The official `gpui` crate on crates.io is still 0.2.2, published in October 2025.
- In 2026 Zed said GPUI work would focus on Zed's own needs. The alternatives are a one-person republish
  (`gpui-unofficial`) and a quiet community fork (gpui-ce).

Tauri has its own repository and regular stable releases: 2.12.3 came out on 2026-10-10, and 3.0 is in alpha.

The user does not want to ship v0.1 on a library that has no repository of its own.

## Decision
- The desktop app's UI is rebuilt on **Tauri 2**. Pages are HTML/CSS/TypeScript, shown in the system webview: WebKit on
  macOS, WebView2 on Windows, WebKitGTK on Linux. The Rust side is Tauri's backend.
- **v0.1 is released with the Tauri UI, not GPUI.**
- Unchanged: `crewboard-core`, `crewboard-cli`, the plugin, the socket protocol, and the app's role as a client of the
  background service (0002, 0022).
- **All Tauri calls are kept in one small Rust module and one small TypeScript file.** The rest of the app never
  names Tauri, so the upgrade to Tauri 3 touches only those two places. That upgrade's known breaking changes are
  renames plus the Linux move to GTK4 and WebKitGTK 6.
- The look stays the design's own (0057): the same custom look on every OS, not native widgets.

## Alternatives considered
- **Stay on GPUI, pinned to a Zed commit.** It works today, but the user won't ship on it (see Context).
- **A GPUI republish or fork** (`gpui-unofficial`, gpui-ce, gpui-box). These don't solve the problem: each is a small
  outside project tracking Zed's monorepo.
- **Finish v0.1 on GPUI, then move to Tauri.** This was recommended so v0.1 would come sooner. The user declined it.

## Consequences
- 0001's trade-off is reversed and accepted: the UI is HTML in a system webview, "not truly native". What we gain:
  - text inputs, selection and copy, scrolling, IME and accessibility come with the webview;
  - the Pencil designs map naturally onto HTML/CSS.
- About 34k lines of GPUI drawing code in `crewboard-app` are rewritten. The view-model logic (about 19k lines, no GPUI,
  tested) is kept in Rust (0075).
- **Risks:**
  - Linux (WebKitGTK) is the weakest platform for rendering speed.
  - Webviews claim some keys for themselves (on Windows, for example, Ctrl+F, Ctrl+P and F5); they must be switched off
    to keep 0058's shortcut map.
  - The project has two languages, and `scripts/check.sh` gains a frontend build step.
- Every UI hand check against the design frames is redone on the new UI.
- **The rest of M12 (decided 2026-10-11):**
  - **Now, on GPUI-era `main`:** the core, CLI and plugin work of the remaining M12 tickets (T-0198–T-0202, plus the
    core parts of T-0193–T-0197 and T-0205).
  - **After that:** the UI migration.
  - **Built in the new UI:** the UI parts of those tickets.
  - **After the new UI:** the VM checks of T-0206.
  - No new GPUI screens are built. The GPUI app only gets the changes it needs to keep compiling.
- **Follow-ups:** decided on 2026-10-12 in 0075 (test build first, React with TypeScript, screen logic split between Rust
  and React, xterm.js, strict TypeScript and ESLint, Node 24 with npm, milestone M14, the SDD and prompt updates).

## Changelog
- 2026-10-11: created (user decision); supersedes 0001.
- 2026-10-11: the rest of M12: core, CLI and plugin work now; UI parts held for the Tauri rebuild (user decision).
- 2026-10-12: the follow-ups are decided in 0075.
