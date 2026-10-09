# 0008. Interactive Claude Code CLI in tmux, embedded terminal

- **Status:** Superseded by 0022
- **Date:** 2026-10-08
- **Related:** 0001, 0009

## Context
The user wants control and visibility. They want to watch each agent and talk to it, rather than
fully automating everything at this stage.

## Decision
- Each ticket gets a **tmux session** (`factory-<ticket>`) running the real interactive `claude` CLI,
  with the ticket's worktree as its working directory.
- Clicking a card opens a side panel (**Terminal · Diff · Preview · Ticket · Runs**). The Terminal tab
  is a live, interactive terminal embedded in the app (`alacritty_terminal`, rendered in GPUI).
- tmux keeps sessions alive across app restarts and lets the user `tmux attach -t factory-12` from any
  terminal.

## Alternatives considered
- Running the agent headless through the Agent SDK or `claude -p`. Rejected for now because it
  hides the agent and the user can't talk to it.

## Changelog
- 2026-10-08: created
- 2026-10-09: superseded by 0022. tmux was replaced by the factory's own background service, so Windows can be supported.
