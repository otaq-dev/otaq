# 0006. Dependency visualization: badges, chain highlight, graph view

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0005

## Decision
- **Board:** each card shows dependency badges (`⛓ #12, #15`). Hovering over or selecting a card
  highlights its whole chain (parents and children) and dims everything else. Arrows are drawn only for
  the highlighted chain.
- **Stack list (v0.1):** the ticket side panel has a **Stack** section: an indented list of the ticket's
  parents and children, each with its status (for example "Paused (parent changed)"). Click a line to jump
  to that ticket.
- **Graph view: maybe later.** A full box-and-arrow screen showing the DAG is **not in v0.1**. It will be
  revisited after the Stack list has been used for a while. GPUI has no ready-made graph layout, so it would
  have to be built from scratch.

## Alternatives considered
- Always drawing every arrow on the board. Rejected because the board becomes unreadable past about
  15 tickets.

## Changelog
- 2026-10-08: created
- 2026-10-09: added the Stack list to v0.1. The graph view moved to "maybe later" (Topic 12).
- 2026-10-09: dependency badges and the Stack list stay; with 0049 they show "waits for SHOP-18 to merge" rather than stacked branches.
