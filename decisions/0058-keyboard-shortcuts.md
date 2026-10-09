# 0058. Keyboard shortcuts: the full map, with no duplicates across columns

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0001, 0016, 0019, 0022

## Context
The keyboard map in `crewboard.pen` (01 · Components → Keyboard map) has three columns: macOS, Windows/Linux, and "in
terminal" (Windows/Linux move app shortcuts to Ctrl+Shift while the terminal has focus, because Ctrl+letter belongs to the
CLI). Two shortcuts collided on Windows/Linux: Ctrl+Shift+P (switch project vs. Project terminal) and Ctrl+Shift+Enter
(Approve vs. Expand panel).

## Decision
**Rule:** no shortcut may appear twice across the three columns, and OS system shortcuts (for example ⌘` on macOS) are off-limits.

| Action | macOS | Windows / Linux | In terminal (Win/Linux) |
|---|---|---|---|
| Next inbox item (terminal focused) | ⌘J | Ctrl+J | Ctrl+Shift+J |
| Open inbox | ⇧⌘I | Ctrl+Shift+I | Ctrl+Shift+I |
| Previous / next ticket in panel | ⌘[ / ⌘] | Ctrl+[ / Ctrl+] | Ctrl+Shift+[ / ] |
| Leave terminal focus | ⇧⌘` | Ctrl+Shift+` | Ctrl+Shift+` |
| Search / filter | ⌘K | Ctrl+K | Ctrl+Shift+K |
| Switch project | ⌘P | Ctrl+P | Ctrl+Shift+P |
| Project 1–9 | ⌘1–9 | Ctrl+1–9 | Ctrl+Shift+1–9 |
| New ticket | ⌘N | Ctrl+N | Ctrl+Shift+N |
| **Project terminal** | **⇧⌘T** | **Ctrl+Shift+T** | **Ctrl+Shift+T** |
| Settings | ⌘, | Ctrl+, | Ctrl+Shift+, |
| Open / close panel | ↵ / Esc | ↵ / Esc | — |
| **Expand panel** | **⇧⌘E** | **Ctrl+Shift+E** | **Ctrl+Shift+E** |
| Switch tab | ⌘⌥1–6 | Ctrl+Alt+1–6 | Ctrl+Shift+Alt+1–6 |
| Approve ▾ | ⌘↵ | Ctrl+↵ | Ctrl+Shift+↵ |
| Send back | ⌘⌫ | Ctrl+⌫ | Ctrl+Shift+⌫ |
| Start / move to Dev | ⇧⌘D | Ctrl+Shift+D | Ctrl+Shift+D |

- While the terminal has focus, every other key goes to the CLI (Esc, Ctrl+C, Ctrl+J, Ctrl+T…). On macOS, app shortcuts keep
  working because the terminal doesn't use ⌘.
- Find in terminal scrollback (brief 06) uses ⌘F / Ctrl+Shift+F.

## Changelog
- 2026-10-09: created (SDD preparation, user decision): Project terminal moved to ⇧⌘T / Ctrl+Shift+T, Expand panel to ⇧⌘E / Ctrl+Shift+E.
