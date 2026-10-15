# Prompt: rename Crewboard → Otaq, new logo, turquoise "needs you"

Start **one fresh Claude Code session** in this repository and paste everything below the line. It needs:
- the **pen.dev MCP connection** with `crewboard.pen` open (the design is already updated; the session reads tokens and
  exports the app icon from it);
- no other session building on `main` while it runs.

This file (`prompts/otaq-rename.md`) is new and uncommitted when you start; the session commits it with step 1.

---

You are renaming **Crewboard** to **Otaq**, a Rust kanban desktop app (Tauri 2 + React UI, decisions 0074/0075) that
drives Claude Code agents. The user decided this on 2026-10-14. The design file `crewboard.pen` already shows the result.
Your job is to bring the **code, docs and decision log** in line with it.

There are three parts:
1. **Name.** "Crewboard" becomes "Otaq" everywhere the user can see it. That includes the identity strings (binary, data
   folder, branch prefix, plugin, bundle id), within the limits the user confirms in §3.
2. **Logo.** The new mark is "Command tent": a tent outline with two board-column lines inside and a turquoise flag on
   the ridge. It replaces the A3 "Moving card" mark everywhere, including the app icon, the About screen and notifications.
3. **Colors.** The "needs you" family (Asks you / Needs you / Idle; token `st-attn`) changes from **orange** to
   **turquoise**. Because turquoise collided with the old teal **In review** (`st-review`), In review becomes **magenta**.
   Two palettes also move **Investigating** (`st-investigate`) to free their pink.

The name comes from the Turkic *otağ*, a commander's tent where reports came in and decisions were made. Use that only
as background for the decision text. The product itself doesn't change.

## 1. Read first
1. `CLAUDE.md` and `decisions/README.md` (the rules for refining and superseding decisions).
2. Decisions **0028** (product name), **0042** (orange family), **0016**, **0055** (data folder), **0070** (one theme),
   **0071** (About screen and copyright), **0072** (GitHub releases), **0073**, **0074** and **0075**.
3. `SDD.md`: the theme, tokens and attention sections. `DESIGN.md`. `design-briefs/01-design-system.md`, `11-…`, `15-…`.
4. `prompts/implementer.md` §5. `scripts/check.sh` must be green before every commit.
5. The design. Load the pen.dev tools with ToolSearch (`select:mcp__pencil__get_app_state,mcp__pencil__execute`) and
   look at:
   - `1. Otaq · Command tent` (`PBIqo`): the logo board.
   - `01 · Foundations` → Status Legend.
   - `03 · Board — shop-web · 1440` Light and Dark.
   - `11 · Banners, toasts & OS notifications`.
   - `15 · Settings`: the About rows.
   - `01 · Theme palettes`.

## 2. Before anything else
- `git status` shows only this prompt file as new, and HEAD is on `main`. If anything else is uncommitted, ask the user
  whose it is. Don't commit, stash or discard it.
- `cargo`, Node 24 (`PATH="$HOME/.nvm/versions/node/v24.21.0/bin:$PATH"`) and `scripts/check.sh` work on a clean tree.

## 3. Settle these with the user first, one topic at a time
`CLAUDE.md` says to decide one topic at a time. Ask each question on its own and give the recommendation. Record the
answers in the decisions (step 1). Don't start the identity rename (step 4) until 3a–3e are answered.

- **3a. How far the identity rename goes.** Today the identity is spread across:
  - crate names `crewboard-cli/-core/-desktop/-view`;
  - binary names `crewboard` and `crewboard-app`;
  - env vars `CREWBOARD_*`;
  - data folder `~/.crewboard/` (d0055);
  - branch prefix `crewboard/` (`naming.rs`);
  - plugin name `crewboard` (`plugin/.claude-plugin/plugin.json`), whose skills and hooks call the `crewboard` CLI;
  - Tauri `productName` and `identifier` (`io.github.owner.crewboard`).

  *Recommendation:* rename all of them now. v0.1 isn't released and only the user dogfoods it.
