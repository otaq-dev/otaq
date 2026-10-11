# M13 · Themes — review

Decision 0069, requirement UI-11. M13 was built before M12 (decision 0053); M12's tickets are not written yet.

## 1. What was built

| Ticket | Title | Commit |
|---|---|---|
| — | M13 tickets: themes (UI-11, decision 0069) | 1056d84 |
| T-0168 | Core: a dark theme and a light theme next to the mode | e38cfcf |
| T-0169 | Theme palettes in the app: 15 themes, resolved from the mode and the OS | 63cf997 |
| T-0170 | Terminal colors from the active theme | 9bee8ef |
| T-0171 | Colors that assumed Crewboard | 5c8e43e |
| T-0172 | Theme picker component | 42edcd5 |
| T-0173 | Onboarding: the Theme step | 922b97f |
| T-0174 | Settings: Appearance and Theme credits | a308561 |
| T-0169 (fix) | The stored theme is applied once per change, so the gallery's switch isn't undone | f11ce31 |

One commit per ticket; a ticket's `tickets/progress.md` row gets its hash with the next commit. Every commit was
checked with `scripts/check.sh` on macOS on exactly the staged state (a separate worktree with its own target folder).
Final M13 tree: `scripts/check.sh` green (fmt, clippy -D warnings, tests: **887 passed, 0 failed**, 1 ignored as before).

## 2. How to try it

Use a throw-away data folder so nothing touches your real `~/.crewboard/`:

```sh
scripts/check.sh                       # fmt, clippy -D warnings, tests
cargo build --workspace
export CREWBOARD_HOME=$(mktemp -d)
target/debug/crewboard daemon start
target/debug/crewboard-app             # no projects yet: opens on onboarding 1 · Welcome
```

- **Gallery** (the palettes, without the service): `CREWBOARD_GALLERY=1 target/debug/crewboard-app`. The top row has
  System / Light / Dark; the second row has one button per theme (dark themes, then light ones, as in *01 · Theme
  palettes*). A theme button switches to that theme and its mode. *Color Tokens* shows the 39 tokens and `ansi-0` …
  `ansi-15`; under it the shared theme picker (applied in the gallery, not saved).
- **Onboarding**: with the fresh `CREWBOARD_HOME` above, Welcome → **Choose a theme** → Pick a look → **Check my
  tools** → Tools → Continue → Project. Once a project is added, onboarding is done and never shows again.
- **Settings**: add a project (or finish onboarding), press ⌘, and click **Appearance** in the nav.
- **The OS switch**: System Settings → Appearance → Light / Dark (or Auto) while the app is open.
- **Terminals**: add a project, then ⇧⌘T (Project terminal) and run `ls --color=always /`, or a 16-colour chart:
  `for i in $(seq 0 15); do printf '\e[48;5;%sm %3s \e[0m' $i $i; [ $i = 7 ] && echo; done; echo`.
- **Stored values**: `sqlite3 "$CREWBOARD_HOME"/crewboard.db "select key, value_json from settings where key in
  ('appearance','dark_theme','light_theme')"`.

## 3. Hand checks

Compare the running app with the named frames. Pairs of light and dark frames: switch macOS's appearance, or pick
the mode in the app.

**T-0169 · Theme palettes** (gallery)
- [ ] `crewboard.pen → 01 · Theme palettes → Dark themes → Crewboard Dark` and `→ Light themes → Crewboard Light`:
  in the gallery press *Crewboard Dark*, then *Crewboard Light*; compare every swatch of *Color Tokens* (surfaces &
  lines, text & accent, status, terminal & diff) and the status pills of *Status Legend* with the card.
- [ ] Same for `Catppuccin Mocha` (or any Catppuccin), `Dracula` and `Solarized Light`: the swatches, the 16
  terminal colours (`ansi-0` … `ansi-15` at the end of *Color Tokens*) and the pills.
- [ ] Live switching: set the gallery to **System**, switch macOS between Light and Dark: the gallery follows
  (Crewboard Light ↔ Crewboard Dark, or the themes picked in its picker).
- [ ] The Diff tab follows the theme with no change of its own (syntax colours come from status tokens): open a
  ticket's Diff tab (any ticket with commits) in **Dracula** and in **Solarized Light** (Settings → Appearance), and
  compare with the card's *Terminal & diff* sample (added/removed backgrounds, keyword/string colours).

**T-0170 · Terminal colours**
- [ ] In the Project terminal (⇧⌘T), run the 16-colour chart above and `ls --color=always /` in **Tokyo Night** and
  in **Solarized Light**: the colours match `01 · Theme palettes → <theme> → Terminal & diff`; background and text
  are the theme's `term-bg` / `term-fg`.
