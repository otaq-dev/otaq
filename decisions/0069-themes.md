# 0069. Themes: a dark and a light theme chosen from 15 bundled ones, picked in onboarding and Settings

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0021, 0023, 0053, 0054, 0057

## Context
Until now Crewboard had one look in two versions, Crewboard Light and Crewboard Dark, and brief 15 listed "light, dark or
system" only as a proposal. On 2026-10-11 the user had the pen agent add popular editor themes to `crewboard.pen` (0057):
the frame *01 · Theme palettes*, a new **Theme** step in *14 · First run & onboarding*, and a theme picker plus a **Theme
credits** dialog in *15 · Settings*. This decision records what the design shows, so the app can be built from it.

## Decision
**Mode plus two themes.** The user picks a **mode** (Light, Dark or System; System is the default) and **two themes**: one
used when the window is dark and one used when it is light. For example System + Tokyo Night + Daylight means Tokyo Night
while macOS is in dark mode and Daylight while it is light. In Dark mode only the dark theme is used; in Light mode only the
light theme is used.

**The bundled themes** (design palette key in brackets). Crewboard is the only theme with both versions; every other theme
is either dark or light.

| Dark | Light |
|---|---|
| **Crewboard Dark** (`crewboard`, default) | **Crewboard Light** (`crewboard`, default) |
| Catppuccin Frappé (`catppuccin-frappe`) | Catppuccin Latte (`catppuccin-latte`) |
| Catppuccin Macchiato (`catppuccin-macchiato`) | Daylight (`daylight`) |
| Catppuccin Mocha (`catppuccin-mocha`) | Solarized Light (`solarized-light`) |
| Tokyo Night (`tokyo-night`) | Rosé Pine Dawn (`rose-pine-dawn`) |
| Dracula (`dracula`) | |
| Nord (`nord`) | |
| Gruvbox Dark (`gruvbox-dark`) | |
| Rosé Pine (`rose-pine`) | |
| One Dark (`one-dark`) | |

**Colors come from the design, exactly.** Every theme fills the same 39 color tokens (backgrounds, borders, text, accent,
focus, selection, the status colors and their tints, terminal and diff backgrounds) plus its own **16 terminal colors**
(`ansi-0` … `ansi-15`). The values are the variables of `crewboard.pen` and are copied as written; the derived ones (status
tints, accent hover and soft, selection, diff backgrounds) are already worked out in the design, so the app mixes nothing at
run time. A test pins every value to the design, as for Crewboard today. Status icons and labels don't change: color is still
never the only signal.

**What recolors.** The whole window: board, cards, side panel, dialogs, inbox, Settings, onboarding, the ticket and Project
terminals (background, text and the 16 colors), and the Diff tab including syntax highlighting. A program in a terminal that
sets its own colors still wins, as today.

**Where it is picked.**
- **Onboarding** gets a step: Welcome → **Theme** → Tools → Project → Done. The Theme step ("Pick a look") shows the mode
  switch and two rows of theme cards (dark, light), each a small preview with a one-line tagline such as "Neon city after
  dark". The window recolors as soon as a card or mode is clicked. The step says the choice can be changed anytime in
  Settings → Appearance.
- **Settings → Appearance** has the same mode switch and picker. In System mode, the theme row that is currently in use
  carries a tag such as "In use · OS is light".
- Every click is applied and saved at once; there is no Save button.

**Credits.** Settings → Appearance has a **Theme credits** row ("Bundled themes are based on open-source palettes, used
under the MIT License. Copyright stays with their authors.") that opens a dialog. The dialog lists each source project with
the themes taken from it, its copyright line, its license and a link to its source: Catppuccin, Tokyo Night, Dracula, Nord,
Gruvbox, Rosé Pine, Atom One Dark, Primer Primitives (Daylight) and Solarized. It ends with the full MIT text and says
Crewboard adapted some colors and isn't affiliated with or endorsed by these projects. This is the notice those licenses
ask Crewboard to ship.

**Storage.** The mode and the two theme names are app settings (0023), next to today's `appearance`. A stored theme name
the app doesn't know (say, one removed in a later version) falls back to Crewboard Dark or Crewboard Light. People who
already finished onboarding don't see the new step; they keep their mode with the Crewboard themes until they change them
in Settings.

**Not included:** custom or imported theme files, a per-project theme, and themes for the app's fonts. Each could come
later as its own decision.

## Alternatives considered
- **One theme setting instead of a dark one and a light one:** System mode would then have nothing to switch to, or would
  need every theme to come in both versions. Most of these themes exist in only one.
- **Store a few base colors per theme and compute the rest in the app:** the design already works out every value, and
  copying them exactly keeps the app and the design in step through the same kind of test Crewboard already has.
- **Credits only in a file in the repository:** the licenses ask for the notice to ship with the app, and a dialog people
  can open is where they would look for it.

## Consequences
- Built as milestone **M13 · Themes** (0053), part of v0.1. It touches nothing M12 changes, so it may be built before M12.
- `theme.rs` holds 15 sets of 39 tokens plus 16 terminal colors instead of two sets, all pinned to the design by a test.
- The terminal colors move from `terminal/colors.rs` into the themes. Crewboard's own terminal colors change slightly to the
  design's values (for example dark black is `#1C1E23` in the design, `#3A3E47` in the app before this).
- A few badges that hard-code white text (the needs-you counters, the selected project's icon) must use the theme's
  "text on accent" color, because themes with light accents (Catppuccin, Dracula) set it to a dark color.
- Adding a theme later means: the design first (*01 · Theme palettes*, the pickers and the credits), then copy its values.

## Changelog
- 2026-10-11: created from the pen agent's theme update to `crewboard.pen` (user decision)
- 2026-10-11: milestone M13, may be built before M12 (user decision)
