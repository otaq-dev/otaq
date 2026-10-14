# 0051. Each ticket has a base branch (project default, per-ticket override)

- **Status:** Accepted
- **Note (0059):** epic children use their **epic's branch** as base (not the epic's base). The project base is prefilled from the remote's default branch.
- **Date:** 2026-10-09
- **Related:** 0015, 0024, 0038, 0044, 0049, 0050

## Context
Grilling session, question 12. Each project had one base branch (usually `main`): every ticket started from it, Local merge
squashed into it and Open PR targeted it. Teams that branch by kind of work (0050) need more. In `shop-web`, features start
from `develop`, but a production hotfix (SHOP-31) must start from `main` (or `release/2.3`) and its PR must target `main`.

## Decision
- **The project's base branch is the default** (project settings), for example `develop`.
- **Each ticket can override it** in the ticket form: "Start from and merge into: [develop ▾]", choosing from local and
  remote branches. It's **fixed once the ticket starts**.
- Everything that said "main" uses **the ticket's base**: where the branch starts, Local merge, the Open PR target, the
  "conflicts with …" flag (for example "conflicts with `develop`", 0038), and the rebases in 0038 and 0044.
- **Linked tickets must share a base.** A dependency is ready when the parent is merged into the child's base (0049), so a
  link across bases is rejected in the editor and by `otaq dep add`, with a hint: "SHOP-31 merges into `main`, SHOP-20
  into `develop`. Linked tickets must use the same base branch."
- **Epics (grilling session, question 19):**
  - An epic has a base branch (the project default, editable on the epic). Changing it is allowed only while none of its
    children has started.
  - **Children always use the epic's base**, with no per-child override. The child's editor shows it read-only: "From the
    epic: `develop`".
  - Depending on an epic (0046) follows the same rule: the dependent ticket must have the epic's base.
  - **Move to epic** (0044): the never-started ticket adopts the epic's base, and the dialog says so ("SHOP-51 will start
    from `develop` instead of `main`").
- **Extra merges are not modeled.** Gitflow's "also merge the hotfix back into develop" is up to the user, or to their Open
  PR skill (0050), which can open a second PR.

## Alternatives considered
- One base branch per project: rejected, because hotfixes and release work need a different base.
- Letting the Open PR skill pick the target at push time: rejected, because the branch must start from the right base,
  and Local merge needs a target too.

## Changelog
- 2026-10-09: created (grilling session, user decision)
- 2026-10-09: epics have a base branch; children always use it; epic dependencies and Move to epic follow it (grilling session, user decision).
- 2026-10-09: the base branch setting is prefilled from the remote's default branch; epic children use the epic's branch (0059).
- 2026-10-15: wording updated to the name Otaq and the needs-you family in turquoise (0076, 0077); the decision itself is unchanged.
