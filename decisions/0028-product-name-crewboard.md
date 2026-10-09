# 0028. Product name: Crewboard

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0002, 0009, 0021, 0022

## Context
"factory" was a working name. It is generic as a command, and Factory (factory.ai) is an existing AI
coding-agent company, so "Software Factory" sat too close to their brand. The name spreads into the command,
the plugin, environment variables and the user's own skills, so the user chose to pick the final name now.

## Decision
The product is **Crewboard**: "the board where you run your crew of agents".

Renames (older decisions use the working name. Read them with this mapping):

| Working name | Final |
|---|---|
| Software Factory / "the factory" | Crewboard |
| `factory` command (`factory daemon`, `factory attach`, `factory mcp`, `factory hook`) | `crewboard …` |
| `/factory:work-ticket` etc. (plugin namespace) | `/crewboard:work-ticket` etc. |
| `FACTORY_TICKET` | `CREWBOARD_TICKET` |
| crates `factory-core` / `factory-app` / `factory-cli` | `crewboard-core` / `crewboard-app` / `crewboard-cli` |

- The code still reads the product name, command name, plugin namespace and environment variable prefix from
  **one constant**, so a forced rename later is cheap.

## Availability (checked 2026-10-09)
- Free on Homebrew (formula and cask), npm and crates.io. GitHub had about 57 small repos with the name, none notable.
- **Not checked: trademarks.** Run a trademark search before the public launch.
  Include **CrewAI**, a well-known open-source multi-agent framework in the same space. "Crewboard"
  sits close to it, much as the old name sat close to Factory.ai.

## Still to decide before publishing
- ~~License (MIT, Apache-2.0 or AGPL)~~. Decided: MIT (0054).

## Changelog
- 2026-10-09: created
- 2026-10-09: added a note to include CrewAI in the trademark check (product owner).
- 2026-10-09: license decided: MIT (0054).
