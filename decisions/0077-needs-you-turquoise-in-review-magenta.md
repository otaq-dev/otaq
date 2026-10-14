# 0077. Needs-you turquoise, In review magenta

- **Status:** Accepted
- **Date:** 2026-10-15
- **Related:** 0016, 0042 (refined), 0069, 0070, 0076

## Context
0042 grouped Asks you, Needs you and Idle into one "orange family" (token `st-attn`). With the new name and logo (0076),
the flag the board raises when a ticket needs you is turquoise, so the needs-you color follows it. Turquoise was already
taken by **In review** (`st-review`, teal), so In review needs a new color, and in two palettes moving In review to pink took
**Investigating**'s color (`st-investigate`).

## Decision
- **The concept is renamed to the needs-you family**, so its name no longer depends on a color. In older decisions and
  docs, "orange" (orange family, orange ticket, turns orange) means this family. Its behavior (0042, 0016, 0047) doesn't
  change.
- **Needs you is turquoise** (`st-attn`, `st-attn-bg`): each palette shows it in its own teal or cyan.
- **In review is magenta** (`st-review`, `st-review-bg`): each palette's pink or magenta.
- **Investigating** (`st-investigate`) moves only where its color is now In review's.
- Nothing else changes: error red, approved green and paused amber stay, and so do the terminal contents (ANSI colors,
  Claude Code's own orange `#D97757`).

Per palette (values are the `crewboard.pen` variables and are copied as written, 0069):

| Palette | Needs you | In review | Investigating |
|---|---|---|---|
| Otaq Light / Dark | `#087C71` / `#2DD4BF` | `#C03A8C` / `#F07CC4` | unchanged |
| Catppuccin Frappé, Macchiato, Mocha | the palette's teal | the palette's pink | lavender (pink is taken) |
| Catppuccin Latte | teal | pink | mauve |
| Dracula | `#40e0d0`, an off-palette turquoise (its cyan is already Working) | pink, unchanged | unchanged |
| Gruvbox dark | aqua | purple `#d3869b` | `#a89adb`, an off-palette soft violet |
| Tokyo Night, Nord, Rosé Pine, One Dark, Rosé Pine Dawn | the palette's teal or cyan | an added (off-palette) pink or magenta | unchanged |
| Daylight, Solarized light | the palette's teal or cyan | the palette's own magenta | unchanged |

- **Contrast:** Otaq Light's `#087C71` on `#E0F5F2` and `#C03A8C` on `#FBEAF4` both pass 4.5:1.
- **Side effect, accepted:** everything already drawn with `st-review` turns magenta with it, including the string color in
  the diff and terminal syntax highlighting and the "passed" / "Done" chips.
- The logo's flag is a fixed brand turquoise, not `st-attn` (0076).

## Alternatives considered
- **Keep orange:** it would clash with the new mark, whose flag is the "needs you" signal.
- **Turquoise for needs you, but keep In review teal:** the two states would be hard to tell apart on a card.

## Consequences
- The token table in the theme source, the generated stylesheet and the tests that pin the old values change together.
- Copy that says "orange" ("before the card turns orange") says turquoise or uses color-free wording.

## Changelog
- 2026-10-15: created (design update with the rename to Otaq, user decision)