- **3b. Existing data.** *Recommendation:*
  - On first start, if `~/.crewboard/` exists and `~/.otaq/` doesn't, move it once and log it. Treat `CREWBOARD_HOME` as
    a fallback for `OTAQ_HOME` for one release.
  - New tickets get `otaq/` branches. Existing branches and worktrees keep the names already stored in the database.
    Never rename a live branch.
  - Ask whether the user's own skills or settings call `crewboard`, and whether a temporary `crewboard` → `otaq` shim is
    wanted. The recommendation is no shim.
- **3c. Bundle identifier.** Changing it gives macOS a new app identity: notification permission is asked again and the
  app-support folder moves. *Recommendation:* change it to `io.github.owner.otaq` now, before release.
- **3d. GitHub repo and release URLs (0072).** Ask whether the repo is or will be renamed. Don't change URLs until the
  user confirms.
- **3e. Author name.** `LICENSE`, `Cargo.toml` `authors` and `plugin.json` `author.name` still carry a personal name. The
  user's standing preference is that no personal name appears, and decision 0071 already uses "The Crewboard Authors".
  *Recommendation:* use "The Otaq Authors" in all three, and as `© 2026 The Otaq Authors · MIT License` in the UI.

## 4. Steps
Do one step at a time. Run `scripts/check.sh` and commit after each one, with a message like
`Rename to Otaq: <step>`.

### Step 1: decisions and living docs
- Write **0076 "Product name: Otaq"**. It supersedes 0028; mark 0028 Superseded per the README rules. Include:
  - the 3a–3e answers;
  - a mapping table (Crewboard → Otaq, `crewboard` → `otaq`, `~/.crewboard` → `~/.otaq`, `CREWBOARD_` → `OTAQ_`,
    `crewboard/` → `otaq/`);
  - the logo: Command tent, from `crewboard.pen` frame `1. Otaq · Command tent`.

  Move the open question "Trademark check for Crewboard" to "Otaq".
- Write **0077 "Needs-you turquoise, In review magenta"**. It refines 0042 and the theme decisions. Rename the concept
  from "orange family" to the **needs-you family** so the name doesn't depend on a color (each palette shows it in its own
  teal). Record the per-palette choices from §5.
- Update the decision index, `CLAUDE.md`, `DESIGN.md`, `SDD.md`, `design-briefs/00`–`18` and `README`s to say Otaq and
  "needs-you"/turquoise instead of orange.
- **Don't rewrite history:**
  - leave old decisions' bodies alone (only status and changelog lines change);
  - leave `CHANGES-*.md`, `AUDIT-*`, `REVIEW-*`, `tickets/review-*.md` and `tickets/progress.md` entries as they are.

### Step 2: color tokens
- The token source is `crates/crewboard-view/src/theme.rs`: the `st_*` arrays near line 218 and the hex table near
  line 633. Replace the six rows below. The order is the existing one: Crewboard light, Crewboard dark, Frappé,
  Macchiato, Mocha, Tokyo Night, Dracula, Nord, Gruvbox dark, Rosé Pine, One Dark, Latte, Daylight, Solarized light,
  Rosé Pine Dawn.

