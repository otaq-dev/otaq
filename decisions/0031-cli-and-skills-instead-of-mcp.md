# 0031. Agents use the `otaq` CLI through skills, with no MCP server

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009 (MCP part superseded), 0016, 0019, 0022, 0027, 0029, 0030
- **Supersedes:** the MCP server part of 0009

## Context
0009 gave agents Otaq's tools through an MCP server (`otaq mcp`). The `otaq` binary already
exists for `daemon`, `attach` and `hook`, so the MCP server is one more component to build and maintain, and
its tool definitions sit in every session's context. The user prefers low maintenance.

## Decision
- **No MCP server.** Agents call **`otaq` CLI subcommands**, and the plugin's **skills** teach them when and how.
- Commands talk to the background service over its user-locked socket (0022, 0027). They default to the
  ticket in `$OTAQ_TICKET`. Otaq puts the `otaq` binary on the session's `PATH` when it launches
  `claude`.
- **Long text always goes through stdin** (summaries, questions, answers, ticket bodies), never through
  quoted arguments, which avoids shell-escaping problems. Output is human-readable by default, with `--json`.
- Command set (replaces the former MCP tool names). **Refined in 0032 (beads-style naming). The table there is current.**

| Former MCP tool | CLI command |
|---|---|
| `get_ticket` | `otaq ticket show [KEY]` |
| `get_review_feedback` | `otaq review feedback` (in-app comments plus unresolved PR comments, 0017) |
| `ready_for_review(summary)` | `otaq ready-for-review` (summary on stdin) |
| `ask_human(question)` | `otaq ask-human` (question on stdin) |
| `create_ticket` | `otaq ticket create` (body on stdin) |
| `link_tickets` | `otaq ticket link <KEY> --depends-on <KEY>` |
| `update_ticket` | `otaq ticket update [KEY]` (fields on stdin) |
| `post_investigation` | ~~`otaq investigation post`~~ (removed, 0036) |
| `ask_ticket(key, q)` | `otaq ask <KEY>` (question on stdin; linked tickets only, 0030) |
| `answer_question(id, a)` | `otaq answer <QUESTION-ID>` (stdin) |

- The service still enforces all the rules (dependency-only asking, bottom-up, cycle rejection). The CLI is
  only a thin client.
- Permissions follow the user's own Claude Code mode (0011), except that Otaq's own commands are pre-approved at
  launch with `--allowedTools "Bash(otaq:*)"` (0011).
- The same commands are usable by the user and by scripts.

## What an agent may change (grilling session)
Agents run `otaq` without permission prompts (0011), so the service limits what an **agent caller** may do:
- **Read anything** in the project (`show`, `list`, `ready`, `blocked`, `dep tree`, `prime`).
- **Change only its own ticket** (`update`, `review submit`, `ask`, `answer`, `pr propose`, `pr opened`, `branch rename`).
- **Create tickets in Backlog** (including `--parent` for epics), and add dependencies only from its own ticket or between
  tickets it created.
- Everything else is refused: "SHOP-18's agent can't change SHOP-12".
- The service decides who is calling from **where the process runs** (inside one of its agent sessions, by process
  ancestry), not only from `$OTAQ_TICKET`, so unsetting the variable doesn't grant full access.
- The Project terminal and the user's own terminals keep full access.
- **Validator sessions (0063)** may read anything and run `validate pass|fail`; they change no ticket.
- **Project agents (0068)** may read anything, create tickets in Backlog, edit, link and prioritise tickets that are still
  in Backlog, `ask <KEY>`, and `agent name`. They can't move, start, approve, merge, close or delete tickets.

## What stays from 0009
- The plugin, the skills (`work-ticket`, `address-review`, `restack`), the hooks, and
  the launch through `--plugin-dir` with `OTAQ_TICKET`.

## Alternatives considered
- Keep the MCP server: typed tools, but an extra component and context cost.
- Both (CLI plus a thin MCP wrapper): unnecessary for now.

## Changelog
- 2026-10-09: created
- 2026-10-09: command names and conventions refined to follow beads `bd` (0032).
- 2026-10-09: `otaq` commands are pre-approved at launch instead of asking per ticket (0011).
- 2026-10-09: agents may read anything, change only their own ticket, and create Backlog tickets; enforced by the service (grilling session, user decision).
- 2026-10-11: permission scopes for validator sessions (0063) and Project agents (0068).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