- [ ] With the terminal open, switch the theme in Settings: the terminal recolours at once and the shell keeps
  running (type a command afterwards).

**T-0171 · Colours that assumed Crewboard**
- [ ] `02 · App shell & project switcher → Header states`, `→ Another project needs you`, `→ Quick switcher · ⌘P`:
  the needs-you counters (inbox button, rail, switcher) and the selected project's icon, in **Crewboard Dark** (white
  as before) and in **Dracula** (dark text on the orange and the purple accent). Needs a ticket in Needs you (a
  permission prompt in a ticket's session, as in review-M4 §3) and two projects (the seed of review-M2).
- [ ] In **Catppuccin Latte** and **Dracula**, gallery `01 · Components → Badges & Inputs`: the orange and accent count
  badges and the checked checkbox read well on their fill.

**T-0172 · Theme picker** (compare in onboarding and Settings; the gallery shows it too)
- [ ] `14 · First run & onboarding → 2 · Theme → Dark theme / Light theme` and `15 · Settings → Global · Usage,
  notifications, data, appearance → Dark theme / Light theme`: the card grid (5 per row), each preview (title bar
  dots, sidebar, three columns with status dots) drawn in that theme's own colours, the accent ring and ✓ on the
  selected card, names and taglines ("Neon city after dark"), the hover border on the others.
- [ ] `→ In use` tag: in System mode, "In use · OS is light" on the Light theme section while macOS is light, and
  "In use · OS is dark" on the Dark theme section while it is dark; it moves when macOS switches; no tag in Light or
  Dark mode.
- [ ] A click on a card or a mode recolours every window at once (with the gallery open too), and after quitting and
  reopening the app the same mode and themes are back.

**T-0173 · Onboarding Theme step** (fresh `CREWBOARD_HOME`)
- [ ] `14 · First run & onboarding → 1 · Welcome` and `→ Dark · welcome`: the button reads **Choose a theme**; the
  step indicator reads Welcome · Theme · Tools · Project · Done.
- [ ] `→ 2 · Theme`: title "Pick a look", the description, the mode switch at the top right, both sections, "Change it
  anytime in Settings → Appearance." and **Check my tools**; Welcome shows as done in the indicator, Theme as step 2.
  Click System + Tokyo Night + Daylight: the window is Tokyo Night while macOS is dark and Daylight while it is light.
- [ ] **Check my tools** opens Tools with the check running (3 in the indicator); Continue leads to Project (4).
- [ ] With a project already added, onboarding never appears; "Check tools" on an empty board (a project with no
  tickets) opens straight at Tools.

**T-0174 · Settings → Appearance and Theme credits**
- [ ] `15 · Settings → Global · Usage, notifications, data, appearance → Appearance` (light) and `15 · Settings → Dark ·
  project settings → Settings · Dark` (its Appearance part): Mode row with its description, the Dark theme and Light
  theme sections, the Theme credits row with **View credits**, then Terminal font size.
- [ ] System mode while switching macOS: the In use tag moves and the window recolours.
- [ ] `15 · Settings → Theme credits · dialog`: title, the introduction, the nine entries (project, "→ themes",
  copyright line in mono, the licence chip — `MIT/X11` for Gruvbox — and **Source ↗**), "MIT License — full text"
  unfolded with its note, and the footer `esc close`, **Copy notices**, **Done**.
- [ ] Each **Source** opens the project's repository in the browser (list in §4).
- [ ] **Copy notices** puts the notices on the clipboard (paste them somewhere; the button then reads "Copied");
  the chevron folds and unfolds the MIT text; Esc, Done and a click outside close the dialog.
- [ ] A small window (e.g. 1280 × 800): the dialog stays inside the window and its body scrolls.

**Optional: Windows and Linux.** M13 adds no platform code. If you want, run `scripts/check.ps1` in the Windows VM
and `scripts/check.sh` in the Linux VM as in `scripts/README.md`, and look at onboarding's Theme step once on each.

## 3a. Visual check in the Windows VM (2026-10-11)

Run on Windows 11 ARM64 (1280 × 720 screen) with the app built from M13, a fresh data folder and a sample repo.

Seen working: onboarding Welcome ("Choose a theme") and the five-step indicator; the Theme step's cards, previews,
ring and ✓, taglines; Light / Dark / System recolouring the window at once; Dracula and Solarized Light picked per
side; System mode following a Windows light/dark switch live, with "In use · OS is light/dark" moving; Tools,
Project and the board in Dracula; the selected project icon in Dracula's dark text-on-accent; the Project
terminal's 16 colours matching Dracula's palette; Settings → Appearance (Mode, both sections, Theme credits, View
credits); the credits dialog (entries, MIT/X11, scrolling body, MIT text folding, Copy notices → "Copied" with
every entry and the MIT text on the clipboard, a Source link opening the Dracula repository, Esc); the choice kept
after a restart.

Fixed after the check (commit a519f3b):
- Text in a row didn't wrap and ran out of its card: Welcome's three descriptions, the Theme step's description
  (which pushed the Light / Dark / System switch outside the card), the "In use · OS is dark" tag beside the long
  Dark theme description (outside the padding, in onboarding and Settings), and the Tools step's permission
  warning. The text columns now shrink and wrap.
- On a window shorter than the Theme step (720 px here), the step couldn't scroll and Check my tools was cut off.
  Onboarding now scrolls, and is still centred when it fits.

Seen, older than M13 and left as they are: Settings is a fixed 720 px tall, so on a 720 px screen its bottom is cut
off; with the Project terminal maximised, the empty board's text is drawn over the header's search box; Copy
notices uses LF line breaks (Windows tools that expect CRLF read it as one line).

## 4. Implementation choices (SDD §26)

- **Credits Source links** (not in the design; please confirm): Catppuccin `https://github.com/catppuccin/catppuccin`,
  Tokyo Night `https://github.com/enkia/tokyo-night-vscode-theme`, Dracula `https://github.com/dracula/dracula-theme`,
  Nord `https://github.com/nordtheme/nord`, Gruvbox `https://github.com/morhetz/gruvbox`, Rosé Pine
  `https://github.com/rose-pine/rose-pine-theme`, Atom One Dark
  `https://github.com/atom/atom/tree/master/packages/one-dark-syntax`, Primer Primitives
  `https://github.com/primer/primitives`, Solarized `https://github.com/altercation/solarized`.
- **Copy notices** text: the dialog's introduction, then per project "Project → themes", the copyright line and
  "License: MIT · <source>", then "MIT License" and its three paragraphs. After a copy the button reads "Copied"
  until the dialog closes.
- **Storage:** the mode is still the `appearance` row; new rows `dark_theme` and `light_theme` hold the design's
  palette keys (`tokyo-night`, `daylight`; Crewboard is `crewboard` on both sides, `naming::THEME_KEY`, because the
  naming test of decision 0028 forbids spelling the product name elsewhere). An unknown or wrong-kind name reads as
  Crewboard; the other settings keep their values. `stored_enum!` now accepts constants as stored strings.
- **Token tables:** generated once from the design's variables (`GetVariables`), one row per token with a column per
  theme, `#[rustfmt::skip]` so they read like the design's table; `token_values_match_the_design` keeps a second,
  independent copy of the design's strings and pins every value of every theme.
- **Terminal colours:** `Base` carries the theme's 16 colours; the 256-colour cube and grey ramp stay xterm's; dim
  colours are still mixed two thirds toward the background (the design defines no dim colours).
- **White texts:** `text_on_accent` replaces white on the needs-you counters, the selected project icon, the Count
  Badge's attention/accent tones and the checked checkbox. Kept: the Windows close button (OS red, white glyph) and
  the Toggle knob (white in the design for every theme).
- **Picker:** one component (`components/theme_picker.rs`) with its logic in `view_models/theme_picker.rs`; the
  caller applies and saves a click: onboarding through `Workspace::set_theme_choice` (which loads the settings first if
  they aren't loaded yet), Settings through its own save so its copy of the settings stays current. Unselected cards
  get a `border-strong` ring on hover (not in the design). The mode switch reuses the app's Segmented control
  (options 10 px apart horizontally instead of the frame's 12).
- **Gallery:** the palette switch sits in T-0169 rather than the picker ticket, so every theme could be compared
  before any picker existed; the gallery also shows the picker (applied, not saved).
- **Dialog:** the credits dialog is a `ConfirmDialog` with two small additions: a footer hint word ("close" instead
  of "cancel") and a body that scrolls when the window is shorter than the dialog. Its title uses the dialogs' 16 px,
  the frame's is 15 px.

## 5. Open questions and notes

No questions were added to `tickets/questions.md`.

Differences seen in the frames that M13 didn't change (older than M13, outside decision 0069):
- **Onboarding step indicator:** the frames draw a finished step as a green (`st-approved`) circle with a white
  check; the app (since T-0018) draws it on `bg-column` with a green check. Now visible on the Theme step (Welcome is
  done).
- **Terminal font size** row: the frame's description is "Geist Mono. Also ⌘+ / ⌘− inside a focused terminal."; the app
  says "Geist Mono, used by the terminal." (T-0045).
- The intro of `tickets/v0.1-tickets.md` still says the user reviews everything after M11; M12 and M13 came later.
