# 0009. Agent behavior packaged as a Claude Code plugin

- **Status:** Accepted. The MCP server part is superseded by 0031 (CLI + skills).
- **Date:** 2026-10-08
- **Related:** 0002, 0004, 0008

## Context
Putting the ticket and instructions into a long `claude "<prompt>"` string hard-codes agent behavior
in Rust code and requires shell escaping. The user chose a plugin because it is easier to maintain.

## Decision
- The app launches: `FACTORY_TICKET=<n> claude --plugin-dir <plugin> "/factory:work-ticket"`.
  The exact flag for loading a local plugin is still to be verified when this is built.
- The `plugin/` folder contains:
  - **Skills:** `work-ticket` (fetch the ticket, plan, implement, `ready_for_review`),
    `address-review` (fetch the feedback, fix it, resubmit), `restack` (resolve rebase conflicts), `investigate-ticket` (study the ticket, post findings and questions,
    update the ticket; 0019).
  - **MCP server** (`factory mcp`, stdio, connected to the background service over its local socket (0022)) with the tools
    `get_ticket`, `get_review_feedback`, `create_ticket`, `link_tickets`, `ask_ticket`, `answer_question` (0030), `update_ticket`, `post_investigation` (0019),
    `ready_for_review(summary)` (moves the ticket to Ready for Review. The
    factory runs no checks; any verification or retry logic lives in the user's own skills) and `ask_human(question)`.
  - **Hooks:** `Notification` sets the "needs you" badge; `Stop` without a submission marks the card
    "idle, needs attention". Hooks do nothing when `FACTORY_TICKET` is unset.
- **Send back:** the app types `/factory:address-review` into the session, or into
  `claude --resume <id>` if the session is gone.

## Consequences
- Agent behavior can be changed by editing Markdown, without recompiling the app.
- The plugin also works outside the app (`FACTORY_TICKET=12 claude` in a worktree).

## Changelog
- 2026-10-08: created
- 2026-10-09: the MCP server was replaced by `otaq` CLI commands taught by skills (0031).
- 2026-10-09: the socket now belongs to the background service (0022).
- 2026-10-09: added the investigation skill and the ticket tools (0019).
- 2026-10-09: the `investigate-ticket` skill was removed (0036).
- 2026-10-09: removed the factory test gate, retry limit and Failed column. Checks and retries belong in the user's own skills (0018).
- 2026-10-09: added built-in workflow skills `commit`, `push`, `open-pr` (replaceable per project, 0050) and `answer-question` (0030).
- 2026-10-09: added a `StopFailure` hook for usage-limit detection (0020).
- 2026-10-09: hooks: Stop reads background_tasks/session_crons; Notification mapped by type; UserPromptSubmit marks the next turn (0042).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
