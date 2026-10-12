# 0075. The Tauri UI's stack and how it is built (M14)

- **Status:** Accepted
- **Date:** 2026-10-12
- **Related:** 0002, 0022, 0053, 0055, 0056, 0058, 0062, 0069, 0071, 0073, 0074

## Context
0074 moved the UI to Tauri 2 and left six follow-ups to decide one at a time: a test build first or not, the frontend
framework, where the screen logic lives, the terminal library, the milestone and its tickets, and the updates to `prompts/`
and `SDD.md`. They were decided in one grilling session on 2026-10-12, together with the TypeScript rules and the toolchain.

What exists today: the GPUI app (`crewboard-app`) has about 34k lines of drawing code and **66 view-model files, about 19k
lines and 282 tests**, of which only 4 files touch GPUI. Many view models call the core's rules directly (for example
`view_models/drag.rs` turns `check_move` into "Only agents move tickets to Ready for Review"). The terminal is about 2,900
lines on `alacritty_terminal`: attach with scrollback replay, offsets, selection, scrolling, Jump to latest, Find. The service
already sends the app each session's raw output.

## Decision
**1. A test build comes first, with a go / no-go from the user.** A small throwaway Tauri app, on its own branch, not merged:
- a board with a few hundred cards (scroll and drag), and 5–10 live xterm.js terminals fed by the real service through the
  existing socket;
- the webview's own keys switched off (on Windows, for example Ctrl+F, Ctrl+P, F5) so 0058's map works;
- one drag and one dialog built the way of point 3.

It runs on macOS and in the Ubuntu VM, and in the Windows VM when it's up. **A VM that won't start or can't be reached is
skipped**: that platform is recorded as unchecked and the rest goes on. It ends with a short written report; the user says
go or no-go. Its pieces may be copied into the real crate afterwards.

**2. Frontend: React with TypeScript, built with Vite.** TypeScript is required; no JavaScript files in the app's source.
Ready-made parts are allowed (for example dnd-kit, Radix, cmdk, CodeMirror); each new npm package is named, with why, in the
commit body, like new crates.

**3. Where the screen logic lives: split by kind.**
- **Rust** keeps everything that comes from the board's data and rules: card wording, which buttons are enabled, banners,
  which columns accept a dragged card, inbox items. The existing view models move into a crate without GPUI or Tauri
  (`crewboard-view`) and keep their tests.
- **React** keeps only the moment of interaction: hover, focus, which menu or dialog is open, text being typed, where a
  dragged card is. Rust sends what an interaction needs up front (for example, at drag start, which columns accept the card),
  so nothing waits on a round trip per mouse move.
