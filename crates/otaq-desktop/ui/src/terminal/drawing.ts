// How every terminal draws its text (d0117 points 1 and 6): a contrast floor and the tools' character widths. The
// terminal component applies both to each xterm.js it makes, so the WebGL and the DOM renderer draw alike.
import { Unicode11Addon } from "@xterm/addon-unicode11";
import type { Terminal } from "@xterm/xterm";

/**
 * The contrast floor (d0117 point 1, WCAG AA as in d0096): a text colour too close to its background is shifted at draw
 * time, whoever chose it (the theme or the tool). The theme's own values stay as they are.
 */
export const MINIMUM_CONTRAST_RATIO = 4.5;

/**
 * Counts character widths as the tools do, by Unicode 11 (d0117 point 6). Called before anything is written, the replay
 * too: drawn at Unicode 6 widths, a wide symbol puts the terminal and the tool a column apart and leaves stray
 * characters on screen.
 */
export function countWidthsAsUnicode11(term: Pick<Terminal, "loadAddon" | "unicode">) {
  term.loadAddon(new Unicode11Addon());
  term.unicode.activeVersion = "11";
}
