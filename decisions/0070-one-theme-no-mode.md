# 0070. One theme, no Light / Dark / System mode

- **Status:** Accepted
- **Date:** 2026-10-11
- **Related:** 0023, 0069

## Context
0069 had people pick a **mode** (Light, Dark or System) and then **two themes**, a dark one and a light one. Once it was
built (M13), choosing a mode and then a theme confused people: with 15 themes, the theme already says whether the window
is dark or light, so asking about it first adds a step without adding a choice.

## Decision
**One setting: the theme.** People pick one of the 15 bundled themes (0069's list stays the same, with Crewboard Dark and
Crewboard Light as two separate cards), and that theme is used all the time. There is no Light / Dark / System switch, and
Crewboard no longer follows macOS, Windows or Linux switching between light and dark.

**The picker.** The onboarding Theme step ("Pick a look") and Settings → Appearance show the same 15 cards. They stay
grouped under the headings "Dark" and "Light" so people can find what they want, but the headings are only labels, not a
selection. The picked card has the check mark; the "In use · OS is light" tag goes away. Clicking a card recolors the window
and saves at once, as before.

**Default.** On the very first launch the app starts in Crewboard Dark or Crewboard Light, whichever matches the OS at that
moment, and the onboarding Theme step shows that card as picked. From then on it is a fixed theme: if the OS later switches,
Crewboard stays as it is.

**Storage.** One app setting (0023), the theme name, replaces the mode and the two theme names. People who already picked
something keep what they see today: Dark mode keeps their dark theme, Light mode keeps their light theme, and System mode
keeps whichever of the two the OS was showing when the update first runs. A stored theme name the app doesn't know is
treated like a first launch: Crewboard Dark or Crewboard Light, as the OS is at that moment, and then stored.

## Alternatives considered
- **Keep 0069 (mode plus two themes):** it lets the window follow the OS at sunset, but the mode step confused people, and
  most people pick one theme and keep it.
- **One theme plus a "Match the OS light/dark" checkbox that reveals a second theme:** keeps OS-following for the few who
  want it, but brings back the same two-step choice in another form.

## Consequences
- Replaces 0069's **Mode plus two themes** and the mode parts of **Where it is picked** and **Storage**. Everything else in
  0069 stays: the 15 themes, colors copied from the design, what recolors, the Theme credits dialog, and what is not included.
- The design (`crewboard.pen`, 0057) changes first: *14 · First run & onboarding → 2 · Theme* and *15 · Settings →
  Appearance* lose the mode switch and the "In use" tag.
- The app (built in M13) changes with it: core settings (one `theme` instead of `appearance`, `dark_theme`,
  `light_theme`, with the move above), the theme resolution in the app (no OS lookup after the first launch), and the
  picker, onboarding step and Settings row. Done on the GPUI app as the change it needs to keep compiling (0074), and
  matched to the updated design.
- Following the OS could come back later as its own decision if people ask for it.

## Changelog
- 2026-10-11: created; replaces the mode and the two themes of 0069 (user decision)
- 2026-10-11: built in core and the GPUI app (user asked for it). While building, an unknown stored name became "treated
  like a first launch" instead of always Crewboard Dark, so it reuses the first-launch path
- 2026-10-15: the defaults are Otaq Dark / Otaq Light (0076).