```
("st-attn",           ["#087C71", "#2DD4BF", "#81c8be", "#8bd5ca", "#94e2d5", "#73daca", "#40e0d0", "#8fbcbb", "#8ec07c", "#9ccfd8", "#56b6c2", "#179299", "#1b7c83", "#2aa198", "#56949f"]),
("st-attn-bg",        ["#E0F5F2", "#0E2E2A", "#3F4F5C", "#374654", "#33414C", "#2A3D44", "#2C4B52", "#3F4C56", "#3A4337", "#363D4D", "#30454E", "#D9E8EC", "#E8F2F3", "#E8EEDC", "#EEF0EB"]),
("st-review",         ["#C03A8C", "#F07CC4", "#f4b8e4", "#f5bde6", "#f5c2e7", "#f77fd6", "#ff79c6", "#dc94bd", "#d3869b", "#ec9bd1", "#e77fc0", "#ea76cb", "#bf3989", "#d33682", "#b85fa8"]),
("st-review-bg",      ["#FBEAF4", "#3A1830", "#534C62", "#4A4259", "#453C4F", "#422D46", "#4F3850", "#4D4557", "#47393D", "#44344B", "#4A3B4D", "#EFE5F1", "#F9EBF3", "#F9E3D9", "#F8EBEC"]),
("st-investigate",    ["#7650D2", "#B196FF", "#babbf1", "#b7bdf8", "#b4befe", "#bb9af7", "#d6acff", "#b48ead", "#a89adb", "#c4a7e7", "#c678dd", "#8839ef", "#8250df", "#6c71c4", "#907aa9"]),
("st-investigate-bg", ["#F2EEFC", "#251C3B", "#494C65", "#3E425C", "#393B53", "#37324C", "#47415A", "#464454", "#3F3D48", "#3D364F", "#443A52", "#E5DFF4", "#F1ECFB", "#EDE7E0", "#F3ECEB"]),
```

  Keep both representations in sync (`0x…FF` u32 and `"#…"` strings). Verify against the pen file:
  `Print(GetVariables().variables["st-attn"])`. The theme axis value `crewboard` is the first palette there.
- Regenerate the stylesheet: `cargo test -p crewboard-desktop regenerate_stylesheet -- --ignored`. Never hand-edit
  `ui/src/theme/generated/*`. Also regenerate the other generated TS types if the generator covers them.
- Fix tests that assert the old hex values (`cards.test.ts`, `components.test.ts`, `inbox.test.ts`, view-model tests, …).
- Only the needs-you family changes. Two things stay as they are:
  - **Terminal content:** ANSI colors, and Claude Code's own orange `#D97757` in terminal output.
  - **Error red, approved green and paused amber.**
- Side effect to keep: anything already bound to `st-review` turns magenta, including syntax-string colors in the diff
  and terminal views and the "passed"/"Done" chips. The design accepts this. Mention it in the step report.
- Contrast: Otaq-light `#087C71` on `#E0F5F2` and `#C03A8C` on `#FBEAF4` both pass 4.5:1. Don't lighten them.

### Step 3: user-facing copy
- Every visible string: "Crewboard" → "Otaq". The theme picker's "Crewboard Light/Dark" becomes "Otaq Light/Dark". The
  About copyright follows 3e.
- Copy that says "orange" (tooltips, settings help such as "before the card turns orange", onboarding, notifications)
  becomes "turquoise", or color-free wording where that reads better.
- In code comments, test names and doc comments, replace "orange ticket/state/label" with "needs-you" wording where it
  refers to the concept.
- **Never edit files in `crates/crewboard-core/migrations/`.** `sqlx::migrate!` checksums them, so even a comment
  change breaks existing databases. Rename stored *values* only through a new migration, and only if 3a/3b need it.

### Step 4: identity rename (only what 3a–3e confirmed)
- Rename the crates and binaries, then fix the workspace `Cargo.toml`, the `use` paths, `package.json`,
  `tauri.conf.json` (`productName`, `identifier`, window titles) and the `scripts/`.
- Data folder and env var: implement the 3b migration in `crewboard-core/src/data_dir.rs` (or its renamed successor),
  with tests that use a temp HOME and never touch the real `~/.crewboard/`.
- Branch prefix in `naming.rs`. Update the tests that expect `crewboard/…`.
- Plugin: rename it. Every skill and hook must call the new binary name (look for `crewboard ` in `plugin/`).
- Persisted settings: if the stored theme id is `crewboard`, accept it as an alias of `otaq` when reading. Write `otaq`
  from now on.
