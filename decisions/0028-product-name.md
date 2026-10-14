# 0028. Product name (replaces the working name "factory")

- **Status:** Superseded by [0076](0076-product-name-otaq.md)
- **Date:** 2026-10-09
- **Related:** 0002, 0009, 0021, 0022, 0076

## Context
"factory" was a working name. It is generic as a command, and Factory (factory.ai) is an existing AI
coding-agent company, so "Software Factory" sat too close to their brand. The name spreads into the command,
the plugin, environment variables and the user's own skills, so the user chose to pick the final name now.

## Decision
The working name is replaced by a final product name. The name chosen on 2026-10-09 was "Crewboard"; 0076 renamed it
**Otaq** on 2026-10-15, before any release. This file keeps the mapping from the working name:

| Working name | Final |
|---|---|
| Software Factory / "the factory" | Otaq |
| `factory` command (`factory daemon`, `factory attach`, `factory mcp`, `factory hook`) | `otaq …` |
| `/factory:work-ticket` etc. (plugin namespace) | `/otaq:work-ticket` etc. |
| `FACTORY_TICKET` | `OTAQ_TICKET` |
| crates `factory-core` / `factory-app` / `factory-cli` | `otaq-core` / `otaq-app` / `otaq-cli` |

- The code reads the product name, command name, plugin namespace and environment variable prefix from
  **one constant**, so a forced rename later is cheap. (0076 was that rename.)

## Availability
- Checked 2026-10-09 for the first name: free on Homebrew, npm and crates.io. Its closeness to **CrewAI**, a well-known
  multi-agent framework in the same space, was one reason for 0076.
- **Not checked: trademarks.** Run a trademark search for "Otaq" before the public launch (open question in the index).

## Still to decide before publishing
- ~~License (MIT, Apache-2.0 or AGPL)~~. Decided: MIT (0054).

## Changelog
- 2026-10-09: created
- 2026-10-09: added a note to include CrewAI in the trademark check (product owner).
- 2026-10-09: license decided: MIT (0054).
- 2026-10-15: superseded by 0076: the product is renamed Otaq (user decision).
- 2026-10-15: rewritten for the name Otaq, file renamed from `0028-product-name-crewboard.md` (user decision: older
  decisions read with the current name).
