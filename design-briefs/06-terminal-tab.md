# 06: Terminal tab (live Claude Code session)

Read `00-README.md` and `01-design-system.md` first.

## Context
Each ticket runs the **real interactive `claude` CLI** in its own session, owned by Crewboard's background
service (0008, 0022). The Terminal tab shows that live session, and **the user can type into it at any time**.
The same session can also be opened from any external terminal with `crewboard attach SHOP-18`. Crewboard
never changes how the Claude CLI behaves (0018).

## Design
1. **The terminal view** inside the side panel, and in expanded or full-window mode:
   - Realistic Claude Code output: the agent reading files, a diff summary, a permission prompt, the user typing
   - Monospace grid, ANSI colors, cursor, selection, scrollback, find in scrollback
2. **Terminal header strip:**
   - Session state (live, waiting for you, ended)
   - Ticket worktree path, and the agent's `$PORT` (the Run tab shows the separate Run port, 0026)
   - "Copy attach command" (`crewboard attach SHOP-18`)
   - Font size
3. **Focus handling:** it must be clear when keystrokes go to the agent vs. to the app (focus ring, plus a hint).
   While the terminal has focus, **every key goes to Claude Code, Esc included** (Claude uses Esc to stop the agent and
   Esc Esc to rewind). Leave with **⌘⇧`** (Ctrl+Shift+` on Windows and Linux), a click outside, or the inbox shortcut.
   On Windows and Linux, app shortcuts use Ctrl+Shift while the terminal is focused.
4. **The permission prompt moment** (SHOP-22): the Claude CLI asks "Allow `rm -rf .next`?". The panel should draw
   attention to the prompt line without covering the real CLI output.
5. **Answering a question** (SHOP-23): the user types the answer directly into the terminal.

## States
- Live and working (output streaming)
- Waiting for the user (permission prompt, agent question via `crewboard ask human`)
- Reconnecting to the background service (scrollback still visible, input disabled)
- Session ended normally
- **Interrupted** (session lost after a reboot), with a Resume call to action (0018)
- Very long output and a scrolled-up view, with a "jump to latest" control
- A narrow panel vs. full window
- **Message waiting** (0048): an agent message is held because the user has unsent text in the
  input line. A "1 message waiting · Deliver now" chip on the card and in the terminal header strip.
- **Unsent-text prompt** (0048): the user clicks Send back or answers from the inbox while the input line has unsent
  text. "SHOP-20 has unsent text in its terminal" with **Keep it and cancel** / **Clear it and send**.

## Constraints
- Must work as a real terminal grid. No chat-bubble re-rendering of Claude's output.
