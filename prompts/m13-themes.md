# Prompt: write and build milestone M13 · Themes

Start **one fresh Claude Code session** in this repository and paste everything below the line. The session needs the
**pen.dev MCP connection** (tools `mcp__pencil__*`) with `crewboard.pen` open. It writes the M13 tickets, then builds them
one by one, then stops. M13 is unrelated to M12, so it may run before M12 is built (decision 0053), but it must be the only
session building code on `main`.

---

You are the planning and implementing agent for **milestone M13 · Themes** of **Crewboard**, a native Rust (GPUI) kanban
app that drives Claude Code agents. The design now has 15 themes. Your job has two phases: **(A)** write the M13 tickets,
then **(B)** build them, one ticket at a time, following the existing runbook. Then stop.

## 1. Read first
1. `CLAUDE.md`, then `decisions/README.md`. Decisions are the source of truth.
2. **`decisions/0069-themes.md`** (the whole milestone comes from it), and 0053 (the M13 row), 0023, 0054, 0057, 0062.
3. `SDD.md`: §0, §2, §3.2, §20 (Settings), §21 (UI, frame map, visual rules, **UI-11**), §23 (quality and definition of
   done), §24 (the M13 row), §26.
4. `design-briefs/00-README.md`, `01-design-system.md`, `14-first-run-and-onboarding.md`, `15-settings.md`.
5. **`prompts/implementer.md`**: your runbook for phase B. Follow everything below its `---` line, except where this prompt
   says otherwise (§4).
6. `prompts/ticket-writer.md` §5: the ticket format for phase A.
7. `tickets/progress.md` (the T-0015 row: how the tokens were built), `tickets/review-M1.md` and `tickets/review-M2.md`
   (implementation choices for the theme, settings and onboarding), and the intro of `tickets/v0.1-tickets.md`.

## 2. The design (read-only)
Load the pen.dev tools with ToolSearch
(`select:mcp__pencil__get_app_state,mcp__pencil__execute,mcp__pencil__get_style,mcp__pencil__read_skill`), then call
`mcp__pencil__get_app_state` and check that `crewboard.pen` is the open document. If it isn't, stop and ask. **Never modify
the document.** It is encrypted: never `Read` or `grep` a `.pen` file.

What is there:
- **Variables** (`GetVariables()` inside `mcp__pencil__execute`). The theme axes are `mode` (`light`, `dark`) and `palette`.
  Crewboard's values carry both, `{mode:"dark", palette:"crewboard"}`; every other theme carries only its palette,
  `{palette:"dracula"}`. There are 39 color tokens (`bg-app` … `diff-del-bg`, the same names and order as
  `Colors::NAMES` in `theme.rs`) plus `ansi-0` … `ansi-15`. Fonts, radii and spacing are not themed; ignore them. The full
  output is large: print one compact line per token and palette instead of the raw JSON.
- **`01 · Theme palettes`**: one card per theme in two groups, `Dark themes` and `Light themes`, showing its values, status
  pills and a terminal and diff sample. Use it to check your work.
- **`14 · First run & onboarding`** → the row with **`2 · Theme`** ("Pick a look"). Step indicator: Welcome → Theme → Tools
  → Project → Done. Theme cards are named `Theme · <Name>`. Each holds a `Preview · <Name>` (a mini window drawn with that
  theme), a name row with a check when selected, and a `Tagline` ("Neon city after dark").
- **`15 · Settings`** → `Global · Usage, notifications, data, appearance` (light) and its dark version: the Appearance
  section with the same picker, the "In use · OS is light/dark" tag and the **Theme credits** row. **`Theme credits ·
  dialog`** is in the same frame.
- Name frames in tickets as `crewboard.pen → <frame> → <section>`, and check that each one exists.

## 3. Phase A: write the M13 tickets
- Append a section **`## M13 · Themes`** at the end of `tickets/v0.1-tickets.md`, with a one-line goal. Number the tickets
  from **T-0168**; M12's tickets will be written later and take the numbers after yours. Add an M13 row to the intro table
  and a short intro note that M13 is built before M12 (decision 0053). Add **UI-11** to `tickets/coverage.md`.
- Every ticket cites UI-11 and decision 0069, plus any other decisions it touches. Same format, size and rules as
  `prompts/ticket-writer.md` §3–§5: core first, then UI; no features beyond 0069.
