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
| **Terminals & agents** (last-used Project terminal; 0068) | **⇧⌘T** | **Ctrl+Shift+T** | **Ctrl+Shift+T** |
| **New Project agent** (0068) | **⇧⌘A** | **Ctrl+Shift+A** | **Ctrl+Shift+A** |
| Settings | ⌘, | Ctrl+, | Ctrl+Shift+, |
| Open / close panel | ↵ / Esc | ↵ / Esc | — |
| **Expand panel** | **⇧⌘E** | **Ctrl+Shift+E** | **Ctrl+Shift+E** |
| Switch tab | ⌘⌥1–6 | Ctrl+Alt+1–6 | Ctrl+Shift+Alt+1–6 |
| Approve ▾ | ⌘↵ | Ctrl+↵ | Ctrl+Shift+↵ |
| Send back | ⌘⌫ | Ctrl+⌫ | Ctrl+Shift+⌫ |
| Start / move to Dev | ⇧⌘D | Ctrl+Shift+D | Ctrl+Shift+D |
| **Focus the epic of the selected card** (0080) | **⇧⌘G** | **Ctrl+Shift+G** | **Ctrl+Shift+G** |

- While the terminal has focus, every other key goes to the CLI (Esc, Ctrl+C, Ctrl+J, Ctrl+T…). On macOS, app shortcuts keep
  working because the terminal doesn't use ⌘.
- Find in terminal scrollback (brief 06) uses ⌘F / Ctrl+Shift+F.
- **Inside the description editor** (0078), ⌘K / Ctrl+K inserts a link while the editor has focus. Everywhere else ⌘K stays
  Search / filter. This is the one shortcut whose meaning depends on focus, the way the terminal column already does.

## Changelog
- 2026-10-09: created (SDD preparation, user decision): Project terminal moved to ⇧⌘T / Ctrl+Shift+T, Expand panel to ⇧⌘E / Ctrl+Shift+E.
- 2026-10-11: "Project terminal" row renamed "Terminals & agents" (opens the drawer on the last-used terminal, or a new one); ⇧⌘A / Ctrl+Shift+A opens a new Project agent (design audit K, user decision).
- 2026-10-15: ⌘K / Ctrl+K inserts a link while the description editor has focus; Search / filter everywhere else (0078, user decision).
- 2026-10-15: ⇧⌘G / Ctrl+Shift+G focuses the epic of the selected card; Start / move to Dev also queues an epic (0080, user decision).
