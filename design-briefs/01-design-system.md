# 01: Design system foundations

Read `00-README.md` first. Every other brief reuses what you define here, so design this one first.

## Goal
A calm, dense, developer-grade visual language for a native app that one person watches all day while
5–10 agents work. It should feel closer to a code editor (Zed, Linear, a terminal) than to a consumer app.

## Design
1. **Color tokens**, light and dark: background layers (app, panel, card, raised), borders, text
   (primary, secondary, muted), accent, focus ring, selection.
   **Themes (0069):** besides Crewboard Light and Dark, 13 bundled themes fill the same tokens plus a 16-color terminal
   palette (frame *01 · Theme palettes*): Catppuccin Frappé, Macchiato, Mocha and Latte, Tokyo Night, Dracula, Nord,
   Gruvbox Dark, Rosé Pine, Rosé Pine Dawn, One Dark, Daylight and Solarized Light. A new theme adds a column of values
   for every token there, never a new token.
2. **Status colors**: one consistent color plus icon pair for each column and status in the README vocabulary.
   The most important split:
   - **Needs the user** (Needs you, Asks you, Interrupted, Start interrupted, Parent closed) must stand out.
   - **Working normally** (Agent Working, Queued) must stay calm.
   - **Waiting on the user's review** (Ready for Review, PR open) is in between.
   - **Done** fades back.

   Color must never be the only signal: always pair it with an icon or label (accessibility, color blindness).
3. **Typography**: a UI sans-serif and a monospace (suggest open-license fonts to bundle), with a type scale
   for header, column title, card title, metadata, code and terminal.
4. **Spacing and density**: a grid and spacing scale. The board must fit about 10 columns at 1920 px and stay
   usable at 1280 px.
5. **Iconography**: a set covering agent working, needs you, interrupted, dependency,
   branch, PR, local merge, run, shell, port, tokens, terminal, diff, inbox, settings, project.
6. **Core components**, each with all their states (default, hover, pressed, focused, disabled, loading
   where relevant):
   - Buttons (primary, secondary, destructive, ghost) and icon buttons
   - Badges and pills (status, count, dependency `⛓ SHOP-18`, "last signal 40 s")
   - Tabs, segmented control (Unified / Side-by-side)
   - Text input, multi-line input, checklist item, dropdown, toggle
   - Tooltip, context menu, confirm dialog, toast, banner
   - Keyboard shortcut hint chips (show both ⌘ on macOS and Ctrl on Windows/Linux)
7. **Keyboard map** (show ⌘ and Ctrl+Shift forms; Ctrl+letter belongs to the terminal on Windows and Linux): propose shortcuts for next inbox item, open or close the side panel, switch tabs,
   switch project, new ticket, approve, send back and search.

## Constraints
- Custom look in HTML/CSS (Tauri, decision 0074): flat fills, borders, radius, simple shadows. No blur and no heavy gradients.
- The same look on macOS, Windows and Linux.

## Deliverables
A token sheet (light and dark), a status legend, a type scale, a component sheet with states, and a
keyboard map.
