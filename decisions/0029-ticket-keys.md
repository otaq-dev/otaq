# 0029. Ticket keys: project key + number (SHOP-18)

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0009, 0024, 0028, 0030

## Context
Ticket numbers like `#18` only made sense within one project. With multiple projects (0024) and agents that
refer to each other's tickets (0030), every ticket needs an ID that is unique across all projects and
readable by people.

## Decision
- Each project has a short **key**, suggested from the repo name when the project is added and editable at
  that point (for example `SHOP` for shop-web and `BILL` for billing-api).
- Tickets are identified as **`<KEY>-<number>`**, for example **SHOP-18**. Numbers increase per project and are
  **never reused**, even after deletion.
- The key is used everywhere: the card, the temporary branch name (`otaq/SHOP-18`; the real name is chosen at the first push, 0050), the worktree folder
  (`<app-data>/wt/<repo-name>/SHOP-18`, for example `~/.otaq/wt/shop-web/SHOP-18`), the session name,
  `OTAQ_TICKET=SHOP-18`, commit messages, inbox items, `otaq attach SHOP-18`, and the `otaq` commands (0031).
- Internally the database can also use surrogate IDs. The key is the user-facing and agent-facing identifier.
- Older decisions and examples that use `#18` should be read as `SHOP-18`.
- **Epic children** use hierarchical keys, `SHOP-40.1`, `SHOP-40.2` (one level, beads-style; 0039).

## Open
- Whether a project key can be renamed after tickets exist (it would affect branch names). Default: not renamable.

## Changelog
- 2026-10-09: created
- 2026-10-09: hierarchical keys for epic children (0039).
- 2026-10-09: worktree folders are named by ticket key (raised by the design agent).
- 2026-10-09: branch names are temporary until the first push, then chosen by the Open PR skill and confirmed by the user (0050).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