- Afterwards, `git grep -il crewboard` may only list:
  - historical docs (§4 step 1);
  - `decisions/0028`;
  - migrations;
  - the 3b compatibility code and its tests.

  List any other hit in the step report.

### Step 5: logo and app icon
- **App icon source:** node `wLWnz` (`App Icon 96`) in `crewboard.pen`. It's a midnight-indigo glass squircle
  (`#2C3366` → `#0F1230`) with the white tent and a turquoise flag (`#8EF0E4` → `#14B8A6`).
  - Export it at 2× or more with the pen.dev `Export` call.
  - Build a 1024×1024 PNG following each platform's icon grid (macOS: an ~824 px body centred on a transparent canvas).
  - Run `cargo tauri icon <png>` (or the npm `tauri icon`) to regenerate `crates/…-desktop/icons/*`. Check the `.icns`,
    the `.ico` and the PNGs.
- **In-app mark** for the About screen (64 px, like `15 · Settings` → About), the title bar and wherever the A3 mark
  was. Search for the old mark first (`git grep -n "Moving card\|A3"` in `ui/src`). Make it one React component built
  from this SVG (viewBox 120):

```svg
<svg viewBox="0 0 120 120" fill="none" xmlns="http://www.w3.org/2000/svg">
  <path d="M18 106 V70 Q42 60 60 30 Q78 60 102 70 V106 Z" stroke="currentColor" stroke-width="6.5" stroke-linejoin="round" stroke-linecap="round"/>
  <path d="M46 62 V106 M74 62 V106 M60 30 V8" stroke="currentColor" stroke-width="4.6" stroke-linecap="round"/>
  <path d="M60 4 H80 Q83 4 81 7 L77 12 L81 17 Q83 20 80 20 H60 Z" fill="#14B8A6"/>
</svg>
```

  - Lines use `currentColor`: ink `#1A1B1E` on light, `#F4F5F7` on dark.
  - The flag is the fixed brand turquoise (`#0E9C8E` on light surfaces, `#14B8A6` on dark). It is **not** `--st-attn`,
    so the logo doesn't change with the palette.
  - Below ~24 px, keep strokes at least 1.1 px, which in practice means `vector-effect` or a thicker small-size variant.
- OS notifications use the bundle icon. Check one on macOS once step 4 has changed the identifier.

## 5. Per-palette notes for decision 0077
| Palette | Needs you | In review | Investigating |
| --- | --- | --- | --- |
| Otaq (light / dark) | `#087C71` / `#2DD4BF` | `#C03A8C` / `#F07CC4` | unchanged |
| Catppuccin (all four) | the palette's **teal** | the palette's **pink** | moved to **lavender** (Latte: **mauve**), because pink is now taken |
| Dracula | `#40e0d0` (off-palette turquoise; its cyan is already Working) | pink, unchanged | unchanged |
| Gruvbox dark | aqua | purple `#d3869b` | `#a89adb` (off-palette soft violet) |
| Tokyo Night, Nord, Rosé Pine, One Dark, Rosé Pine Dawn | the palette's teal or cyan | an added pink/magenta (off-palette) | unchanged |
| Daylight, Solarized light | the palette's teal or cyan | the palette's own magenta | unchanged |

## 6. Done when
- `scripts/check.sh` is green, including the UI tests.
- The app is hand-checked on macOS against:
  - `03 · Board` Light and Dark (turquoise cards and inbox, magenta In review);
  - `15 · Settings` About (new icon, "Otaq", copyright);
  - `11 · …` (a real OS notification shows the new icon);
  - the theme picker (shows "Otaq Light/Dark"; one dark and one light third-party palette checked).
- A first start with an existing `~/.crewboard/` migrates it once, and the board shows the same tickets.
- The final report lists:
  - each commit;
  - the 3a–3e answers;
  - the remaining `crewboard` hits and why each one stays;
  - the `st-review` side effects;
  - anything you deferred.
