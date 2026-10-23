// The active theme (UIT-07, d0070): the page's root element carries `data-theme="<key>"`, which picks one block of
// the generated `tokens.css`, so a new theme recolors everything at once without a reload. The theme comes from the
// Rust side (`theme.active`) and is asked again on every `settings_changed` event, so a pick in Settings or in
// another window lands here too. The frame mounts it with one line: `useTheme()`.
import { useLayoutEffect, useState } from "react";

import type { ActiveTheme } from "../generated/ActiveTheme";
import type { ThemeParams } from "../generated/ThemeParams";
import { type PageEvents, pageEvents, watchView } from "../service";
import { request } from "../tauri";
import { FIRST_LAUNCH } from "./generated/tokens";

/** Whether the OS draws windows dark now. Only used until a theme is stored (the first launch). */
export function osIsDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

/** Follows the active theme: once the event stream is open, then on each settings change. Returns the stop. */
export function watchTheme(
  onChange: (theme: ActiveTheme) => void,
  osDark: () => boolean,
  events: PageEvents = pageEvents,
): () => void {
  const load = () => {
    const params: ThemeParams = { os_dark: osDark() };
    return request("theme.active", params) as Promise<ActiveTheme>;
  };
  return watchView(
    load,
    ["settings_changed"],
    (state) => {
      if (state.view) onChange(state.view);
    },
    events,
  );
}

/**
 * Keeps `root`'s `data-theme` on the active theme, which picks its block of `tokens.css`: the page recolors at once,
 * without a reload. Until the first answer, the first-launch theme for the OS. Returns the stop.
 */
export function followTheme(
  root: Pick<HTMLElement, "dataset">,
  onChange: (theme: ActiveTheme) => void,
  osDark: () => boolean,
  events: PageEvents = pageEvents,
): () => void {
  root.dataset.theme ??= osDark() ? FIRST_LAUNCH.dark : FIRST_LAUNCH.light;
  return watchTheme(
    (active) => {
      root.dataset.theme = active.key;
      onChange(active);
    },
    osDark,
    events,
  );
}

/**
 * Applies the active theme to the page's root element and returns it (`undefined` until it is known). Until then the
 * page shows the first-launch theme for the OS, so the first frame isn't unstyled.
 */
export function useTheme(): ActiveTheme | undefined {
  const [theme, setTheme] = useState<ActiveTheme>();
  // Before the first paint, so the first frame isn't unstyled.
  useLayoutEffect(() => followTheme(document.documentElement, setTheme, osIsDark), []);
  return theme;
}

/** The terminal colours of the theme in use, named as xterm.js names them (its `ITheme`). */
export interface TerminalColors {
  background: string;
  foreground: string;
  cursor: string;
  selectionBackground: string;
  selectionInactiveBackground: string;
  selectionForeground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

/**
 * Reads a terminal's colours from the CSS variables (`--term-bg`, `--ansi-0` … `--ansi-15`), e.g. from
 * `getComputedStyle(element)`. A terminal reads them again when the theme changes (`useTheme`'s value).
 */
export function terminalColors(style: Pick<CSSStyleDeclaration, "getPropertyValue">): TerminalColors {
  const read = (name: string) => style.getPropertyValue(`--${name}`).trim();
  const ansi = (n: number) => read(`ansi-${String(n)}`);
  return {
    background: read("term-bg"),
    foreground: read("term-fg"),
    cursor: read("term-fg"),
    // Selected text keeps the terminal's text colour on the same tint, focused or not (d0117 point 2).
    selectionBackground: read("selection"),
    selectionInactiveBackground: read("selection"),
    selectionForeground: read("term-fg"),
    black: ansi(0),
    red: ansi(1),
    green: ansi(2),
    yellow: ansi(3),
    blue: ansi(4),
    magenta: ansi(5),
    cyan: ansi(6),
    white: ansi(7),
    brightBlack: ansi(8),
    brightRed: ansi(9),
    brightGreen: ansi(10),
    brightYellow: ansi(11),
    brightBlue: ansi(12),
    brightMagenta: ansi(13),
    brightCyan: ansi(14),
    brightWhite: ansi(15),
  };
}
