# 0057. Repository layout and design source

- **Status:** Accepted
- **Date:** 2026-10-09
- **Related:** 0002, 0023, 0053, 0054, 0056

## Decision
- **This folder is the Otaq repository.** M0 runs `git init` here and adds `LICENSE` (MIT, 0054), the Cargo workspace
  (`crates/otaq-core`, `crates/otaq-app`, `crates/otaq-cli`) and `plugin/`, next to the existing `decisions/`,
  `design-briefs/`, `DESIGN.md` and `SDD.md`.
- **The design lives only in pen.dev**, in the document **`crewboard.pen`**. It is not exported into the repo. The SDD and
  UI tickets refer to it by document and frame name (for example "crewboard.pen → 03 · Board — shop-web · 1440 · Light").
  Agents that build UI need the pen.dev connection.

## Changelog
- 2026-10-09: created (SDD preparation, user decision)
- 2026-10-15: the product is Otaq (0076); the design file keeps its name `crewboard.pen`.
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
