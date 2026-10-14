# 0032. The `otaq` CLI follows beads (`bd`) conventions

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0003, 0019, 0023, 0029, 0030, 0031

## Context
The user asked to use the **beads `bd` CLI** as the reference for the agent-facing CLI (0031). `bd` is built
for coding agents and is widely used with Claude Code. Reference: github.com/steveyegge/beads
(`docs/CLI_REFERENCE.md`, read 2026-10-09).

## Conventions adopted from `bd`
| `bd` convention | Otaq |
|---|---|
| **Hybrid naming:** bare verbs for core ticket operations (`bd create/show/list/update/ready`), noun-verb for subsystems (`bd dep add`) | Same split (see below) |
| **Aliases** (`new`→create, `view`→show) | `new`, `view` |
| **Global `--json`** on every command, `-q/--quiet`, and `--silent` (print only the ID) | Same. JSON errors as `{"error": {...}}` with a non-zero exit |
| **Long text:** `-d` inline for short text, `--body-file <file>` (`-` = stdin), and the alias `--stdin` | Same. Skills always use `--stdin` for multi-line text (keeps the 0031 rule) |
| **Omitted ID → the current issue** (`--current`) | Omitted KEY → `$OTAQ_TICKET` |
| **Partial IDs** | A bare number (`18`) resolves within the current project |
| **`bd prime`**: AI-optimized workflow context | `otaq prime`: the current ticket, parents and their summaries, review feedback, pending questions, the rules, and the
  user's **Agent instructions** (0040). Skills call it first |
| **`bd ready` / `bd blocked`** | `otaq ready` / `otaq blocked` (dependency state per project) |
| **`bd dep add <issue> <depends-on>`, `dep remove`, `dep tree`, `dep cycles`** | Same shape. `dep tree` prints the Stack list (0006) |
| **`--actor`** (default `$BEADS_ACTOR`, then git user, then `$USER`) | `--actor` defaults to `agent:<KEY>` when `$OTAQ_TICKET` is set, otherwise to the OS user. It drives "updated by agent" in the UI (0019) |
| **`bd create -f file.md`** (batch create from markdown) | `otaq create -f plan.md`, useful for the user's planning skills (0019) |

## Command set (replaces the table in 0031)
**Core (bare verbs):**
- `otaq show [KEY]` (alias `view`): ticket details. `--long` adds messages and history
- `otaq list`: `--status`, `--project`, `--tree`
- `otaq ready` / `otaq blocked`
- `otaq create "Title"` (alias `new`): `-d`, ~~`--done-when <item>` (repeatable)~~ (removed, 0067), `-p 1–4` (0065), `--deps SHOP-18,SHOP-19`,
  `--stdin` (markdown body), `-f <file>` (batch), `--silent`. New tickets land in Backlog (0019)
- `otaq update [KEY]`: `--title`, `-d`, ~~`--add-done-when`~~ (removed, 0067), `-p 1–4` (0065), `--stdin`
- `otaq prime`: context for the current ticket's agent; `prime --validate` for the validator (0063), `prime --project`
  for a Project agent (0068)
- `otaq ask <KEY|human> --stdin`: `human` replaces `ask_human` (the ticket shows "Asks you" at the top of Dev, 0037).
  `<KEY>` asks a directly linked ticket (0030). `ask human` takes optional answer options:
  `--option "<label>"` (repeatable, 2–4) and `--recommended N` (0061); `ask <KEY>` stays free text
- `otaq answer <QUESTION-ID> --stdin`, and `otaq questions` (pending questions for this ticket)

**Subsystems (noun-verb):**
- `otaq review submit [KEY] --stdin`: the summary. Starts validation, or moves the ticket to Ready for Review when
  validation is off (0063) (replaces `ready_for_review`)
- `otaq validate pass|fail --stdin`: the validator's verdict and report (0063)
- `otaq agent name "<name>"`: a Project agent names itself (0068)
- `otaq review feedback [KEY]`: in-app comments plus unresolved PR comments (0017)
- `otaq dep add <KEY> <DEPENDS-ON>` / `dep remove` / `dep tree [KEY]` / `dep cycles`
- `otaq project list | show`
- `otaq pr propose --stdin`, `otaq pr opened <url>`, `otaq branch rename <name>` (0050)
- `otaq attach <KEY>` (0022), `otaq daemon start | stop | status`, `otaq hook <event>` (internal)

**Global flags:** `--json`, `-q/--quiet`, `-P/--project <KEY>`, `--actor`.

## Deliberately NOT adopted
- **Hash IDs** (`bd-a1b2`). They solve merge collisions between distributed writers. Otaq's service
  allocates numbers centrally, so readable `SHOP-18` keys stay (0029).
- **`--claim` / `--claim-next`.** In Otaq, the user starts work by dragging. Agents never pick up work
  themselves (0018).
- ~~**Priorities.** Not in the ticket form (0019).~~ *Adopted by 0065: priority 1–4, `-p`.* ~~Issue types~~: **epics were adopted in 0039**, so the types are ticket and
  epic, with `--type epic`, `--parent` and hierarchical child keys (SHOP-40.1).
- **`bd init` writing `AGENTS.md`, `.beads/` storage, and Dolt sync.** Otaq never writes into repos (0023),
  and its storage is the app's SQLite (0003).
- **Gates and molecules.** Out of scope.

## Changelog
- 2026-10-09: created
- 2026-10-09: removed `otaq investigation post` (0036).
- 2026-10-09: adopted epics, `--parent` and hierarchical child keys (0039).
- 2026-10-09: added `otaq pr propose`, `otaq pr opened`, `otaq branch rename` (0050).
- 2026-10-09: `ask human` takes `--option` / `--recommended` (0061); the `pr` and `branch rename` commands listed under
  Subsystems (gap review).
- 2026-10-11: `--done-when` / `--add-done-when` removed (0067); `-p` priorities adopted (0065); `validate pass|fail`, `prime --validate` (0063); `agent name`, `prime --project` (0068).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