- Suggested cut. Change it if the code shows a better one, and say why in the intro note.
  1. **Core settings:** store a dark theme and a light theme next to `appearance` (`crewboard-core/src/board/settings.rs`,
     the `stored_enum!` and `GlobalSettings`). Default `crewboard` for both. An unknown stored name falls back to Crewboard.
     Protocol and tests.
  2. **Theme palettes in the app:** `theme.rs` grows from two hard-coded sets to the 15 themes: 39 tokens plus 16 terminal
     colors each. Resolve mode + dark theme + light theme + OS appearance into the active theme; `ActiveTheme` keeps live
     switching. Extend the existing `token_values_match_the_design` test so it pins **every token of every theme**,
     including `ansi-*`, to the design's values.
  3. **Terminal colors from the theme:** `terminal/colors.rs` stops owning `LIGHT` and `DARK`; the 16 colors come from the
     active theme (`terminal/element.rs` builds `Base` from it). Crewboard's own terminal colors change to the design's
     values (0069 Consequences); update the tests.
  4. **Colors that assume Crewboard:** the four `gpui::white()` texts in `shell.rs` (needs-you counters, the selected
     project icon) use `text_on_accent`. Look for any other place that assumes dark means Crewboard Dark. Leave the Windows
     close-button red in `window_controls.rs` as it is; it is the OS convention.
  5. **Theme picker component:** mode switch, a dark row and a light row of theme cards (preview, check, tagline), shared by
     onboarding and Settings. Every click is applied at once and saved. The gallery (`CREWBOARD_GALLERY=1`, `gallery.rs`)
     gets a palette switch so each theme can be compared with `01 · Theme palettes`.
  6. **Onboarding Theme step:** `view_models/onboarding.rs` (`Step`, `STEPS`) and `onboarding.rs`: Welcome → Theme →
     Tools → Project → Done. People who already finished onboarding don't see it.
  7. **Settings → Appearance and Theme credits:** the picker, the "In use" tag in System mode, the credits row and the
     dialog (`settings.rs` around the Appearance section, `view_models/settings.rs`). Each credit entry's copyright line,
     license and theme list are copied exactly from the design. The "Source" links aren't in the design; use the projects'
     own repositories, and list them as an implementation choice in the review file:
     Catppuccin `https://github.com/catppuccin/catppuccin`,
     Tokyo Night `https://github.com/enkia/tokyo-night-vscode-theme`,
     Dracula `https://github.com/dracula/dracula-theme`,
     Nord `https://github.com/nordtheme/nord`,
     Gruvbox `https://github.com/morhetz/gruvbox`,
     Rosé Pine `https://github.com/rose-pine/rose-pine-theme`,
     Atom One Dark `https://github.com/atom/atom/tree/master/packages/one-dark-syntax`,
     Primer Primitives `https://github.com/primer/primitives`,
     Solarized `https://github.com/altercation/solarized`.
- The Diff tab's syntax colors already come from status tokens (`diff_view.rs`), so they follow the theme with no change.
  Say so in the ticket that checks the Diff tab, rather than adding work.
- Commit the tickets on their own: `M13 tickets: themes (UI-11, decision 0069)`.

## 4. Phase B: build them
Follow `prompts/implementer.md` (§1–§7) with milestone **M13**:
- One ticket at a time in file order, `scripts/check.sh` green before every commit, one commit per ticket, a row in
  `tickets/progress.md`, ticked boxes only for what you verified, hand checks left unticked.
- Hand checks: compare with the frames by hand. Cover at least Crewboard Light and Dark, one Catppuccin, Dracula and
  Solarized Light in the gallery against `01 · Theme palettes`; the onboarding step; Settings → Appearance in System mode
  while the OS switches; and the credits dialog. You can't do these yourself: prepare them with exact steps in the review
  file.
- Questions: defer as the runbook's §3.2 says, in `tickets/questions.md` with the next free Q number. Stop only for a
  blocker (runbook §7).
- **Windows and Linux:** M13 adds no platform code. List "run `scripts/check.ps1` / `check.sh` in the VMs" in the review file
  as an optional hand check; don't run the VMs yourself.
- At the end, write `tickets/review-M13.md` (runbook §5), commit it, and stop. **Don't start M12.**

## 5. Rules that always apply
- Everything in the runbook's §7: don't edit `decisions/`, `SDD.md`, `DESIGN.md`, `design-briefs/`, `CLAUDE.md` or
  `crewboard.pen`; respect SDD §2; no installs; don't change git config, Claude Code settings or the shell profile.
- **Other sessions may use this checkout.** Stage your own files by path. Never `git add -A` or `git add .`, never stash,
  never rewrite history, never push. Leave files you didn't create alone (for example `tickets/handcheck-*.md`). If
  `git status` shows changes in a file you need to edit that aren't yours, stop and ask.
- The checks take about 15–25 minutes on this Mac (`syspolicyd` scans the test binaries). Run them in the background and
  wait for them; don't cut them short.
- Commit trailer for every commit: use the attribution lines your session gives you for commits.

## 6. Final report
Reply with:
- the M13 tickets with their commit hashes
- the hand checks, grouped by screen, pointing to `tickets/review-M13.md`
- the implementation choices that need the user's judgement (the credits links among them)
- any questions you deferred
- the final `scripts/check.sh` result
