# 0012. Built as a possible open-source product, so flexibility comes first

- **Status:** Accepted
- **Date:** 2026-10-08
- **Related:** 0003, 0009, 0010, 0011

## Context
The tool starts as the user's own workflow automation, but it may be published as an open-source product.

## Decision
Design for other people's setups, not only the author's:
- Respect existing user configuration (Claude Code settings, git config) rather than overriding it.
- Put per-project behavior in **in-app per-repo settings** instead of hard-coding it. Never add files to user repos (0023).
- Keep integration points pluggable: ticket importers, the approve action (merge or PR), and preview
  and services commands.

## Consequences
- Each new decision should state what is configurable and what the default is.
- macOS, Windows and Linux are all first-class (0021). Avoid platform-specific dependencies (for example,
  tmux was replaced in 0022).

## Changelog
- 2026-10-08: created
- 2026-10-09: per-project config moved from `.factory.toml` to in-app settings (0023).
- 2026-10-09: platform note updated for 0021 and 0022.