- **Types shared by both sides are generated from the Rust structs with `ts-rs`**, never written twice. A check fails when
  the generated files are out of date. (Not tauri-specta: it would tie the types to Tauri, against 0074's "one module names
  Tauri".)

**4. Terminal: xterm.js with its WebGL renderer** (and its Find and links add-ons). The Rust side passes each session's raw
output through; the attach-and-replay logic (scrollback, then live output from the right offset) is kept as plumbing.
`alacritty_terminal` goes away with the GPUI app.

**5. TypeScript rules.**
- `tsconfig`: `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, `noUnusedLocals`,
  `noUnusedParameters`. Not `exactOptionalPropertyTypes`.
- **ESLint with typescript-eslint**, warnings fail (`--max-warnings 0`):
  - the `strict-type-checked` preset (no `any`);
  - `no-floating-promises`, `no-misused-promises`;
  - `switch-exhaustiveness-check`: a new state added in Rust breaks the build until the UI handles it;
  - `react-hooks` (rules-of-hooks, exhaustive-deps) as errors;
  - `jsx-a11y`;
  - `no-restricted-imports` for `@tauri-apps/*` outside the one Tauri file (0074);
  - `import/no-extraneous-dependencies`;
  - disabling a rule needs a written reason.
- **Prettier** for formatting.
- Frontend unit tests run with Vitest.

**6. Toolchain: Node 24 (LTS) with npm.** `package-lock.json` is committed and installs use `npm ci`. The Node version is
pinned in `.nvmrc` and `package.json` `engines`. `scripts/check.sh` stops with a clear message on the wrong version. The Tauri
CLI comes in as an npm dev dependency (`@tauri-apps/cli`), so nothing is installed globally.

**7. `scripts/check.sh` gains the frontend steps** (0056): the Node version check, `npm ci`, the ts-rs freshness check,
`tsc --noEmit`, ESLint, the Prettier check and Vitest, next to `cargo fmt`, clippy and `cargo test`.

**8. The milestone: M14 · Tauri UI** (0053), in this order:
1. **Test build** and the user's go / no-go (point 1).
2. **Groundwork:**
   - the new crate `crewboard-desktop` with Tauri 2, React, Vite and TypeScript, and the `crewboard-view` crate, into
     which **all the view models move at once** (the frozen GPUI app is touched once, and screens can be built side by
     side);
   - the one Rust module and the one TypeScript file that name Tauri;
   - the service connection, the ts-rs types, the TypeScript rules and the `check.sh` steps;
   - the design tokens and the 15 themes as CSS variables (0069, 0070);
   - the app's own window chrome and the shortcut map (0058).
3. **Screens**, one ticket per design-brief area, each connecting its view models (already in `crewboard-view`) to the
   page and checked by hand against its frames.
4. **M12's held UI parts** (T-0193–T-0197, T-0201, T-0203–T-0205), written again for the new UI.
5. **Removing the GPUI app:**
   - delete `crewboard-app`, `gpui`, the Zed git dependencies and `alacritty_terminal`;
   - `crewboard-desktop`'s binary takes the name `crewboard-app`, so the CLI launcher and 0072's archive layout don't change;
   - regenerate `THIRD-PARTY-LICENSES.md` with an npm-packages part added (0071).
6. **VM checks:** M11's checks and T-0206 on Windows and Ubuntu, and every UI hand check redone against the frames.

Until step 5 the GPUI app stays in place, **frozen**: it only gets the changes it needs to keep compiling. About and the
update check (0071, 0073) are not in M14; they come with the release work.

**9. Documents.** `SDD.md` §5.5 holds the rules above, and every M14 ticket inherits them. `prompts/implementer.md`,
`orchestrator.md` and `ticket-writer.md` say Tauri. `prompts/m14-tauri.md` is the brief the M14 orchestrator runs.

## Alternatives considered
- **Straight to the rewrite:** faster if everything works, but a Linux (WebKitGTK) problem would show up halfway through
  rewriting 34k lines.
- **Svelte 5, SolidJS, Vue:** fewer ready-made parts, and agents know them less well. Solid stays the fallback if React is
  too slow in the test build.
- **All screen logic in TypeScript:** 19k lines rewritten and re-tested, and core rules such as `check_move` copied into a
  second language where they can drift.
- **All screen logic in Rust, including hover and drag:** a round trip per mouse move.
- **Keeping `alacritty_terminal` in Rust and drawing its cells in the page:** we'd rebuild drawing, selection, cursor and
  Find highlighting ourselves, which is the part xterm.js already does.
- **Biome or oxlint instead of ESLint:** faster, but their type-aware rules are a subset, and Biome has no full React hooks
  rules.
- **pnpm or bun:** pnpm is one more tool on every machine and VM, and its strictness is covered by an ESLint rule. bun is
  younger, and its Windows support has had rough edges.

## Consequences
- Two languages in one repository; `check.sh` gets longer.
- The test build can stop M14: on no-go, the user decides what happens next (for example Solid, or a different terminal).
- Open implementation choices, recorded in the review files: the CSS approach (CSS variables from the tokens are required),
  where the frontend folder sits, and how the diff view highlights (the Rust `syntect` output or a web highlighter).
- Installing the WebKitGTK development packages in the Ubuntu VM, and Node in both VMs, is part of the VM work (0062's
  VM rule: the teammate may set up the VMs).

## Changelog
- 2026-10-12: created (grilling session, user decisions on each point; the VM skip rule at the user's request)
- 2026-10-12: the view models all move into `crewboard-view` in the groundwork, not screen by screen (Q41, user decision).
