# 0062. v0.1 is built outside Crewboard; Crewboard is used from the work after v0.1

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0025, 0053 (dogfooding part superseded), 0056
- **Supersedes:** the "Dogfooding" bullet of 0053 (the remaining v0.1 tickets run through Crewboard once the core loop works,
  around M6) and the matching 0025 changelog line

## Context
Gap review before development. 0053 switched Crewboard's own development to Crewboard as soon as the core loop worked (the
ticket marked `Dogfood: yes`, T-0095 in M6). The review found what that switch would cost in the middle of v0.1:
- **Restarts:** building Crewboard with Crewboard means rebuilding and restarting the background service often, but Interrupted
  and Resume only arrive in M7. A restart in M6 would end every running agent with no way to resume it.
- **Importing the tickets:** the remaining tickets are grouped into one epic per milestone. Under 0051 and 0059, links between
  tickets of different epics are refused (for example T-0129 in M8 depends on T-0115 in M7), and an epic's work only reaches
  `main` when the whole epic is reviewed.
- **Installing a personal build:** no ticket covers putting your own build on the PATH, or an app bundle for notifications.

## Decision
- **All of v0.1 (M0–M11) is built outside Crewboard**, by an agent following `prompts/implementer.md`, one milestone per
  session with a review stop after each milestone (0056).
- **Crewboard is used for the work that comes after v0.1**: new features for Crewboard itself, and the user's other projects.
- T-0095 no longer carries a `Dogfood: yes` marker. Nothing in v0.1 switches to Crewboard part-way.

## Alternatives considered
- **Switch at M6, as 0053 planned:** rejected for the restart, import and install problems above.
- **Switch after M7** (once Interrupted and Resume exist): rejected. The import problem would remain, and the user prefers one
  clean switch at the end.

## Consequences
- The import problem and the "restart before M7" problem go away. No decision is needed on how to import the v0.1 tickets.
- **Still open, before Crewboard is first used:** how the personal build is installed (the `crewboard` command on the PATH, the
  plugin folder, an app bundle so macOS notifications work). It is a separate topic, to be decided before the switch.
- 0056's definition of done applies to every v0.1 ticket, all built by the implementing agent.

## Changelog
- 2026-10-09: created (gap review, user decision)
